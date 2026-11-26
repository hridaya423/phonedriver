//! The calls that are not touches: apps, shell, recordings, logs, web pages, settings, health, screenshots and builds.

use super::*;

/// How many files a listing shows, newest first: of each shelf when it reads all three, and of one folder.
const LISTED_OF_A_SHELF: usize = 10;
const LISTED_OF_A_FOLDER: usize = 50;

impl Actor {
    /// Install, stop, clear data or uninstall. On a physical device only an app phonedriver installed (recorded in
    /// `installed.json` under the daemon's home) can be cleared or removed.
    pub(super) async fn app(&mut self, s: &mut Session, p: AppParams) -> Result<Value, Error> {
        let device = self.device()?;
        if p.action == "install" {
            let path = p.path.ok_or(Error::Unsupported {
                op: Op::App,
                reason: "install needs the path of an app file",
            })?;
            let added = device.install(std::path::Path::new(&path)).await?;
            self.forget_apps();
            let summary = match added.as_slice() {
                [] => "installed (it replaced an app already there)".to_string(),
                [id] => {
                    if self.physical() {
                        let installed_at = device.installed_at(id).await?;
                        let key = self.ledger_key(&*device).await?;
                        self.ledger
                            .update(&key, |l| {
                                l.retain(|e| e.id != id.0);
                                l.push(Entry {
                                    id: id.0.clone(),
                                    installed_at,
                                });
                            })
                            .map_err(|why| Error::Transport {
                                detail: format!("installed {}, but {why}", id.0),
                            })?;
                    }
                    format!("installed {}", id.0)
                }
                several => {
                    let names: Vec<&str> = several.iter().map(|a| a.0.as_str()).collect();
                    format!(
                        "installed, but {} packages are new ({}) and phonedriver cannot tell which is this app, so it records none of them",
                        several.len(),
                        names.join(", ")
                    )
                }
            };
            return Ok(json!({"ok": true, "summary": summary}));
        }
        let op = match p.action.as_str() {
            "stop" => Some(AppOp::Stop),
            "clear" => Some(AppOp::ClearData),
            "uninstall" => Some(AppOp::Uninstall),
            "grant" | "revoke" | "notify" => None,
            _ => {
                return Err(Error::Unsupported {
                    op: Op::App,
                    reason: "the app action is install, stop, clear, uninstall, grant, revoke or notify",
                });
            }
        };
        let query = p.app.ok_or(Error::Unsupported {
            op: Op::App,
            reason: "this action needs the app's name or id",
        })?;
        let found = self.app_named(&*device, &query).await?;
        if op == Some(AppOp::Uninstall) {
            self.forget_apps();
        }
        let Some(op) = op else {
            if p.action == "notify" {
                let payload = p.payload.ok_or(Error::Unsupported {
                    op: Op::App,
                    reason: "notify needs the push notification as a JSON `payload`, such as {\"aps\": {\"alert\": \"Hello\"}}",
                })?;
                device.push(&found.id, &payload).await?;
                s.set_last(None);
                return Ok(json!({
                    "ok": true,
                    "summary": format!("sent a push notification to {}", found.name),
                }));
            }
            if p.permissions.is_empty() {
                return Err(Error::Unsupported {
                    op: Op::App,
                    reason: "grant and revoke need `permissions`, such as camera or location",
                });
            }
            if self.physical() && !self.installed_by_us(&*device, &found.id).await? {
                return Err(Error::Unsupported {
                    op: Op::App,
                    reason: concat!(
                        "phonedriver did not install this app on this physical device, or it was installed again since, so it will not change its permissions",
                        own_app_hint!()
                    ),
                });
            }
            let names = p.permissions.join(", ");
            let summary = if p.action == "grant" {
                device.grant(&found.id, &p.permissions).await?;
                format!("granted {names} to {}", found.name)
            } else {
                device.revoke(&found.id, &p.permissions).await?;
                format!("revoked {names} from {}", found.name)
            };
            s.set_last(None);
            return Ok(json!({"ok": true, "summary": summary}));
        };
        if op != AppOp::Stop
            && self.physical()
            && !self.installed_by_us(&*device, &found.id).await?
        {
            return Err(Error::Unsupported {
                op: Op::App,
                reason: concat!(
                    "phonedriver did not install this app on this physical device, or it was installed again since, so it will not clear or remove it",
                    own_app_hint!()
                ),
            });
        }
        device.app_op(op, &found.id).await?;
        if op == AppOp::Uninstall && self.physical() {
            let key = self.ledger_key(&*device).await?;
            self.ledger
                .update(&key, |l| l.retain(|e| e.id != found.id.0))
                .map_err(|why| Error::Transport {
                    detail: format!("uninstalled {}, but {why}", found.name),
                })?;
        }
        s.set_last(None);
        let word = match op {
            AppOp::Stop => "stopped",
            AppOp::ClearData => "cleared the data of",
            AppOp::Uninstall => "uninstalled",
        };
        Ok(json!({"ok": true, "summary": format!("{word} {}", found.name)}))
    }

    /// Runs a command in the device's shell, then (unless it only reads) settles and reports what changed on screen.
    pub(super) async fn shell(
        &mut self,
        s: &mut Session,
        p: ShellParams,
    ) -> Result<ShellResult, Error> {
        self.need(
            Capabilities::SHELL,
            Op::Shell,
            "an iPhone or iPad has no shell for anyone to run commands in; use launch, open, type and the other verbs, or a Simulator",
        )?;
        if self.physical() && !p.force {
            let device = self.device()?;
            let key = self.ledger_key(&*device).await?;
            let named: Vec<String> = self
                .ledger
                .entries(&key)
                .map_err(|detail| Error::Transport { detail })?
                .into_iter()
                .map(|e| e.id)
                .filter(|id| p.command.contains(id.as_str()))
                .collect();
            let mut mine = Vec::new();
            for id in named {
                let app = AppId(id);
                if matches!(self.installed_by_us(&*device, &app).await, Ok(true)) {
                    mine.push(app.0);
                }
            }
            if let Some(reason) = shell::refuse_on_physical(&p.command, &mine) {
                return Err(Error::Unsupported {
                    op: Op::Shell,
                    reason,
                });
            }
        }
        let settle = p.settle && !shell::only_reads(&p.command);
        let mut cfg = Config::action();
        cfg.expect_change = Some(Duration::from_millis(p.expect_change_ms.unwrap_or(250)));
        if let Some(t) = p.timeout_ms {
            cfg.timeout = Duration::from_millis(t);
        }
        if settle && s.last.is_none() {
            let now = self.fresh(s).await?;
            s.set_last(Some(now));
        }
        let limit = Duration::from_secs(p.time_limit_s.unwrap_or(30));
        let device = self.device()?;
        let req = ShellRequest {
            command: p.command.clone(),
            timeout: limit,
        };
        let started = self.begin_action();
        let ran = Instant::now();
        let reply = tokio::time::timeout(limit + Duration::from_secs(5), device.shell(&req))
            .await
            .map_err(|_| Error::Timeout {
                waited_ms: (limit.as_millis() + 5000) as u32,
                condition: "the shell command to return".into(),
            })??;
        let ms = ran.elapsed().as_millis() as u32;
        let tidy = shell::tidy(
            &reply.stdout,
            &reply.stderr,
            p.grep.as_deref(),
            p.head.unwrap_or(shell::DEFAULT_HEAD),
        );
        let action = if settle {
            let summary = if p.secret {
                format!("ran a command ({} chars)", p.command.chars().count())
            } else {
                "ran".to_string()
            };
            Some(
                self.finish(s, true, cfg, started, summary, None, None)
                    .await?,
            )
        } else {
            if !p.settle {
                self.unobserved = Some(started);
            }
            None
        };
        Ok(ShellResult {
            exit: reply.exit,
            output: tidy.text,
            omitted: tidy.omitted,
            timed_out: reply.timed_out,
            ms,
            action,
        })
    }

    /// Starts or stops a recording of the screen (decision D59). The device writes the file; the daemon keeps the
    /// actions done meanwhile beside it.
    pub(super) async fn record(&mut self, p: RecordParams) -> Result<Value, Error> {
        let device = self.device()?;
        match p.action {
            RecordAction::Start => {
                let named = p.path.is_some();
                let path = p.path.unwrap_or_else(|| self.tmp_file("mp4"));
                if let Some(dir) = path.parent() {
                    if named {
                        std::fs::create_dir_all(dir)
                    } else {
                        crate::private::dir(dir)
                    }
                    .map_err(|e| Error::Transport {
                        detail: format!("cannot create {}: {e}", dir.display()),
                    })?;
                }
                device
                    .record_start(&path)
                    .await
                    .map_err(|e| match (e, &self.recording) {
                        (Error::Refused { .. }, Some(running)) => Error::Refused {
                            detail: format!(
                                "a recording is already running to {}; `record stop` ends it",
                                running.path.display()
                            ),
                        },
                        (e, _) => e,
                    })?;
                let taping = Taping {
                    path: path.clone(),
                    began: Instant::now(),
                    steps: 0,
                };
                match std::fs::remove_file(taping.steps_path()) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                        let _ = device.record_stop().await;
                        return Err(Error::Refused {
                            detail: format!(
                                "the steps file of an earlier recording to this path cannot be replaced: {e}"
                            ),
                        });
                    }
                    _ => {}
                }
                self.recording = Some(taping);
                Ok(json!({"path": path}))
            }
            RecordAction::Stop => {
                let Some(taping) = self.recording.take() else {
                    return Err(Error::Refused {
                        detail: "no recording is running; `record start` begins one".into(),
                    });
                };
                let Some(done) = device.record_stop().await? else {
                    return Err(Error::Refused {
                        detail: format!(
                            "the recording ended when the connection to the device did; {} holds the video up to then",
                            taping.path.display()
                        ),
                    });
                };
                let bytes = std::fs::metadata(&done.path).map(|m| m.len()).unwrap_or(0);
                let mut reply = json!({
                    "path": done.path,
                    "ms": done.ms,
                    "frames": done.frames,
                    "width": done.width,
                    "height": done.height,
                    "bytes": bytes,
                    "steps": taping.steps,
                });
                if taping.steps > 0 {
                    reply["steps_path"] = json!(taping.steps_path());
                }
                if let Some(why) = done.early {
                    reply["early"] = json!(why);
                }
                Ok(reply)
            }
        }
    }

    /// A new file name under this device's folder in the daemon's `tmp`.
    pub(super) fn tmp_file(&self, extension: &str) -> PathBuf {
        self.home
            .join("tmp")
            .join(self.id.replace([':', '/', '\\'], "_"))
            .join(format!("{}.{extension}", ulid::Ulid::generate()))
    }

    /// The last lines an app logged, the app in front unless one is named, and its last crash (decision D58).
    pub(super) async fn logs(&mut self, s: &mut Session, p: LogsParams) -> Result<Value, Error> {
        let device = self.device()?;
        let app = match p.app {
            Some(name) => self.find_app(&device.apps().await?, &name)?.id.clone(),
            None => {
                let now = self.fresh(s).await?;
                let front = now.header.app.as_ref().map(|a| a.id.clone());
                AppId(front.ok_or(Error::Unsupported {
                    op: Op::Logs,
                    reason: "no app is in front; name one with `app`",
                })?)
            }
        };
        let reply = device
            .logs(&LogRequest {
                app: app.clone(),
                since: Duration::from_secs(p.since_s.unwrap_or(LOGS_SINCE_S)),
                crashes: false,
            })
            .await?;
        let all = shell::tidy(&reply.text, "", p.grep.as_deref(), 0);
        let lines: Vec<&str> = all.text.lines().collect();
        let keep = match p.head.unwrap_or(shell::DEFAULT_HEAD) {
            0 => lines.len(),
            n => n.min(lines.len()),
        };
        let omitted = lines.len() - keep;
        Ok(json!({
            "app": app.0,
            "output": lines[omitted..].join("\n"),
            "omitted": omitted,
            "crash": reply.crash,
        }))
    }

    /// The web page in front as text, whole (decision D71): no blank lines, and cut at a line end near `PAGE_CHARS`
    /// unless `all`. With `grep`, only the lines with one of its words, each with the `GREP_LINES_AFTER` lines after it.
    pub(super) async fn page(&mut self, p: PageParams) -> Result<Value, Error> {
        let page = self.device()?.page(None).await?;
        let keep = p
            .grep
            .as_deref()
            .filter(|g| !g.is_empty())
            .map(shell::matcher);
        let all: Vec<&str> = page
            .text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let lines: Vec<&str> = match &keep {
            None => all,
            Some(keep) => (0..all.len())
                .filter(|&i| (i.saturating_sub(GREP_LINES_AFTER)..=i).any(|j| keep(all[j])))
                .map(|i| all[i])
                .collect(),
        };
        let mut text = String::new();
        let mut shown = 0;
        let mut used = 0;
        let mut cut = 0;
        for line in &lines {
            let length = line.chars().count();
            if !p.all && used + length > PAGE_CHARS {
                if shown == 0 {
                    text.extend(line.chars().take(PAGE_CHARS));
                    text.push('…');
                    cut = length - PAGE_CHARS;
                    shown = 1;
                }
                break;
            }
            if shown > 0 {
                text.push('\n');
            }
            text.push_str(line);
            used += length;
            shown += 1;
        }
        let left: usize = cut
            + lines[shown..]
                .iter()
                .map(|l| l.chars().count())
                .sum::<usize>();
        Ok(json!({
            "title": page.title, "url": page.url, "text": text,
            "lines": lines.len(), "omitted_lines": lines.len() - shown, "omitted_chars": left,
        }))
    }

    /// Finds, reads, writes or opens a system setting by key (decision D57). A write settles and reports the
    /// screen change, as `shell` does.
    pub(super) async fn setting(
        &mut self,
        s: &mut Session,
        p: SettingParams,
    ) -> Result<Value, Error> {
        let device = self.device()?;
        let SettingReply::Index(index) = device.setting(SettingOp::Index).await? else {
            return Err(Error::Transport {
                detail: "the device answered a settings index request with a value".into(),
            });
        };
        if let Some(query) = &p.find {
            let searches: Vec<&str> = query.split('|').filter(|q| !q.trim().is_empty()).collect();
            let (limit, screens) = if searches.len() > 1 {
                (SETTING_LINES_OF_MANY, 1)
            } else {
                (SETTING_LINES, 3)
            };
            let mut lines: Vec<String> = Vec::new();
            let mut more = 0;
            for search in searches {
                let found = index.find(search, limit, screens);
                more += found.more;
                for line in found.lines {
                    if !lines.contains(&line) {
                        lines.push(line);
                    }
                }
            }
            return Ok(json!({"lines": lines, "more": more}));
        }
        if let Some(name) = p.open {
            let Some(found) = index.named(&name) else {
                let near: Vec<String> = index
                    .near(&name, SETTING_SCREENS_NEAR)
                    .iter()
                    .map(|s| format!("{} ({})", s.key, s.title()))
                    .collect();
                return Err(Error::Refused {
                    detail: if near.is_empty() {
                        format!(
                            "no settings screen has the key {name:?}; `find` lists screens as `screen <key>`"
                        )
                    } else {
                        format!(
                            "no settings screen has the key {name:?}; open one of: {}",
                            near.join(", ")
                        )
                    },
                });
            };
            let screen = found.key.clone();
            let link = found.link.clone().ok_or_else(|| Error::Refused {
                detail: format!(
                    "the settings screen {screen} cannot be opened by key: it shows one item (an app, a SIM) the device must be told"
                ),
            })?;
            let act = ActParams {
                action: Action::Open {
                    url: link,
                    app: None,
                },
                approval: None,
                approved: None,
                ask: Vec::new(),
                settle: p.settle,
                timeout_ms: p.timeout_ms,
                expect_change_ms: p.expect_change_ms,
                secret_field_only: false,
                on: None,
                next: None,
            };
            let mut result = self.act(s, act).await?;
            result.summary = format!("opened the settings screen {screen}");
            return Ok(to_value(result));
        }
        let unknown = |key: &str| {
            let found = index.find(&key.replace(['/', '_'], " "), 3, 0);
            let close: Vec<&str> = found
                .lines
                .iter()
                .filter_map(|l| l.split_whitespace().next())
                .collect();
            Error::Refused {
                detail: if close.is_empty() {
                    format!("no setting has the key {key:?}; `find` with a word lists keys")
                } else {
                    format!(
                        "no setting has the key {key:?}; closest: {}",
                        close.join(", ")
                    )
                },
            }
        };
        if let Some(keys) = p.restore {
            return self
                .restore_settings(s, &index, keys, p.force, p.settle, p.timeout_ms)
                .await;
        }
        let single = p.values.is_none();
        let writes: Vec<(String, String)> = match (p.values, p.key, p.value) {
            (Some(values), _, _) => values.into_iter().collect(),
            (None, Some(key), Some(value)) => vec![(key, value)],
            (None, key, _) => {
                let key = key.unwrap_or_default();
                if index.item(&key).is_none() {
                    return Err(unknown(&key));
                }
                let SettingReply::Value { value, hint } =
                    device.setting(SettingOp::Get(key.clone())).await?
                else {
                    return Err(Error::Transport {
                        detail: "the device answered a settings read with an index".into(),
                    });
                };
                return Ok(json!({"key": key, "value": value, "hint": hint}));
            }
        };
        for (key, _) in &writes {
            let item = index.item(key).ok_or_else(|| unknown(key))?;
            if !item.writable {
                return Err(Error::Refused {
                    detail: format!("{key} is read-only"),
                });
            }
            if item.confirm && self.physical() && !p.force {
                return Err(Error::Unsupported {
                    op: Op::Setting,
                    reason: "the device marks this setting as one its user must confirm; pass `force` only when the user asked for exactly this change",
                });
            }
        }
        let mut cfg = Config::action();
        cfg.expect_change = Some(Duration::from_millis(p.expect_change_ms.unwrap_or(250)));
        if let Some(t) = p.timeout_ms {
            cfg.timeout = Duration::from_millis(t);
        }
        if p.settle && s.last.is_none() {
            let now = self.fresh(s).await?;
            s.set_last(Some(now));
        }
        let store = self.ledger_key(device.as_ref()).await?;
        self.settings_store = Some(store.clone());
        let stored = |detail| Error::Transport { detail };
        let mut kept = originals::read(&self.home, &store).map_err(stored)?;
        let started = self.begin_action();
        let mut done: Vec<(String, String)> = Vec::new();
        let mut was = serde_json::Map::new();
        for (key, value) in writes {
            let first = !kept.contains_key(&key);
            if first {
                match device.setting(SettingOp::Get(key.clone())).await {
                    Ok(SettingReply::Value { value: before, .. })
                        if self.secrets.masked(&before) == before
                            && self.secrets.masked(&value) == value =>
                    {
                        kept.insert(
                            key.clone(),
                            Original {
                                was: before,
                                set: value.clone(),
                            },
                        );
                        originals::write(&self.home, &store, &kept).map_err(stored)?;
                    }
                    Ok(_) | Err(Error::Refused { .. }) => {}
                    Err(e) => return Err(e),
                }
            }
            let op = SettingOp::Set {
                key: key.clone(),
                value,
            };
            let wrote = device.setting(op).await;
            match (&wrote, kept.get(&key).cloned()) {
                (Ok(SettingReply::Value { value, .. }), Some(o)) if *value == o.was => {
                    kept.remove(&key);
                    originals::write(&self.home, &store, &kept).map_err(stored)?;
                }
                (Ok(SettingReply::Value { value, .. }), Some(o)) => {
                    if first {
                        was.insert(key.clone(), json!(o.was));
                    }
                    if self.secrets.masked(value) != *value {
                        kept.remove(&key);
                        originals::write(&self.home, &store, &kept).map_err(stored)?;
                    } else if *value != o.set {
                        kept.insert(
                            key.clone(),
                            Original {
                                set: value.clone(),
                                ..o
                            },
                        );
                        originals::write(&self.home, &store, &kept).map_err(stored)?;
                    }
                }
                (Err(_), Some(_)) if first => {
                    kept.remove(&key);
                    originals::write(&self.home, &store, &kept).map_err(stored)?;
                }
                _ => {}
            }
            match wrote {
                Ok(SettingReply::Value { value, .. }) => done.push((key, value)),
                Ok(SettingReply::Index(_)) => {
                    return Err(Error::Transport {
                        detail: "the device answered a settings write with an index".into(),
                    });
                }
                Err(Error::Refused { detail }) if !done.is_empty() => {
                    let before: Vec<String> =
                        done.iter().map(|(k, v)| format!("{k} = {v}")).collect();
                    return Err(Error::Refused {
                        detail: format!(
                            "{detail}{}{}",
                            phonedriver_core::error::ALREADY_SET,
                            before.join(", ")
                        ),
                    });
                }
                Err(e) => return Err(e),
            }
        }
        let action = self
            .finish(s, p.settle, cfg, started, "set".to_string(), None, None)
            .await?;
        if single {
            let (key, value) = done.remove(0);
            let mut reply = json!({"key": key, "value": value, "action": action});
            if let Some(before) = was.remove(&key) {
                reply["was"] = before;
            }
            return Ok(reply);
        }
        let values: serde_json::Map<String, Value> =
            done.into_iter().map(|(k, v)| (k, json!(v))).collect();
        let mut reply = json!({"values": values, "action": action});
        if !was.is_empty() {
            reply["was"] = Value::Object(was);
        }
        Ok(reply)
    }

    /// Writes back what each of `keys` (every changed key when there are none) was before phonedriver first wrote
    /// it, and says what happened to each (decision D120). A key whose value is no longer the one phonedriver left
    /// was changed by someone else since: it keeps that value.
    async fn restore_settings(
        &mut self,
        s: &mut Session,
        index: &phonedriver_core::settings::SettingsIndex,
        keys: Vec<String>,
        force: bool,
        settle: bool,
        timeout_ms: Option<u64>,
    ) -> Result<Value, Error> {
        let device = self.device()?;
        let store = self.ledger_key(device.as_ref()).await?;
        self.settings_store = Some(store.clone());
        let stored = |detail| Error::Transport { detail };
        let mut kept = originals::read(&self.home, &store).map_err(stored)?;
        if let Some(key) = keys.iter().find(|k| !kept.contains_key(*k)) {
            let changed: Vec<&str> = kept.keys().map(String::as_str).collect();
            return Err(Error::Refused {
                detail: if changed.is_empty() {
                    format!("phonedriver did not change {key} on this device, or any other setting")
                } else {
                    format!(
                        "phonedriver did not change {key} on this device; it changed {}",
                        changed.join(", ")
                    )
                },
            });
        }
        let keys: Vec<String> = if keys.is_empty() {
            kept.keys().cloned().collect()
        } else {
            keys
        };
        let mut cfg = Config::action();
        if let Some(t) = timeout_ms {
            cfg.timeout = Duration::from_millis(t);
        }
        if settle && s.last.is_none() && !keys.is_empty() {
            let now = self.fresh(s).await?;
            s.set_last(Some(now));
        }
        let started = self.begin_action();
        let mut lines: Vec<Value> = Vec::new();
        let mut wrote = false;
        for key in keys {
            let Some(o) = kept.get(&key).cloned() else {
                continue;
            };
            let now = match device.setting(SettingOp::Get(key.clone())).await {
                Ok(SettingReply::Value { value, .. }) => value,
                Ok(SettingReply::Index(_)) => {
                    return Err(Error::Transport {
                        detail: "the device answered a settings read with an index".into(),
                    });
                }
                Err(Error::Refused { detail }) => {
                    lines.push(json!({"key": key, "state": "failed", "value": o.set, "was": o.was, "why": detail}));
                    continue;
                }
                Err(e) => return Err(e),
            };
            let confirm = index.item(&key).is_some_and(|i| i.confirm);
            let line = if now == o.was {
                kept.remove(&key);
                json!({"key": key, "state": "already", "value": now, "was": o.was})
            } else if now != o.set {
                kept.remove(&key);
                json!({"key": key, "state": "kept", "value": now, "was": o.was})
            } else if confirm && self.physical() && !force {
                json!({"key": key, "state": "needs_force", "value": now, "was": o.was})
            } else {
                let op = SettingOp::Set {
                    key: key.clone(),
                    value: o.was.clone(),
                };
                match device.setting(op).await {
                    Ok(SettingReply::Value { value, .. }) => {
                        wrote = true;
                        kept.remove(&key);
                        json!({"key": key, "state": "restored", "value": value, "was": o.was, "from": now})
                    }
                    Ok(SettingReply::Index(_)) => {
                        return Err(Error::Transport {
                            detail: "the device answered a settings write with an index".into(),
                        });
                    }
                    Err(Error::Refused { detail }) => {
                        json!({"key": key, "state": "failed", "value": now, "was": o.was, "why": detail})
                    }
                    Err(e) => return Err(e),
                }
            };
            originals::write(&self.home, &store, &kept).map_err(stored)?;
            lines.push(line);
        }
        let mut reply = json!({"restored": lines});
        if wrote {
            reply["action"] = to_value(
                self.finish(s, settle, cfg, started, "restored".to_string(), None, None)
                    .await?,
            );
        }
        Ok(reply)
    }

    /// What `release` answers: the settings phonedriver changed on this device that it has not put back. Nothing is
    /// restored here, because the change may be what the agent was asked for.
    pub(super) async fn settings_left(&mut self) -> Value {
        if self.settings_store.is_none()
            && let Ok(device) = self.device()
        {
            self.settings_store = self.ledger_key(device.as_ref()).await.ok();
        }
        let Some(store) = self.settings_store.clone() else {
            return json!({});
        };
        let left: Vec<Value> = originals::read(&self.home, &store)
            .unwrap_or_default()
            .into_iter()
            .map(|(key, o)| json!({"device": self.id, "key": key, "was": o.was, "set": o.set}))
            .collect();
        if left.is_empty() {
            json!({})
        } else {
            json!({"settings_changed": left})
        }
    }

    /// Copies one file between the folder the person allowed and the device's shared storage (decision D119). The
    /// reply is a receipt: where the file is now, its size and its SHA-256, never its bytes.
    pub(super) async fn files(
        &mut self,
        s: &mut Session,
        p: crate::wire::FilesParams,
    ) -> Result<Value, Error> {
        use super::super::files as host;
        let refused = |detail| Error::Refused { detail };
        let device = self.device()?;
        let folder = host::folder(p.folder.as_deref()).map_err(refused)?;
        if p.push.is_none() && p.pull.is_none() {
            return self.files_listed(s, p).await;
        }
        if let Some(name) = p.push {
            let from = host::source(&folder, &name).map_err(refused)?;
            let base = from
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| refused(format!("{name} has a name that is not text")))?;
            let base = phonedriver_core::files::name(base).map_err(refused)?;
            let to = p.to.unwrap_or(phonedriver_core::files::Shelf::of(base));
            let bytes = tokio::fs::read(&from)
                .await
                .map_err(|e| refused(format!("cannot read {}: {e}", from.display())))?;
            host::too_large(&name, bytes.len() as u64).map_err(refused)?;
            let sha256 = host::sha256(&bytes);
            self.transfer_allowed(
                p.approval,
                p.approved.as_ref(),
                format!("copy {base} to the phone's {} folder", to.folder()),
                vec![
                    format!("from {}", from.display()),
                    format!("{} bytes, SHA-256 {sha256}", bytes.len()),
                ],
            )?;
            let at = device.file_push(to, base, &bytes).await?;
            return Ok(
                json!({"pushed": name, "to": to, "device_path": at, "bytes": bytes.len(), "sha256": sha256}),
            );
        }
        let asked = p.pull.unwrap_or_default();
        let path = phonedriver_core::files::shared(&asked).map_err(refused)?;
        let base = path.rsplit('/').next().unwrap_or_default().to_string();
        let to = host::destination(&folder, &base).map_err(refused)?;
        let named = path.strip_prefix("/sdcard/").unwrap_or(&path);
        self.transfer_allowed(
            p.approval,
            p.approved.as_ref(),
            format!("copy {named} from the phone"),
            vec![format!("to {}", to.display())],
        )?;
        let bytes = device.file_pull(&path, host::LIMIT).await?;
        host::keep(&to, &bytes).map_err(refused)?;
        Ok(
            json!({"pulled": named, "file": base, "bytes": bytes.len(), "sha256": host::sha256(&bytes)}),
        )
    }

    /// What a pull could name: the newest files of each shelf, or more of the one folder asked for, each with its
    /// size and when it was written. The names of a person's files are theirs, so on their phone a listing waits
    /// for their word, which may be for the session; it moves no file.
    async fn files_listed(
        &mut self,
        s: &mut Session,
        p: crate::wire::FilesParams,
    ) -> Result<Value, Error> {
        let within = p
            .list
            .as_deref()
            .map(phonedriver_core::files::folder)
            .transpose()
            .map_err(|detail| Error::Refused { detail })?;
        let label = match &within {
            Some(folder) => format!("list the names of the files in the phone's {folder} folder"),
            None => format!(
                "list the names of the files in the phone's {} folders",
                phonedriver_core::files::PULL_FROM.join(", ")
            ),
        };
        if self.physical() && !s.files_listed {
            match (p.approval, p.approved.as_ref()) {
                (Some(Approval::Autonomous), _) => {}
                (Some(Approval::Once), Some(shown)) if shown.label == label => {}
                (Some(Approval::Session), Some(shown)) if shown.label == label => {
                    s.files_listed = true;
                }
                _ => {
                    return Err(Error::NeedsApproval {
                        label,
                        category: Category::Files,
                        app: None,
                        details: Vec::new(),
                    });
                }
            }
        }
        let listing = self.device()?.file_list(within.as_deref()).await?;
        let mut files = listing.files;
        files.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.path.cmp(&b.path)));
        let all = files.len();
        let shown: Vec<_> = match within {
            Some(_) => files.into_iter().take(LISTED_OF_A_FOLDER).collect(),
            None => {
                let mut of_shelf = std::collections::BTreeMap::<String, usize>::new();
                files
                    .into_iter()
                    .filter(|f| {
                        let shelf = f.path.split('/').next().unwrap_or_default();
                        let n = of_shelf.entry(shelf.to_string()).or_default();
                        *n += 1;
                        *n <= LISTED_OF_A_SHELF
                    })
                    .collect()
            }
        };
        Ok(json!({"files": shown, "more": all - shown.len(), "now": listing.now}))
    }

    /// On a person's phone every transfer waits for their word, and their word is for the transfer they were
    /// shown: this file, these bytes, this place. An approval for the session does not cover the next file.
    fn transfer_allowed(
        &self,
        approval: Option<Approval>,
        approved: Option<&Approved>,
        label: String,
        details: Vec<String>,
    ) -> Result<(), Error> {
        if !self.physical() {
            return Ok(());
        }
        match (approval, approved) {
            (Some(Approval::Autonomous), _) => Ok(()),
            (Some(Approval::Once | Approval::Session), Some(shown))
                if shown.label == label && shown.details == details =>
            {
                Ok(())
            }
            _ => Err(Error::NeedsApproval {
                label,
                category: Category::Files,
                app: None,
                details,
            }),
        }
    }

    /// Everything but an emulator or a Simulator belongs to a person.
    pub(super) fn physical(&self) -> bool {
        match self.id.split_once(':') {
            Some(("android", serial)) => !serial.starts_with("emulator-"),
            Some(("sim" | "fake", _)) => false,
            _ => true,
        }
    }

    /// The key of this device's entries in the record: the phone's own serial number, which a change of connection
    /// does not change, so a phone that later gets this one's address does not inherit its list.
    pub(super) async fn ledger_key(&self, device: &dyn Device) -> Result<String, Error> {
        let live = self.live.as_ref().ok_or_else(|| Error::DeviceGone {
            id: self.id.clone(),
        })?;
        live.ledger_key
            .get_or_try_init(|| async {
                Ok(match device.hardware_serial().await? {
                    Some(serial) => format!("android:{serial}"),
                    None => self.id.clone(),
                })
            })
            .await
            .cloned()
    }

    /// Whether phonedriver installed this app on this device and nobody has installed it again since.
    pub(super) async fn installed_by_us(
        &self,
        device: &dyn Device,
        app: &AppId,
    ) -> Result<bool, Error> {
        if self.ledger.is_under_test(&app.0) {
            return Ok(true);
        }
        let key = self.ledger_key(device).await?;
        let entries = self
            .ledger
            .entries(&key)
            .map_err(|detail| Error::Transport { detail })?;
        let Some(entry) = entries.into_iter().find(|e| e.id == app.0) else {
            return Ok(false);
        };
        Ok(match entry.installed_at {
            Some(then) => device.installed_at(app).await?.as_deref() == Some(then.as_str()),
            None => true,
        })
    }

    pub(super) async fn health(&mut self) -> Result<Value, Error> {
        let device = self.device()?;
        device.tree().await?;
        let frames_ms = match device.frames() {
            None => None,
            Some(mut rx) => {
                rx.mark_unchanged();
                let started = Instant::now();
                match tokio::time::timeout(Duration::from_secs(1), rx.changed()).await {
                    Ok(Ok(())) => Some(started.elapsed().as_millis() as u64),
                    _ => None,
                }
            }
        };
        let info = device.info();
        Ok(
            json!({"id": info.id, "model": info.model, "os": info.os, "frames_ms": frames_ms, "hidden_display": device.hidden_display()}),
        )
    }

    /// With `marks`, boxes every element that takes a tap, a long press or text, labelled with the ref a tap on it
    /// would use. Screenshots are one pixel per point, so element bounds are pixel coordinates.
    pub(super) async fn screenshot(
        &mut self,
        s: &mut Session,
        p: ScreenshotParams,
    ) -> Result<ScreenshotResult, Error> {
        let inline = p.inline;
        self.need(
            Capabilities::SCREENSHOT,
            Op::Screenshot,
            "this device has no screen capture",
        )?;
        let device = self.device()?;
        let now = if p.marks || p.within.is_some() {
            Some(self.fresh(s).await?)
        } else {
            None
        };
        let part = match (&p.within, &now) {
            (Some(target), Some(now)) => {
                let found = s.resolve_read(target, now)?;
                let Some(e) = found.element else {
                    return Err(Error::Refused {
                        detail: "`within` takes an element (a ref or its text), not a point".into(),
                    });
                };
                Some((e.r#ref, e.bounds))
            }
            _ => None,
        };
        let shot = device.screenshot(Quality(100)).await;
        // Observed on GitHub's Linux runner (2026-11-24): the video stream ended, and the reconnect below ran
        // while this handle still held the old control connection open, so the agent kept the lane and
        // answered "lane in use" until the wait for it ran out.
        drop(device);
        let mut image = match shot {
            Err(Error::DeviceGone { .. }) => {
                let display = self.live.as_ref().map(|l| l.display);
                self.live = None;
                self.ensure_live(display.unwrap_or(DisplayMode::Main))
                    .await?;
                self.device()?.screenshot(Quality(100)).await?
            }
            shot => shot?,
        };
        let mut origin = None;
        let part = match part {
            Some((r, b)) => {
                let m = crate::wire::CROP_MARGIN;
                let (x, y) = ((b.x - m).max(0.0).floor(), (b.y - m).max(0.0).floor());
                let (right, bottom) = (
                    (b.x + b.w + m).min(image.width as f32).ceil(),
                    (b.y + b.h + m).min(image.height as f32).ceil(),
                );
                if b.w <= 0.0 || b.h <= 0.0 || right <= x || bottom <= y {
                    return Err(Error::Refused {
                        detail: format!(
                            "@{r} is not on the screen, so there is no picture of it; scroll to it first"
                        ),
                    });
                }
                origin = Some(crate::wire::PointJson { x, y });
                Some((x, y, right - x, bottom - y))
            }
            None => None,
        };
        let mut marked = None;
        if let Some(now) = now.filter(|_| p.marks) {
            let marks: Vec<phonedriver_video::marks::Mark> = now
                .elements
                .iter()
                .filter(|e| {
                    e.actions
                        .intersects(Actions::TAP | Actions::LONG_PRESS | Actions::EDIT)
                        && e.bounds.w > 0.0
                        && e.bounds.h > 0.0
                        && part.is_none_or(|(x, y, w, h)| {
                            let c = e.bounds.center();
                            (x..x + w).contains(&c.x) && (y..y + h).contains(&c.y)
                        })
                })
                .map(|e| phonedriver_video::marks::Mark {
                    label: e.r#ref.to_string(),
                    x: e.bounds.x,
                    y: e.bounds.y,
                    w: e.bounds.w,
                    h: e.bounds.h,
                })
                .collect();
            let png = image.bytes.clone();
            let drawn = tokio::task::spawn_blocking(move || {
                phonedriver_video::marks::draw_marks(&png, &marks).map(|b| (b, marks.len()))
            })
            .await
            .map_err(|e| Error::Transport {
                detail: format!("the marks task failed: {e}"),
            })?
            .map_err(|detail| Error::Transport {
                detail: format!("cannot draw marks: {detail}"),
            })?;
            image.bytes = drawn.0;
            marked = Some(drawn.1 as u32);
        }
        if let Some((x, y, w, h)) = part {
            let png = std::mem::take(&mut image.bytes);
            let (w, h, bytes) = tokio::task::spawn_blocking(move || {
                phonedriver_video::marks::crop(&png, x as u32, y as u32, w as u32, h as u32)
            })
            .await
            .map_err(|e| Error::Transport {
                detail: format!("the crop task failed: {e}"),
            })?
            .map_err(|detail| Error::Transport {
                detail: format!("cannot cut the picture: {detail}"),
            })?;
            (image.width, image.height, image.bytes) = (w, h, bytes);
        }
        let path = self.tmp_file("png");
        path.parent()
            .map_or(Ok(()), crate::private::dir)
            .and_then(|()| crate::private::write(&path, &image.bytes))
            .map_err(|e| Error::Transport {
                detail: format!("cannot write {}: {e}", path.display()),
            })?;
        let bar = self
            .shown
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .and_then(|t| t.status_bar);
        let status_bar = bar
            .zip(self.live.as_ref().map(|l| l.screen().h))
            .filter(|&(_, h)| h > 0.0)
            .map(|(bar, h)| (bar * image.height as f32 / h).ceil() as u32);
        Ok(ScreenshotResult {
            path: path.display().to_string(),
            width: image.width,
            height: image.height,
            scale: 1.0,
            data_b64: inline
                .then(|| base64::engine::general_purpose::STANDARD.encode(&image.bytes)),
            marks: marked,
            status_bar: status_bar.filter(|_| origin.is_none()),
            origin,
        })
    }

    pub(super) fn build(
        &self,
        s: &mut Session,
        tree: &RawTree,
        settle: Option<SettleLine>,
    ) -> Rendered {
        *self.shown.lock().unwrap_or_else(|e| e.into_inner()) = Some(tree.clone());
        let (info, screen, secure_hidden) = match &self.live {
            Some(l) => (
                l.device.info().clone(),
                l.screen(),
                l.device.secure_content_hidden(tree),
            ),
            None => return Rendered::build(Vec::new(), &[], Header::default()),
        };
        let nodes = prune(tree);
        let refs = s.refs.assign(tree, &nodes);
        let alerts = phonedriver_core::alert::find(tree, screen);
        let front = tree
            .windows
            .iter()
            .filter(|w| !alerts.iter().any(|(id, _)| *id == w.id))
            .find(|w| w.kind == "application" && w.package.is_some());
        let (left, right) = front
            .filter(|_| tree.outside.is_empty())
            .map_or((false, false), |w| {
                (w.bounds.x > 1.0, w.bounds.x + w.bounds.w < screen.w - 1.0)
            });
        let app = match tree.outside.first() {
            Some(o) => Some(AppLine {
                id: o.package.clone().unwrap_or_else(|| "unknown".into()),
                name: Some(phonedriver_core::scope::OUTSIDE_NAME.into()),
            }),
            None => front.and_then(|w| w.package.clone()).map(|id| {
                let name = self
                    .names
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(&id)
                    .cloned();
                AppLine { id, name }
            }),
        };
        let keyboard = tree
            .windows
            .iter()
            .find(|w| w.kind == "input_method")
            .map(|w| w.bounds);
        let header = Header {
            device: Some(DeviceLine {
                id: info.id,
                model: info.model,
                os: info.os,
                screen,
                orientation: if screen.w > screen.h {
                    Orientation::Landscape
                } else {
                    Orientation::Portrait
                },
            }),
            app,
            keyboard,
            settle,
            secure_hidden,
            alerts: alerts.into_iter().map(|(_, a)| a).collect(),
            popup: left && right,
            partial: left != right,
        };
        Rendered::build(nodes, &refs, header)
    }
}
