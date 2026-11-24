//! `batch` and `wait`.

use super::*;

impl Actor {
    /// Runs the steps in order, each settled before the next, and stops at the first error. The observation is the
    /// diff from what the agent last saw, so a batch costs one reply however many steps it has.
    pub(super) async fn batch(
        &mut self,
        s: &mut Session,
        p: BatchParams,
    ) -> Result<BatchResult, Error> {
        let baseline = s.last.clone();
        let mut done = Vec::new();
        let mut failed = None;
        let mut unseen_ref = false;
        let mut last: Option<ActionResult> = None;
        let mut typed_into = Vec::new();
        for (i, raw) in p.steps.iter().enumerate() {
            let step = Step::parse(raw).map_err(|_| unreachable_state())?;
            let result = match step {
                Step::Act(action) => {
                    self.act(
                        s,
                        ActParams {
                            action,
                            approval: p.approval.filter(|a| *a == Approval::Autonomous || i == 0),
                            approved: p.approved.clone().filter(|_| i == 0),
                            ask: p.ask.clone(),
                            settle: true,
                            timeout_ms: p.timeout_ms,
                            expect_change_ms: p.expect_change_ms,
                            secret_field_only: false,
                            on: None,
                        },
                    )
                    .await
                }
                Step::Wait {
                    condition,
                    timeout_ms,
                } => {
                    self.wait(
                        s,
                        WaitParams {
                            condition,
                            timeout_ms: timeout_ms.or(p.timeout_ms),
                            expect_change_ms: p.expect_change_ms,
                        },
                    )
                    .await
                }
            };
            match result {
                Ok(r) => {
                    let wrong = r
                        .unverified
                        .clone()
                        .filter(|u| u.wrong && i + 1 < p.steps.len());
                    done.push(StepResult {
                        summary: r.summary.clone(),
                        verified: r.verified,
                        unverified: r.unverified.clone(),
                    });
                    typed_into.extend(r.typed_into);
                    last = Some(r);
                    if let Some(u) = wrong {
                        failed = Some(StepFailure {
                            step: i + 2,
                            error: Error::Refused {
                                detail: format!(
                                    "not run, because the text step {} typed did not land: {}; {}",
                                    i + 1,
                                    u.shows,
                                    u.hint
                                ),
                            }
                            .to_rpc(),
                        });
                        break;
                    }
                }
                Err(e @ Error::DeviceGone { .. }) => return Err(e),
                Err(e) => {
                    unseen_ref = matches!(e, Error::StaleRef { unseen: true, .. });
                    failed = Some(StepFailure {
                        step: i + 1,
                        error: e.to_rpc(),
                    });
                    break;
                }
            }
        }
        let mut result = match (last, failed.is_some()) {
            (Some(r), false) => r,
            _ => {
                let tree = self.device()?.tree().await?;
                let rendered = self.build(s, &tree, None);
                let observation = Observation {
                    header: rendered.header.clone(),
                    text: String::new(),
                    kind: Kind::Full,
                    changed: true,
                    settled: false,
                    settle_ms: 0,
                    still_changing: None,
                    loading: rendered.loading(),
                    elements: None,
                    capture: None,
                    empty: false,
                };
                s.set_last(Some(rendered));
                ActionResult {
                    at: None,
                    typed_into: None,
                    unchanged_repeats: None,
                    crashed: None,
                    ok: true,
                    summary: String::new(),
                    via: None,
                    verified: None,
                    unverified: None,
                    settled: false,
                    settle_ms: 0,
                    changed: true,
                    woke: false,
                    unlocked: false,
                    observation,
                }
            }
        };
        if unseen_ref {
            result.observation.kind = Kind::Full;
            result.observation.text = String::new();
        } else if let (Some(base), Some(now)) = (&baseline, &s.last) {
            match diff(base, now) {
                DiffOrFull::Diff(mut changes) => {
                    for (field, replaced) in typed_into {
                        drop_echo(&mut changes, now, field, replaced);
                    }
                    result.observation.kind = Kind::Diff;
                    result.observation.text = DiffOrFull::Diff(changes).text(now);
                }
                DiffOrFull::Full => {
                    result.observation.kind = Kind::Full;
                    result.observation.text = now.text(&Filters::default()).unwrap_or_default();
                }
            }
        } else if let Some(now) = &s.last {
            result.observation.kind = Kind::Full;
            result.observation.text = now.text(&Filters::default()).unwrap_or_default();
        }
        result.observation.changed = result.observation.kind == Kind::Full
            || !result.observation.text.is_empty()
            || result.changed;
        Ok(BatchResult {
            ok: failed.is_none(),
            steps: done,
            failed,
            settled: result.settled,
            settle_ms: result.settle_ms,
            changed: result.observation.changed,
            observation: result.observation,
        })
    }

    pub(super) async fn wait(
        &mut self,
        s: &mut Session,
        p: WaitParams,
    ) -> Result<ActionResult, Error> {
        let condition = p.condition.clone();
        let mut step = journal::wait(&condition, s);
        let result = match condition {
            WaitCondition::Settled(_) => self.wait_settled(s, p).await,
            WaitCondition::Appears(ref t) => self.wait_for(s, t, true, p.timeout_ms).await,
            WaitCondition::Gone(ref t) => self.wait_for(s, t, false, p.timeout_ms).await,
            WaitCondition::Vpn(state) => self.wait_vpn(s, state, p.timeout_ms).await,
            WaitCondition::Log(ref l) => {
                self.wait_log(s, &l.pattern, l.app.as_deref(), p.timeout_ms)
                    .await
            }
        }?;
        step["summary"] = json!(match condition {
            WaitCondition::Log(_) => "a log line matched".to_string(),
            _ => result.summary.clone(),
        });
        s.journal.push(step);
        Ok(result)
    }

    /// The reply of a wait that is not about the screen: the summary, and the header of the screen as it is now.
    pub(super) async fn waited(
        &mut self,
        s: &mut Session,
        summary: String,
    ) -> Result<ActionResult, Error> {
        let rendered = self.fresh(s).await?;
        Ok(ActionResult {
            at: None,
            typed_into: None,
            unchanged_repeats: None,
            crashed: None,
            ok: true,
            summary,
            via: None,
            verified: None,
            unverified: None,
            settled: false,
            settle_ms: 0,
            changed: false,
            woke: false,
            unlocked: false,
            observation: Observation {
                header: rendered.header.clone(),
                text: String::new(),
                kind: Kind::Diff,
                changed: false,
                settled: false,
                settle_ms: 0,
                still_changing: None,
                loading: false,
                elements: None,
                capture: None,
                empty: false,
            },
        })
    }

    /// Waits until a VPN network is up or down, reading the platform's connectivity state once a second: the shell
    /// has no way to be told when it changes.
    pub(super) async fn wait_vpn(
        &mut self,
        s: &mut Session,
        want: VpnState,
        timeout_ms: Option<u64>,
    ) -> Result<ActionResult, Error> {
        let device = self.device()?;
        let timeout = timeout_ms.map_or(Config::action().timeout, Duration::from_millis);
        let started = Instant::now();
        let up = want == VpnState::Up;
        loop {
            let view = device.network().await?;
            let listed = !view.missing.iter().any(|m| m == "networks");
            if listed && view.vpn_up() == up {
                let owner = view
                    .networks
                    .iter()
                    .find_map(|n| n.owner.clone())
                    .map(|o| format!(" ({o})"))
                    .unwrap_or_default();
                let summary = if up {
                    format!("a VPN is up{owner}")
                } else {
                    "no VPN is up".to_string()
                };
                return self.waited(s, summary).await;
            }
            let left = timeout.saturating_sub(started.elapsed());
            if left.is_zero() {
                return Err(Error::Timeout {
                    waited_ms: started.elapsed().as_millis() as u32,
                    condition: format!(
                        "the VPN to be {}; last seen: {}",
                        if up { "up" } else { "down" },
                        if listed {
                            view.brief()
                        } else {
                            "a connectivity dump that did not list the networks".into()
                        }
                    ),
                });
            }
            tokio::time::sleep(left.min(VPN_POLL)).await;
        }
    }

    /// Waits for a log line written from now on that holds one of the `|`-separated words of `pattern`.
    pub(super) async fn wait_log(
        &mut self,
        s: &mut Session,
        pattern: &str,
        app: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<ActionResult, Error> {
        let device = self.device()?;
        let patterns: Vec<String> = pattern
            .split('|')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect();
        if patterns.is_empty() {
            return Err(Error::Refused {
                detail: "wait log: needs the text to look for in a log line".into(),
            });
        }
        let app = match app {
            Some(name) => Some(self.find_app(&device.apps().await?, name)?.id.clone()),
            None => None,
        };
        let timeout = timeout_ms.map_or(Config::action().timeout, Duration::from_millis);
        let started = Instant::now();
        let got = tokio::time::timeout(
            timeout + Duration::from_secs(5),
            device.log_wait(&LogWait {
                app,
                patterns,
                timeout,
            }),
        )
        .await
        .map_err(|_| Error::Timeout {
            waited_ms: started.elapsed().as_millis() as u32,
            condition: format!("a log line with \"{pattern}\""),
        })??;
        match got.line {
            Some(line) => {
                let line: String = self
                    .secrets
                    .masked(&line)
                    .chars()
                    .take(LOG_LINE_CHARS)
                    .collect();
                self.waited(s, format!("log line: {line}")).await
            }
            None => Err(Error::Timeout {
                waited_ms: started.elapsed().as_millis() as u32,
                condition: format!(
                    "a log line with \"{pattern}\" ({})",
                    match got.seen {
                        0 => "no line arrived".to_string(),
                        1 => "1 line arrived, it did not match".to_string(),
                        n => format!("{n} lines arrived, none matched"),
                    }
                ),
            }),
        }
    }

    /// Waits until the target is on screen (or gone). The tree is read once at the start and then once after each
    /// window change or frame that moved enough pixels, never on a timer: apps' own content-change events are not
    /// subscribed (architecture 4.6), pixels are the signal.
    pub(super) async fn wait_for(
        &mut self,
        s: &mut Session,
        target: &Target,
        present: bool,
        timeout_ms: Option<u64>,
    ) -> Result<ActionResult, Error> {
        let device = self.device()?;
        self.unobserved = None;
        let timeout = timeout_ms.map_or(Config::action().timeout, Duration::from_millis);
        let started = Instant::now();
        let deadline = started + timeout;
        let mut reads = 0u32;
        let mut last_read = false;
        loop {
            let tree = device.tree().await?;
            reads += 1;
            let rendered = self.build(s, &tree, None);
            let found = matches!(
                s.seek(target, &rendered, None, true),
                Ok(_) | Err(Error::Ambiguous { .. })
            );
            if found == present {
                let (kind, text) = answer(s, &rendered);
                let changed = kind == Kind::Full || !text.is_empty();
                let what = describe(target);
                let summary = format!(
                    "{what} {} after {reads} tree read{}",
                    if present { "appeared" } else { "is gone" },
                    if reads == 1 { "" } else { "s" }
                );
                let observation = Observation {
                    header: rendered.header.clone(),
                    text,
                    kind,
                    changed,
                    settled: false,
                    settle_ms: 0,
                    still_changing: None,
                    loading: rendered.loading(),
                    elements: None,
                    capture: None,
                    empty: false,
                };
                s.set_last(Some(rendered));
                return Ok(ActionResult {
                    at: None,
                    typed_into: None,
                    unchanged_repeats: None,
                    crashed: None,
                    ok: true,
                    summary,
                    via: None,
                    verified: None,
                    unverified: None,
                    settled: false,
                    settle_ms: 0,
                    changed,
                    woke: false,
                    unlocked: false,
                    observation,
                });
            }
            if last_read {
                return Err(Error::Timeout {
                    waited_ms: started.elapsed().as_millis() as u32,
                    condition: format!(
                        "{} to {}",
                        describe(target),
                        if present { "appear" } else { "be gone" }
                    ),
                });
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if tokio::time::timeout(left, self.next_change())
                .await
                .is_err()
            {
                last_read = true;
            }
        }
    }

    /// Returns after a window change or a frame with a real change, once the burst that carried it has paused.
    pub(super) async fn next_change(&mut self) {
        loop {
            let sig = next_signal(&mut self.live).await;
            let at = self.now();
            let significant = match self.absorb(sig, at) {
                Some(Input::WindowChanged) => true,
                Some(Input::Frame {
                    changed_area_pt2, ..
                }) => changed_area_pt2 > Config::action().first_change_area,
                None => false,
            };
            if significant {
                tokio::time::sleep(WAIT_DEBOUNCE).await;
                return;
            }
        }
    }

    pub(super) async fn wait_settled(
        &mut self,
        s: &mut Session,
        p: WaitParams,
    ) -> Result<ActionResult, Error> {
        let mut cfg = Config::snapshot();
        let timeout = p
            .timeout_ms
            .map_or(Config::action().timeout, Duration::from_millis);
        cfg.timeout = timeout;
        let (cfg, started) = self.observe(cfg, p.expect_change_ms);
        let st = self.settle(cfg, started).await;
        if let State::Done {
            settled: false,
            elapsed,
            still_changing,
            ..
        } = st
        {
            return Err(Error::NotSettled {
                waited_ms: elapsed.as_millis() as u32,
                still_changing: still_changing.unwrap_or_else(zero),
            });
        }
        let tree = self.device()?.tree().await?;
        let mut rendered = self.build(s, &tree, Some(settle_line(st)));
        self.read_checkboxes(&mut rendered, s.last.as_ref()).await?;
        let (kind, text) = answer(s, &rendered);
        let changed = kind == Kind::Full || !text.is_empty();
        let ms = match st {
            State::Done { elapsed, .. } => elapsed.as_millis() as u32,
            State::Waiting => 0,
        };
        let observation = Observation {
            header: rendered.header.clone(),
            text,
            kind,
            changed,
            settled: true,
            settle_ms: ms,
            still_changing: None,
            loading: rendered.loading(),
            elements: None,
            capture: None,
            empty: false,
        };
        s.set_last(Some(rendered));
        Ok(ActionResult {
            at: None,
            typed_into: None,
            unchanged_repeats: None,
            crashed: None,
            ok: true,
            summary: "screen settled".into(),
            via: None,
            verified: None,
            unverified: None,
            settled: true,
            settle_ms: ms,
            changed,
            woke: false,
            unlocked: false,
            observation,
        })
    }
}
