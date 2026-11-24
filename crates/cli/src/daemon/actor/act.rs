//! Touches: resolving a target, the actions themselves, focus, and scrolling to an element.

use super::*;

impl Actor {
    /// Where a target is now. Only a ref or text target needs the screen, so a point costs no tree read.
    pub(super) async fn resolve_target(
        &mut self,
        s: &mut Session,
        target: &Target,
    ) -> Result<Resolved, Error> {
        if let Target::Point { point } = target {
            let screen = self.screen_rect();
            let inside = (screen.x..=screen.x + screen.w).contains(&point.x)
                && (screen.y..=screen.y + screen.h).contains(&point.y);
            if !inside {
                return Err(Error::PointOffScreen {
                    x: point.x,
                    y: point.y,
                    w: screen.w.round(),
                    h: screen.h.round(),
                });
            }
            return Ok(Resolved {
                at: Point {
                    x: point.x,
                    y: point.y,
                },
                element: None,
                line: None,
                under_keyboard: false,
                covered: None,
                beside_bar: false,
            });
        }
        let now = self.fresh(s).await?;
        s.resolve(target, &now)
    }

    /// Wakes a dark screen, swipes a swipe-only lock screen away, and refuses a PIN lock (architecture 6.3).
    pub(super) async fn prepare(&mut self, action: &Action) -> Result<(bool, bool), Error> {
        if matches!(action, Action::Press { key } if key == "lock") {
            self.awake = None;
            return Ok((false, false));
        }
        if self
            .awake
            .is_some_and(|at| self.power_is_pushed() || self.now().saturating_sub(at) < AWAKE_REUSE)
        {
            return Ok((false, false));
        }
        let device = self.device()?;
        let mut state = device.screen_state().await?;
        let (mut woke, mut unlocked) = (false, false);
        if !state.on {
            device.wake().await?;
            woke = true;
            state = self.until_screen(true, "the screen to turn on").await?;
            // Observed on API 26: the keyguard reports locked a few hundred ms after the screen reports on.
            let started = Instant::now();
            while !state.locked && started.elapsed() < KEYGUARD_WAIT {
                tokio::time::sleep(Duration::from_millis(100)).await;
                state = device.screen_state().await?;
            }
        }
        if state.locked {
            if matches!(action, Action::Press { key } if key == "fingerprint" || key == "wrong-fingerprint")
            {
                return Ok((woke, false));
            }
            // Seen on a HONOR X8 (Android 13, PIN), 2026-11-16: while the phone is locked a hidden display shows
            // the keyguard's clock and starts no app, and a swipe on it unlocks nothing.
            if state.secure || device.hidden_display().is_some() {
                return Err(Error::NeedsUser {
                    action: UserAction::UnlockDevice,
                });
            }
            let screen = self.screen_rect();
            // A drag that starts on a notification card (the middle of the screen on API 26) does not unlock.
            let start = Point {
                x: screen.w / 2.0,
                y: screen.h * 0.9,
            };
            device
                .gesture(gesture::swipe_from(start, screen, Direction::Up, 0.6, true))
                .await?;
            self.until_screen(false, "the lock screen to go away")
                .await?;
            unlocked = true;
        }
        if woke || unlocked {
            self.until_app_content(None).await?;
        }
        self.awake = Some(self.now());
        Ok((woke, unlocked))
    }

    pub(super) async fn until_app_content(&self, leaving: Option<&AppId>) -> Result<(), Error> {
        let device = self.device()?;
        // Observed on API 26 and 37: input sent as soon as the screen is on and unlocked is rejected
        // by the dispatcher until a window has input focus again, and on API 26 the tree lists only
        // system windows, then part of the app, until the app's window is back. A cold-started Settings
        // on API 37 likewise reports an empty tree for a second after its first frame.
        let started = Instant::now();
        let mut last_len = 0;
        let mut equal_reads = 0;
        // Observed on the iOS Simulator: Settings adds a Toolbar group 300 ms to 1 s after its first frame, which
        // made the first refs stale at once, so a polled device must read the same count three times in a row. A
        // device with frames settles on them after this wait and reads again until two reads agree, so one read that
        // shows the app is enough here; a second matching read cost about 200 ms per launch on the emulator.
        let needed = if device.info().capabilities.contains(Capabilities::FRAMES) {
            0
        } else {
            3
        };
        while started.elapsed() < SCREEN_WAIT {
            let tree = device.tree().await?;
            equal_reads = if tree.nodes.len() == last_len {
                equal_reads + 1
            } else {
                0
            };
            let stable = equal_reads >= needed;
            let page_pending = tree
                .nodes
                .iter()
                .enumerate()
                .any(|(i, n)| n.role == Role::WebView && tree.children(i).next().is_none());
            last_len = tree.nodes.len();
            // Observed on the API 37 emulator: for about a second after a cold launch, Chrome's first-run screen had
            // its texts and buttons in the tree but not yet visible, and the launch reply listed only an empty pager.
            let app_is_back = tree.nodes.iter().any(|n| {
                n.flags.contains(NodeFlags::VISIBLE_TO_USER)
                    && (n.label.is_some() || n.value.is_some() || n.actions.contains(Actions::TAP))
                    && tree.windows.iter().any(|w| {
                        w.id == n.window
                            && w.kind == "application"
                            && (started.elapsed() > LEAVE_WAIT
                                || leaving
                                    .is_none_or(|l| w.package.as_deref() != Some(l.0.as_str())))
                    })
            });
            // Observed on the iOS 26.5 Simulator: Maps asked for the location at start, and SpringBoard, the app
            // in front before the launch, held the prompt in front of it, so nothing new came to the front.
            let alert_up = !phonedriver_core::alert::find(&tree, self.screen_size()).is_empty();
            if (app_is_back || alert_up)
                && stable
                && !page_pending
                && device.screen_state().await?.focused
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }

    pub(super) async fn until_screen(
        &self,
        wait_for_on: bool,
        what: &str,
    ) -> Result<ScreenState, Error> {
        let device = self.device()?;
        let started = Instant::now();
        loop {
            let state = device.screen_state().await?;
            let done = if wait_for_on { state.on } else { !state.locked };
            if done {
                return Ok(state);
            }
            if started.elapsed() > SCREEN_WAIT {
                return Err(Error::Timeout {
                    waited_ms: started.elapsed().as_millis() as u32,
                    condition: what.to_string(),
                });
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// A replayed step runs only on the kind of screen it was saved on: the same app, and a dialog in front exactly
    /// when there was one. It reads the screen only when the session has none it is sure of, or when that one
    /// differs: then it reads until the screen is the saved kind or `SCREEN_KIND_WAIT` is over.
    pub(super) async fn on_screen(
        &mut self,
        s: &mut Session,
        saved: &OnScreen,
    ) -> Result<(), Error> {
        let mut differs = match s.last.as_ref().filter(|_| !s.unsure) {
            Some(last) => saved.differs(last),
            None => saved.differs(&self.fresh(s).await?),
        };
        let started = Instant::now();
        while differs.is_some() && started.elapsed() < SCREEN_KIND_WAIT {
            tokio::time::sleep(TREE_STEP).await;
            let tree = self.device()?.tree().await?;
            differs = saved.differs(&self.build(s, &tree, None));
        }
        let Some(differs) = differs else {
            return Ok(());
        };
        let was = saved.app.as_deref().unwrap_or("its app");
        Err(Error::Refused {
            detail: format!("this step was saved on {was}, and {differs}; nothing was touched"),
        })
    }

    pub(super) async fn act(
        &mut self,
        s: &mut Session,
        p: ActParams,
    ) -> Result<ActionResult, Error> {
        s.acted_at = Some(self.now());
        let t = Instant::now();
        let prepared = self.prepare(&p.action).await;
        self.lap("prepare", t);
        let (woke, unlocked) = prepared?;
        if let Some(scope) = self.scope.clone()
            && !matches!(&p.action, Action::Launch { .. } | Action::Open { .. })
            && !matches!(&p.action, Action::Press { key } if matches!(key.as_str(), "back" | "home" | "lock"))
        {
            let outside = self.device()?.tree().await?.outside;
            if !outside.is_empty() {
                return Err(crate::daemon::scope::refusal(
                    &scope,
                    &outside,
                    "this action",
                ));
            }
        }
        if !matches!(
            &p.action,
            Action::Tap {
                target: Target::Point { .. },
                ..
            }
        ) {
            self.guard_view = None;
        }
        let leaves = matches!(&p.action, Action::Launch { .. } | Action::Open { .. });
        let front = s
            .last
            .as_ref()
            .and_then(|last| last.header.app.as_ref())
            .map(|app| app.id.clone());
        let began = Instant::now();
        if let Some(on) = &p.on {
            self.on_screen(s, on).await?;
        }
        let kept = Kept::of(&p.action, s);
        let acted = self.act_now(s, p).await;
        if let Err(Error::NotReached { scrolls, end, .. }) = &acted {
            let moved = match end {
                ScrollEnd::Stuck => 0,
                ScrollEnd::Ended => scrolls.saturating_sub(2),
                ScrollEnd::Limit => *scrolls,
            };
            for step in kept.scrolled(moved) {
                s.journal.push(step);
            }
        }
        if acted.is_err() {
            self.awake = None;
            s.unsure = true;
        }
        let mut result = acted?;
        if let Some(taping) = &mut self.recording {
            taping.step(began, &self.secrets.masked(&result.summary), result.at);
        }
        s.journal.push(kept.step(&result));
        result.woke = woke;
        result.unlocked = unlocked;
        if let Some(app) = front.filter(|_| !leaves && result.observation.kind == Kind::Full) {
            let t = Instant::now();
            let asked = LogRequest {
                app: AppId(app.clone()),
                since: began.elapsed() + CRASH_MARGIN,
                crashes: true,
            };
            if let Ok(reply) = self.device()?.logs(&asked).await
                && let Some(what) = reply.crash
            {
                result.crashed = Some(Crashed { app, what });
            }
            self.lap("crash_log", t);
        }
        s.idle = match s.idle.take() {
            _ if !result.settled || result.changed => None,
            Some((summary, n)) if summary == result.summary => Some((summary, n + 1)),
            _ => Some((result.summary.clone(), 1)),
        };
        result.unchanged_repeats = s.idle.as_ref().map(|(_, n)| *n).filter(|n| *n >= 3);
        Ok(result)
    }

    pub(super) async fn act_now(
        &mut self,
        s: &mut Session,
        p: ActParams,
    ) -> Result<ActionResult, Error> {
        if let Action::Tap {
            target,
            select: Some(value),
            ..
        } = &p.action
        {
            let want = pick::Want::parse(value).map_err(|detail| Error::Refused { detail })?;
            let (approval, approved, ask) = (p.approval, p.approved.clone(), p.ask.clone());
            let timeout_ms = p.timeout_ms;
            let sub = move |action: Action| ActParams {
                action,
                approval,
                approved: approved.clone(),
                ask: ask.clone(),
                settle: true,
                timeout_ms,
                expect_change_ms: None,
                secret_field_only: false,
                on: None,
            };
            let open = sub(Action::Tap {
                target: target.clone(),
                count: None,
                long: None,
                select: None,
            });
            return self.pick(s, open, want, sub).await;
        }
        let mut cfg = Config::action();
        if let Some(t) = p.timeout_ms {
            cfg.timeout = Duration::from_millis(t);
        }
        if let Some(t) = p.expect_change_ms {
            cfg.expect_change = Some(Duration::from_millis(t));
        } else if self.cold_until.is_some_and(|t| self.now() < t) {
            cfg.expect_change = Some(COLD_CHANGE_WAIT);
        }
        let device = self.device()?;
        let consent = Consent {
            approval: p.approval,
            ask: p.ask.clone(),
            approved: p.approved.clone(),
        };
        match p.action {
            Action::Tap {
                target,
                count,
                long,
                select: _,
            } => {
                self.need(
                    Capabilities::GESTURE,
                    Op::Gesture,
                    "this device cannot inject touches",
                )?;
                let Resolved {
                    at,
                    line: element,
                    element: found,
                    under_keyboard,
                    covered,
                    beside_bar,
                } = {
                    let t = Instant::now();
                    let r = self.resolve_target(s, &target).await;
                    self.lap("resolve", t);
                    r?
                };
                // Calendar's "Add" on a Simulator without calendar access: the tap waited 2 s for nothing.
                if let Some(e) = found.as_ref().filter(|e| {
                    !e.state.contains(ElementState::ENABLED)
                        && !matches!(target, Target::Point { .. })
                }) {
                    return Err(Error::Disabled { r#ref: e.r#ref });
                }
                let mut hid = false;
                let (at, element, found, covered, beside_bar) = match (under_keyboard, &found) {
                    (true, Some(e)) => {
                        let behind = Error::OffScreen {
                            r#ref: e.r#ref,
                            scroll_hint: Direction::Down,
                            behind_keyboard: true,
                            covered_by: None,
                        };
                        if !p.settle {
                            return Err(behind);
                        }
                        // Agents answered "under the keyboard" with a hide-keyboard call and the same tap, two calls
                        // for one intent (iOS Contacts, run i2c). The tap does both and finds the element again,
                        // since the form moves when the keyboard goes.
                        let wanted = e.r#ref;
                        let started = self.begin_action();
                        device.hide_keyboard().await?;
                        self.settle_probing(cfg, started, false).await;
                        let now = self.fresh(s).await?;
                        let r = s.resolve(&Target::Ref { r#ref: wanted }, &now)?;
                        if r.under_keyboard
                            || now.header.keyboard.is_some()
                                && r.at.y >= now.header.keyboard.map_or(f32::MAX, |k| k.y)
                        {
                            return Err(behind);
                        }
                        hid = true;
                        (r.at, r.line, r.element, r.covered, r.beside_bar)
                    }
                    _ => (at, element, found, covered, beside_bar),
                };
                if let (Some((bar, scroll_hint)), Some(e), false) = (covered, &found, beside_bar) {
                    return Err(Error::OffScreen {
                        r#ref: e.r#ref,
                        scroll_hint,
                        behind_keyboard: false,
                        covered_by: Some(bar),
                    });
                }
                let approved = self
                    .guard_tap(s, &consent, &target, found.as_ref(), at)
                    .await?;
                let started = self.begin_action();
                let t = Instant::now();
                let sent = self
                    .watching(device.gesture(gesture::tap(at, count.unwrap_or(1), long)))
                    .await;
                self.lap("input", t);
                sent?;
                let verb = match (count, long) {
                    (_, Some(ms)) => format!("long-pressed {ms} ms"),
                    (Some(n), None) if n > 1 => format!("tapped {n} times"),
                    _ => "tapped".to_string(),
                };
                let summary = match &element {
                    Some(line) => format!("{verb} {}", short(line)),
                    None => format!("{verb} at {},{}", at.x.round(), at.y.round()),
                };
                let landed = |r: ActionResult| ActionResult { at: Some(at), ..r };
                let summary = if hid {
                    format!("hid the keyboard, then {summary}")
                } else {
                    summary
                };
                let summary = approved_summary(summary, approved);
                if let Some(e) = found.as_ref().filter(|e| {
                    let can_change = match e.role {
                        Role::Checkbox => true,
                        Role::Radio => !e.state.contains(ElementState::CHECKED),
                        _ => false,
                    };
                    can_change && s.last.as_ref().is_some_and(|l| l.in_web_view(e.r#ref))
                }) {
                    self.await_toggle(s, e.r#ref, e.state.contains(ElementState::CHECKED))
                        .await?;
                }
                let r = self
                    .finish(s, p.settle, cfg, started, summary.clone(), None, None)
                    .await?;
                let ripple_only = r.settled
                    && r.changed
                    && r.observation.kind == Kind::Diff
                    && r.observation.text.is_empty();
                if !p.settle || !ripple_only {
                    return Ok(landed(r));
                }
                // MagicOS Settings draws the button's ripple, goes quiet for longer than the quiet window, and only
                // then starts the next page (Honor X8: the ripple and `no element changed`, then the page).
                let again = self.now();
                let late = Config {
                    expect_change: Some(LATE_PAGE_WAIT),
                    ..cfg
                };
                let (st, probe) = self.settle_probing(late, again, true).await;
                match st {
                    State::Done {
                        settled,
                        changed: true,
                        still_changing,
                        elapsed,
                    } => {
                        let st = State::Done {
                            settled,
                            changed: true,
                            still_changing,
                            elapsed: elapsed + Duration::from_millis(r.settle_ms.into()),
                        };
                        self.report(s, st, probe, summary, None, None)
                            .await
                            .map(landed)
                    }
                    _ => Ok(landed(r)),
                }
            }
            Action::Swipe {
                direction,
                from,
                distance,
                fast,
            } => {
                self.need(
                    Capabilities::GESTURE,
                    Op::Gesture,
                    "this device cannot inject touches",
                )?;
                let screen = self.screen_rect();
                let (start, line, from_element) = match &from {
                    Some(t) => {
                        let r = self.resolve_target(s, t).await?;
                        (Some(r.at), r.line, r.element)
                    }
                    None => (None, None, None),
                };
                let distance = distance.unwrap_or(0.5);
                let g = match start {
                    Some(at) => gesture::swipe_from(at, screen, direction, distance, fast),
                    None => gesture::swipe(screen, direction, distance, fast),
                };
                let approved = self
                    .guard_touch(s, &consent, &g, false, None, from_element.as_ref())
                    .await?;
                let started = self.begin_action();
                self.watching(device.gesture(g)).await?;
                let from_text = line
                    .map(|l| format!(" from {}", short(&l)))
                    .unwrap_or_default();
                let summary =
                    approved_summary(format!("swiped {}{from_text}", direction.name()), approved);
                self.finish(s, p.settle, cfg, started, summary, None, None)
                    .await
            }
            Action::Drag { from, to } => {
                self.need(
                    Capabilities::GESTURE,
                    Op::Gesture,
                    "this device cannot inject touches",
                )?;
                let a = self.resolve_target(s, &from).await?;
                let b = self.resolve_target(s, &to).await?;
                let g = gesture::press_drag(a.at, b.at);
                let approved = self
                    .guard_touch(s, &consent, &g, true, None, a.element.as_ref())
                    .await?;
                let started = self.begin_action();
                self.watching(device.gesture(g)).await?;
                let label = |r: &Resolved| {
                    r.line.as_deref().map_or_else(
                        || format!("{},{}", r.at.x.round(), r.at.y.round()),
                        |l| short(l).to_string(),
                    )
                };
                let summary =
                    approved_summary(format!("dragged {} to {}", label(&a), label(&b)), approved);
                self.finish(s, p.settle, cfg, started, summary, None, None)
                    .await
            }
            Action::Pinch {
                direction,
                at,
                distance,
            } => {
                self.need(
                    Capabilities::MULTI_TOUCH,
                    Op::Gesture,
                    "this device cannot inject two-finger touches",
                )?;
                let screen = self.screen_rect();
                let (center, line, pinched) = match &at {
                    Some(t) => {
                        let r = self.resolve_target(s, t).await?;
                        (r.at, r.line, r.element)
                    }
                    None => (screen.center(), None, None),
                };
                let out = direction == PinchDirection::Out;
                let g = gesture::pinch(center, screen, out, distance.unwrap_or(0.5));
                let approved = self
                    .guard_touch(s, &consent, &g, false, None, pinched.as_ref())
                    .await?;
                let started = self.begin_action();
                device.gesture(g).await?;
                let on = line
                    .map(|l| format!(" on {}", short(&l)))
                    .unwrap_or_default();
                let summary = approved_summary(
                    format!("pinched {}{on}", if out { "out" } else { "in" }),
                    approved,
                );
                self.finish(s, p.settle, cfg, started, summary, None, None)
                    .await
            }
            Action::Gesture { fingers } => {
                self.need(
                    Capabilities::GESTURE,
                    Op::Gesture,
                    "this device cannot inject touches",
                )?;
                let g = Gesture {
                    fingers: fingers
                        .iter()
                        .map(|f| {
                            f.iter()
                                .map(|p| TimedPoint {
                                    at: Point { x: p.x, y: p.y },
                                    t_ms: p.t_ms,
                                })
                                .collect()
                        })
                        .collect(),
                };
                let approved = self.guard_touch(s, &consent, &g, false, None, None).await?;
                let started = self.begin_action();
                self.watching(device.gesture(g)).await?;
                let summary =
                    approved_summary(format!("gestured with {} fingers", fingers.len()), approved);
                self.finish(s, p.settle, cfg, started, summary, None, None)
                    .await
            }
            Action::Launch {
                app,
                grant,
                restart,
            } => {
                self.need(
                    Capabilities::LAUNCH,
                    Op::Launch,
                    "this device cannot launch apps",
                )?;
                let t = Instant::now();
                let found = self.app_named(&*device, &app).await?;
                self.lap("apps", t);
                if !grant.is_empty() {
                    if self.physical() && !self.installed_by_us(&*device, &found.id).await? {
                        return Err(Error::Unsupported {
                            op: Op::Launch,
                            reason: concat!(
                                "phonedriver did not install this app on this physical device, or it was installed again since, so it will not grant it permissions",
                                own_app_hint!()
                            ),
                        });
                    }
                    device.grant(&found.id, &grant).await?;
                }
                cfg.expect_change = p
                    .expect_change_ms
                    .map(Duration::from_millis)
                    .or(Config::launch().expect_change);
                let t = Instant::now();
                let leaving = device.foreground().await?.filter(|f| *f != found.id);
                self.lap("foreground", t);
                let started = self.begin_action();
                if restart {
                    let t = Instant::now();
                    // `simctl terminate` exits 3 ("found nothing to terminate") when the app is not running, and
                    // such an app opens on its first screen anyway.
                    let _ = device.app_op(AppOp::Stop, &found.id).await;
                    self.lap("stop", t);
                }
                let t = Instant::now();
                let started_launch = t;
                if let Err(e) = device.launch(&found.id, restart).await {
                    self.forget_apps();
                    return Err(e);
                }
                self.lap("launch", t);
                let t = Instant::now();
                self.until_app_content(leaving.as_ref()).await?;
                self.lap("app_content", t);
                self.cold_until = Some(self.now() + COLD_APP_WINDOW);
                s.set_last(None);
                let verb = if restart { "restarted" } else { "launched" };
                let mut summary = if grant.is_empty() {
                    format!("{verb} {}", found.name)
                } else {
                    format!("{verb} {} (granted {})", found.name, grant.join(", "))
                };
                // Observed on the iOS 26.5 Simulator: Photos crashed at every launch, the home screen stayed, and the
                // agent read "launched Photos" as a launch that worked and tapped the icon again and again.
                // Observed on the API 37 emulator with two lanes opened at once: a hidden display has no application
                // window for seconds after it is created, and a launch there read an empty screen and said `launched`.
                let empty_counts = device.hidden_display().is_some();
                let elsewhere = |front: &Option<AppId>| match front {
                    Some(f) => *f != found.id,
                    None => empty_counts,
                };
                let names_it = |tree: &RawTree| {
                    phonedriver_core::alert::find(tree, self.screen_size())
                        .iter()
                        .any(|(_, a)| a.title.as_deref().is_some_and(|t| t.contains(&found.name)))
                };
                let mut front = device.foreground().await?;
                let mut prompted = false;
                let late = Instant::now();
                while elsewhere(&front) && late.elapsed() < LATE_START_WAIT {
                    if names_it(&device.tree().await?) {
                        prompted = true;
                        break;
                    }
                    tokio::time::sleep(LATE_START_STEP).await;
                    front = device.foreground().await?;
                    if front.as_ref() == Some(&found.id) {
                        self.until_app_content(leaving.as_ref()).await?;
                    }
                }
                let mut crashed = None;
                if elsewhere(&front) && !prompted {
                    let asked = LogRequest {
                        app: found.id.clone(),
                        since: started_launch.elapsed() + CRASH_MARGIN,
                        crashes: true,
                    };
                    crashed = device.logs(&asked).await.ok().and_then(|r| r.crash);
                    summary = match &crashed {
                        Some(_) => format!(
                            "{summary}, but {} crashed at start (`logs {}` prints its last lines)",
                            found.name, found.id.0
                        ),
                        None => format!(
                            "{summary}, but {} is not in front after {} s ({} is): it is still starting, or it quit at start; `snapshot` shows which",
                            found.name,
                            started_launch.elapsed().as_secs(),
                            front.map_or("nothing".to_string(), |f| f.0)
                        ),
                    };
                }
                cfg.timeout += self.now().saturating_sub(started);
                let mut r = self
                    .finish(s, p.settle, cfg, started, summary, None, None)
                    .await?;
                r.crashed = crashed.map(|what| Crashed {
                    app: found.id.0.clone(),
                    what,
                });
                Ok(r)
            }
            Action::Open { url, app } => {
                self.need(
                    Capabilities::OPEN_URL,
                    Op::OpenUrl,
                    "this device cannot open links",
                )?;
                if self.physical()
                    && consent.approval != Some(Approval::Autonomous)
                    && let Some(action) =
                        shell::intent_action(&url).filter(|a| shell::places_a_call(a))
                {
                    return Err(Error::Refused {
                        detail: format!(
                            "on a physical device an intent: link does not place a call, and this one has {action}, which dials without a tap; open a tel: link, which shows the number in the dialer"
                        ),
                    });
                }
                cfg.expect_change = p
                    .expect_change_ms
                    .map(Duration::from_millis)
                    .or(Config::launch().expect_change);
                let target = match &app {
                    Some(name) => Some(self.find_app(&device.apps().await?, name)?.id.clone()),
                    None => None,
                };
                let leaving = device.foreground().await?;
                let started = self.begin_action();
                let t = Instant::now();
                let handler = device.open_url(&Url(url.clone()), target.as_ref()).await?;
                self.lap("open_url", t);
                let leaving = leaving.filter(|l| handler.as_ref() != Some(l));
                let t = Instant::now();
                self.until_app_content(leaving.as_ref()).await?;
                self.lap("app_content", t);
                let waited = Instant::now();
                while waited.elapsed() < PAGE_WAIT && device.tree().await?.web_view_pending() {
                    tokio::time::sleep(TREE_STEP).await;
                }
                self.lap("page", waited);
                self.cold_until = Some(self.now() + COLD_APP_WINDOW);
                cfg.timeout += self.now().saturating_sub(started);
                let summary = format!("opened {url}");
                self.finish(s, p.settle, cfg, started, summary, None, None)
                    .await
            }
            Action::Scroll {
                direction,
                within,
                until,
                max,
                collect,
            } => {
                self.need(
                    Capabilities::GESTURE,
                    Op::Gesture,
                    "this device cannot inject touches",
                )?;
                if let Some(limit) = collect {
                    if until.is_some() {
                        return Err(Error::Refused {
                            detail: "a scroll takes `until` or `collect`, not both; nothing was scrolled".into(),
                        });
                    }
                    return self
                        .collect(s, &consent, cfg, direction, within, limit, max)
                        .await;
                }
                self.scroll(s, &consent, p.settle, cfg, direction, within, until, max)
                    .await
            }
            Action::Press { key } if key == "hide-keyboard" => {
                let shown = device
                    .tree()
                    .await?
                    .windows
                    .iter()
                    .any(|w| w.kind == "input_method");
                if !shown {
                    let st = State::Done {
                        settled: true,
                        changed: false,
                        still_changing: None,
                        elapsed: Duration::ZERO,
                    };
                    return self
                        .report(s, st, None, "keyboard already hidden".into(), None, None)
                        .await;
                }
                let started = self.begin_action();
                let via = device.hide_keyboard().await?;
                let summary = format!("hid the keyboard via {}", via.unwrap_or("back"));
                let result = self
                    .finish(s, p.settle, cfg, started, summary, via, None)
                    .await?;
                if p.settle && result.observation.header.keyboard.is_some() {
                    return Err(Error::Unsupported {
                        op: Op::Key,
                        reason: "this screen kept the keyboard when it was told to hide",
                    });
                }
                Ok(result)
            }
            Action::Press { key } => {
                let approved = if key == "enter" {
                    self.guard_enter(s, &consent).await?
                } else {
                    None
                };
                let started = self.begin_action();
                let via = self.watching(press(&*device, &key)).await?;
                let summary = match via {
                    Some(how) => format!("pressed {key} via {how}"),
                    None => format!("pressed {key}"),
                };
                let summary = approved_summary(summary, approved);
                self.finish(s, p.settle, cfg, started, summary, via, None)
                    .await
            }
            Action::Clear { target } => {
                self.need(
                    Capabilities::TEXT_SET,
                    Op::Text,
                    "this device cannot set field values",
                )?;
                let r = self.resolve_target(s, &target).await?;
                let secure = r.element.as_ref().is_some_and(is_secure);
                let approved = self
                    .guard_tap(s, &consent, &target, r.element.as_ref(), r.at)
                    .await?;
                let started = self.begin_action();
                device.gesture(gesture::tap(r.at, 1, None)).await?;
                device
                    .text(TextInput::SetValue(String::new()))
                    .await
                    .map_err(|e| withhold(e, secure))?;
                let label = r
                    .line
                    .as_deref()
                    .map(|l| format!(" {}", short(l)))
                    .unwrap_or_default();
                let verify = Verify {
                    field: r.element.map(|e| e.r#ref),
                    expect: Expect::Exactly(String::new()),
                    secure,
                    mode: Some("set"),
                    submitted: false,
                };
                self.finish(
                    s,
                    p.settle,
                    cfg,
                    started,
                    approved_summary(format!("cleared{label}"), approved),
                    Some("set"),
                    Some(verify),
                )
                .await
            }
            Action::Type {
                text,
                into,
                mode,
                secret,
                submit,
                append,
            } => {
                let chosen = mode.is_some();
                let mut brought = String::new();
                let (field, tap_at): (Option<Element>, Option<Point>) = match &into {
                    Some(t) => {
                        let found = match self.resolve_target(s, t).await {
                            Err(Error::Ambiguous { .. }) => {
                                let now = self.fresh(s).await?;
                                s.resolve_field(t, &now)
                            }
                            other => other,
                        };
                        let r = match found {
                            Err(missing @ (Error::StaleRef { .. } | Error::NotFound { .. })) => {
                                let gone = match (&missing, t) {
                                    (Error::StaleRef { r#ref, .. }, _) => s.gone_field(*r#ref),
                                    (_, Target::Text { text, .. }) => s.gone_field_named(text),
                                    _ => None,
                                };
                                let Some(gone) = gone.cloned() else {
                                    return Err(missing);
                                };
                                let (r, said) = self.bring_back(s, cfg, &gone, missing).await?;
                                brought = said;
                                r
                            }
                            r => r?,
                        };
                        let inner = r
                            .element
                            .as_ref()
                            .filter(|e| !e.role.is_text_entry())
                            .and_then(|e| {
                                s.last
                                    .as_ref()
                                    .and_then(|now| text_entry_within(now, e.r#ref))
                            });
                        match inner {
                            Some(f) => {
                                let at = f.bounds.center();
                                (Some(f), Some(at))
                            }
                            None => (r.element, Some(r.at)),
                        }
                    }
                    None => {
                        let focused = |r: &Rendered| {
                            r.elements
                                .iter()
                                .find(|e| {
                                    e.state.contains(ElementState::FOCUSED)
                                        && e.role.is_text_entry()
                                })
                                .cloned()
                        };
                        let known = s.last.as_ref().filter(|_| !s.unsure).and_then(focused);
                        if known.is_some() {
                            (known, None)
                        } else {
                            // Chrome, a tap on Wikipedia's search icon while the page loaded: no field took focus,
                            // and the text was pasted into nothing and reported as typed.
                            let now = self.fresh(s).await?;
                            if now.header.keyboard.is_none() && focused(&now).is_none() {
                                return Err(Error::NotFocused {
                                    fields: now
                                        .elements
                                        .iter()
                                        .filter(|e| e.role.is_text_entry())
                                        .take(NOT_FOCUSED_FIELDS)
                                        .map(|e| now.line(e).trim_start().to_string())
                                        .collect(),
                                });
                            }
                            (focused(&now), None)
                        }
                    }
                };
                // A paste in hidden mode still raises the system's clipboard toast on the phone's own screen ("Settings
                // is accessing the clipboard", HONOR X8, Android 13), so a native field there is set instead. A web
                // field keeps paste: set on a WebView falls back to select-all and paste over what it holds.
                let hidden = self
                    .live
                    .as_ref()
                    .is_some_and(|l| l.display == DisplayMode::Hidden);
                let native = field.as_ref().is_some_and(|f| {
                    f.actions.contains(Actions::EDIT)
                        && !s.last.as_ref().is_some_and(|l| l.in_web_view(f.r#ref))
                });
                let can_set = device.info().capabilities.contains(Capabilities::TEXT_SET);
                let masked = secret || field.as_ref().is_some_and(is_secure);
                let whole = into.is_some() && !append;
                let mode = mode.unwrap_or(if hidden && native && can_set && (whole || !masked) {
                    TypeMode::Set
                } else {
                    TypeMode::Paste
                });
                let (cap, reason) = match mode {
                    TypeMode::Paste => (Capabilities::TEXT_PASTE, "this device cannot paste"),
                    TypeMode::Set => (
                        Capabilities::TEXT_SET,
                        "this device cannot set field values",
                    ),
                    TypeMode::Keys => (Capabilities::TEXT_KEYS, "this device cannot type keys"),
                };
                self.need(cap, Op::Text, reason)?;
                if !secret
                    && !text.is_empty()
                    && let Some((f, kind)) = field
                        .as_ref()
                        .and_then(|f| secret_field::kind(f).map(|k| (f, k)))
                {
                    return Err(Error::SecretField {
                        r#ref: f.r#ref,
                        kind,
                    });
                }
                if p.secret_field_only
                    && !field
                        .as_ref()
                        .is_some_and(|f| is_secure(f) || secret_field::kind(f).is_some())
                {
                    let which = field.as_ref().map_or_else(
                        || "no field has focus".to_string(),
                        |f| format!("@{} is neither", f.r#ref),
                    );
                    return Err(Error::Refused {
                        detail: format!(
                            "a replayed secret is typed only into a password or one-time code field, and {which}; set PHONEDRIVER_REPLAY_SECRET_ANY_FIELD=1 where the replay runs to type it into any field"
                        ),
                    });
                }
                let focus_tap = match (&into, tap_at) {
                    (Some(t), Some(at)) => {
                        self.guard_tap(s, &consent, t, field.as_ref(), at).await?
                    }
                    _ => None,
                };
                let sent = if submit {
                    self.guard_submit(s, &consent, field.as_ref()).await?
                } else {
                    None
                };
                let approved = focus_tap.or(sent);
                let secure = secret || field.as_ref().is_some_and(is_secure);
                let replace = into.is_some() && !append;
                let mut held = field
                    .as_ref()
                    .and_then(|f| f.value.clone())
                    .filter(|v| !v.is_empty());
                let mut wipe = replace && held.is_some() && mode != TypeMode::Set;
                let started = self.begin_action();
                let mut field = field;
                let mut zoomed = false;
                if let Some(at) = tap_at {
                    let at = match (&field, wipe && !can_set) {
                        (Some(f), true) => Point {
                            x: f.bounds.x + f.bounds.w - 4.0,
                            y: at.y,
                        },
                        _ => at,
                    };
                    device.gesture(gesture::tap(at, 1, None)).await?;
                    let in_page = field
                        .as_ref()
                        .is_some_and(|f| s.last.as_ref().is_some_and(|l| l.in_web_view(f.r#ref)));
                    if in_page && let Some(f) = self.follow_focus(s, field.as_ref()).await? {
                        // Chrome on Android zooms too, but reports where elements are and zooms back in on the
                        // field while the keyboard is up, so a pinch there cost about half a second and did not hold.
                        let android = device.info().id.starts_with("android:");
                        zoomed = !android
                            && field.as_ref().is_some_and(|before| {
                                before.r#ref == f.r#ref
                                    && f.bounds.w > before.bounds.w * PAGE_ZOOMED
                            });
                        held = f.value.clone().filter(|v| !v.is_empty());
                        wipe = replace && held.is_some() && mode != TypeMode::Set;
                        field = Some(f);
                    } else if !in_page
                        && device.info().id.starts_with("android:")
                        && let Some(f) = field
                            .as_ref()
                            .filter(|f| !f.state.contains(ElementState::FOCUSED))
                    {
                        self.await_focus(s, f.r#ref).await?;
                    }
                }
                let before = held.clone().unwrap_or_default();
                if let Some(held) = held.filter(|_| wipe) {
                    let emptied = if can_set {
                        TextInput::SetValue(String::new())
                    } else {
                        TextInput::Keystrokes("\u{8}".repeat(held.chars().count()))
                    };
                    device
                        .text(emptied)
                        .await
                        .map_err(|e| withhold(e, secure))?;
                }
                let input = match (mode, secure) {
                    (TypeMode::Paste, true) => TextInput::PasteSecret(text.clone()),
                    (TypeMode::Paste, false) => TextInput::Paste(text.clone()),
                    (TypeMode::Set, true) if replace => TextInput::SetSecret(text.clone()),
                    (TypeMode::Set, true) => TextInput::SetSecret(format!("{before}{text}")),
                    (TypeMode::Set, false) if replace => TextInput::SetValue(text.clone()),
                    (TypeMode::Set, false) => TextInput::SetValue(format!("{before}{text}")),
                    (TypeMode::Keys, _) => TextInput::Keystrokes(text.clone()),
                };
                if !(replace && text.is_empty()) {
                    let t = Instant::now();
                    let typed = self.watching(device.text(input)).await;
                    self.lap("text", t);
                    typed.map_err(|e| withhold(e, secure))?;
                }
                let unzoom = zoomed
                    && device
                        .info()
                        .capabilities
                        .contains(Capabilities::MULTI_TOUCH);
                if unzoom {
                    let screen = self.screen_rect();
                    let above = s
                        .last
                        .as_ref()
                        .and_then(|l| l.header.keyboard)
                        .map_or(screen.center().y, |k| (screen.y + k.y) / 2.0);
                    let center = Point {
                        x: screen.center().x,
                        y: above,
                    };
                    device
                        .gesture(gesture::pinch(center, screen, false, 0.5))
                        .await?;
                }
                if submit {
                    self.watching(device.key(Key::Enter)).await?;
                }
                let what = match &field {
                    Some(f) => format!("into @{}", f.r#ref),
                    None => plural(text.chars().count() as u64, "char"),
                };
                let unzoomed = if unzoom {
                    ", then zoomed the page back out"
                } else {
                    ""
                };
                let summary =
                    approved_summary(format!("{brought}typed {what}{unzoomed}"), approved);
                let expect = if replace {
                    Expect::Exactly(text.clone())
                } else {
                    Expect::Appended {
                        before: field
                            .as_ref()
                            .and_then(|f| f.value.clone())
                            .unwrap_or_default(),
                        text: text.clone(),
                    }
                };
                let field_ref = field.map(|f| f.r#ref);
                let verify = Verify {
                    field: field_ref,
                    expect,
                    secure,
                    mode: Some(mode_name(mode)),
                    submitted: submit,
                };
                let rekey = !chosen
                    && !secure
                    && !submit
                    && replace
                    && !text.is_empty()
                    && field_ref.is_some_and(|r| s.last.as_ref().is_some_and(|l| l.in_web_view(r)));
                let keys = rekey.then(|| text.clone());
                let cfg = if submit || p.expect_change_ms.is_some() {
                    cfg
                } else {
                    Config {
                        expect_change: Some(TYPED_QUIET),
                        quiet: TYPED_QUIET,
                        ..cfg
                    }
                };
                let mut first = self
                    .finish(
                        s,
                        p.settle,
                        cfg,
                        started,
                        summary.clone(),
                        Some(mode_name(mode)),
                        Some(verify.clone()),
                    )
                    .await?;
                // Observed in Clock's alarm name field (API 37, bench task alarm-gym): enter left the field and its
                // keyboard as they were, the reply showed only the text, and the agent took a snapshot and a
                // screenshot to learn that Save was under the keyboard.
                if submit
                    && first.observation.kind != Kind::Full
                    && first.observation.header.keyboard.is_some()
                {
                    first.summary.push_str(KEYBOARD_STAYED);
                }
                let Some(keys) = keys.filter(|_| {
                    first.verified == Some(false) && field_empty(s.last.as_ref(), field_ref)
                }) else {
                    return Ok(first);
                };
                let started = self.begin_action();
                device.text(TextInput::Keystrokes(keys)).await?;
                let verify = Verify {
                    mode: Some("keys"),
                    ..verify
                };
                self.finish(
                    s,
                    p.settle,
                    cfg,
                    started,
                    summary,
                    Some("keys"),
                    Some(verify),
                )
                .await
            }
        }
    }

    /// A tap moves input focus a moment after it is sent. Observed on the testbed's sign-up form (API 37 emulators,
    /// 3 of 6 runs, and twice in benchmark pass 9): the field was emptied, the paste that followed at once went
    /// nowhere, and the reply said "NOT verified: shows no text". With the focus read first, 0 of 8.
    pub(super) async fn await_focus(&mut self, s: &mut Session, field: Ref) -> Result<(), Error> {
        let started = Instant::now();
        while started.elapsed() < FOCUS_WAIT {
            let now = self.fresh(s).await?;
            let waiting = now
                .index_of(field)
                .is_some_and(|i| !now.elements[i].state.contains(ElementState::FOCUSED));
            if !waiting {
                break;
            }
            tokio::time::sleep(FOCUS_STEP).await;
        }
        Ok(())
    }

    /// A page can answer a tap on its search box by building another field and focusing that one, a few hundred ms
    /// to a few seconds later (Wikipedia, where the old field is gone and the keyboard comes up with the new one).
    /// Text pasted before that goes to the old field. Reads the screen until the same field has been focused twice
    /// in a row with the keyboard up, and returns that field as it now reads.
    pub(super) async fn follow_focus(
        &mut self,
        s: &mut Session,
        field: Option<&Element>,
    ) -> Result<Option<Element>, Error> {
        let started = Instant::now();
        let mut at = field.map(|f| f.r#ref);
        let mut held = None;
        let mut steady = 0;
        loop {
            tokio::time::sleep(TREE_STEP).await;
            let now = self.fresh(s).await?;
            let focused = now
                .elements
                .iter()
                .find(|e| e.state.contains(ElementState::FOCUSED) && e.role.is_text_entry())
                .filter(|e| now.in_web_view(e.r#ref));
            match focused {
                Some(e) if Some(e.r#ref) == at => {
                    held = Some((*e).clone());
                    steady += 1;
                }
                Some(e) => {
                    at = Some(e.r#ref);
                    held = Some((*e).clone());
                    steady = 1;
                }
                None => steady = 0,
            }
            let waited = started.elapsed();
            let keyboard = now.header.keyboard.is_some() || waited >= FOCUS_KEYBOARD_WAIT;
            if (steady >= 2 && keyboard) || waited >= FOCUS_FOLLOW_WAIT {
                return Ok(held);
            }
        }
    }

    pub(super) fn screen_size(&self) -> Size {
        self.live
            .as_ref()
            .map_or(Size { w: 0.0, h: 0.0 }, Live::screen)
    }

    pub(super) fn screen_rect(&self) -> Rect {
        let size = self.screen_size();
        Rect {
            x: 0.0,
            y: 0.0,
            w: size.w,
            h: size.h,
        }
    }

    /// Brings back a text field the agent was shown that a later screen dropped: a keyboard that opens shrinks a
    /// form's list, and the fields below it leave the tree (Contacts, run a4). Drags the list toward where the field
    /// was until exactly one field with its role and label shows, and says so for the summary.
    pub(super) async fn bring_back(
        &mut self,
        s: &mut Session,
        cfg: Config,
        gone: &Element,
        stale: Error,
    ) -> Result<(Resolved, String), Error> {
        let device = self.device()?;
        let mut now = self.fresh(s).await?;
        let mut drags = 0u32;
        let mut direction = Direction::Down;
        loop {
            let same: Vec<Ref> = now
                .elements
                .iter()
                .filter(|e| e.role == gone.role && e.label == gone.label)
                .map(|e| e.r#ref)
                .collect();
            if let [r] = same[..] {
                let said = match drags {
                    0 => String::new(),
                    n => format!(
                        "scrolled {} {} to {}, then ",
                        direction.name(),
                        plural(n.into(), "time"),
                        quote(gone.label.as_deref().unwrap_or_default())
                    ),
                };
                return Ok((s.resolve(&Target::Ref { r#ref: r }, &now)?, said));
            }
            let view = above_keyboard(self.screen_rect(), now.header.keyboard);
            direction = if gone.bounds.center().y > view.y + view.h / 2.0 {
                Direction::Down
            } else {
                Direction::Up
            };
            let Some(list) = scroll_container(&now, direction).filter(|_| drags < BRING_BACK_DRAGS)
            else {
                return Err(stale);
            };
            let area = above_keyboard(list.bounds, now.header.keyboard);
            let started = self.begin_action();
            device.gesture(gesture::scroll(area, direction)).await?;
            drags += 1;
            self.settle(cfg, started).await;
            now = self.fresh(s).await?;
        }
    }

    /// Scrolls the list a page at a time and gathers its rows, until `limit` rows, the list's end or `max` scrolls
    /// (decision D116). The end is a list that no longer scrolls that way, or two drags in a row that add no row.
    /// The list is the largest element that scrolls that way: a page whose header collapses scrolls up only, around
    /// the list that scrolls down.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn collect(
        &mut self,
        s: &mut Session,
        consent: &Consent,
        cfg: Config,
        direction: Direction,
        within: Option<Target>,
        limit: u32,
        max: Option<u32>,
    ) -> Result<ActionResult, Error> {
        let device = self.device()?;
        let mut now = self.fresh(s).await?;
        let nothing = || Error::Refused {
            detail: format!(
                "nothing on this screen scrolls {}, so there are no rows to collect; nothing was scrolled",
                direction.name()
            ),
        };
        let way = match direction {
            Direction::Up => Actions::SCROLL_UP,
            Direction::Down => Actions::SCROLL_DOWN,
            Direction::Left => Actions::SCROLL_LEFT,
            Direction::Right => Actions::SCROLL_RIGHT,
        };
        let chosen = match &within {
            Some(t) => s.resolve(t, &now)?.element.map(|e| e.r#ref),
            None => now
                .elements
                .iter()
                .filter(|e| e.actions.contains(way))
                .max_by(|a, b| a.bounds.area().total_cmp(&b.bounds.area()))
                .or_else(|| scroll_container(&now, direction))
                .map(|e| e.r#ref),
        }
        .ok_or_else(nothing)?;
        let forward = matches!(direction, Direction::Down | Direction::Right);
        let limit = limit.max(1) as usize;
        let max = max.unwrap_or(DEFAULT_SCROLLS);
        let mut all = Vec::new();
        let (mut scrolls, mut idle, mut last) = (0u32, 0u32, None);
        let why = loop {
            let holder = now
                .index_of(chosen)
                .or_else(|| scroll_container(&now, direction).and_then(|e| now.index_of(e.r#ref)))
                .ok_or_else(nothing)?;
            let added = collect::merge(&mut all, collect::rows(&now, holder), forward);
            idle = if scrolls > 0 && added == 0 {
                idle + 1
            } else {
                0
            };
            if all.len() >= limit {
                break format!("stopped at the limit of {limit}");
            }
            if !now.elements[holder].actions.contains(way) || idle >= 2 {
                break "the list ended".to_string();
            }
            if scrolls >= max {
                break format!("stopped after {}", plural(max.into(), "scroll"));
            }
            let area = above_keyboard(now.elements[holder].bounds, now.header.keyboard);
            let g = gesture::scroll(area, direction);
            self.guard_touch(s, consent, &g, false, Some(&now), None)
                .await?;
            let started = self.begin_action();
            device.gesture(g).await?;
            scrolls += 1;
            last = Some(self.settle(cfg, started).await);
            now = self.fresh(s).await?;
        };
        if forward {
            all.truncate(limit);
        } else {
            all.drain(..all.len().saturating_sub(limit));
        }
        let summary = format!(
            "collected {} in {} {}: {why}",
            plural(all.len() as u64, "row"),
            plural(scrolls.into(), "scroll"),
            direction.name()
        );
        let st = last.unwrap_or(State::Done {
            settled: false,
            changed: false,
            still_changing: None,
            elapsed: Duration::ZERO,
        });
        let mut done = self.report(s, st, None, summary, None, None).await?;
        done.rows = all.into_iter().map(|r| r.text).collect();
        Ok(done)
    }

    /// One drag that stops under the finger, or with `until` drags until the text shows (architecture 10.1). The
    /// end of the content is two drags in a row that change nothing.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn scroll(
        &mut self,
        s: &mut Session,
        consent: &Consent,
        settle: bool,
        cfg: Config,
        direction: Direction,
        within: Option<Target>,
        until: Option<Target>,
        max: Option<u32>,
    ) -> Result<ActionResult, Error> {
        let device = self.device()?;
        let t = Instant::now();
        let mut now = self.fresh(s).await?;
        self.lap("fresh", t);
        let holder = match &within {
            Some(t) => s.resolve(t, &now)?.element,
            None => scroll_container(&now, direction).cloned(),
        };
        let area = holder
            .as_ref()
            .map_or_else(|| self.screen_rect(), |e| e.bounds);
        let area = above_keyboard(area, now.header.keyboard);
        let holder = holder.map(|e| e.r#ref);
        let first = gesture::scroll(area, direction);
        let approved = self
            .guard_touch(s, consent, &first, false, Some(&now), None)
            .await?;
        let Some(until) = until else {
            let started = self.begin_action();
            let t = Instant::now();
            device.gesture(first).await?;
            self.lap("gesture", t);
            let summary = approved_summary(format!("scrolled {}", direction.name()), approved);
            return self
                .finish(s, settle, cfg, started, summary, None, None)
                .await;
        };
        let mut gestures = 0u32;
        let mut idle = 0u32;
        let mut moved_any = false;
        let mut last = None;
        let may_fling = device.info().capabilities.contains(Capabilities::FRAMES);
        let max = max.unwrap_or_else(|| default_scroll_limit(&until, &now, direction));
        let mut sorted = true;
        loop {
            match s.seek(&until, &now, holder, gestures > 0) {
                Ok(found) if found.covered.is_none() || idle >= 2 => {
                    let line = found.line.unwrap_or_default();
                    let summary = if gestures == 0 {
                        format!("already on screen, no scroll: {}", short(&line))
                    } else {
                        format!(
                            "scrolled {} {} to {}",
                            direction.name(),
                            plural(gestures.into(), "time"),
                            short(&line)
                        )
                    };
                    let st = match last {
                        Some(State::Done {
                            settled,
                            still_changing,
                            elapsed,
                            ..
                        }) => State::Done {
                            settled,
                            changed: true,
                            still_changing,
                            elapsed,
                        },
                        _ => State::Done {
                            settled: false,
                            changed: false,
                            still_changing: None,
                            elapsed: Duration::ZERO,
                        },
                    };
                    return self.report(s, st, None, summary, None, None).await;
                }
                Ok(_) | Err(Error::NotFound { .. } | Error::StaleRef { .. }) => {}
                Err(Error::Ambiguous { query, matches }) => {
                    return Err(Error::Ambiguous {
                        query,
                        matches: matches
                            .into_iter()
                            .map(|m| match m.split_once(' ') {
                                Some((r, rest)) if r.starts_with('@') => rest.to_string(),
                                _ => m,
                            })
                            .collect(),
                    });
                }
                Err(e) => return Err(e),
            }
            let query = || match &until {
                Target::Text { text, .. }
                | Target::Id {
                    text: Some(text), ..
                } => text.clone(),
                Target::Id { id, .. } => id.clone(),
                Target::Ref { r#ref } => format!("@{}", r#ref),
                Target::Point { .. } => "the target".into(),
            };
            if idle >= 2 {
                return Err(Error::NotReached {
                    query: query(),
                    scrolls: gestures,
                    end: if moved_any {
                        ScrollEnd::Ended
                    } else {
                        ScrollEnd::Stuck
                    },
                    last: moved_any
                        .then(|| last_label(&now, area, direction))
                        .flatten(),
                });
            }
            if gestures >= max {
                return Err(Error::NotReached {
                    query: query(),
                    scrolls: gestures,
                    end: ScrollEnd::Limit,
                    last: None,
                });
            }
            let started = self.begin_action();
            let (dir, far) = match numbered_distance(&until, &now, direction) {
                Some((behind, rows, span)) => {
                    let dir = if behind {
                        direction.opposite()
                    } else {
                        direction
                    };
                    (dir, rows > 3 * span)
                }
                None => match sorted_distance(&until, &now, direction, holder).filter(|_| sorted) {
                    Some((behind, far)) if behind => (direction.opposite(), far),
                    Some((_, far)) => (direction, far),
                    None => {
                        // The name falls between two rows on screen (or the rows are not in order): from here
                        // the search goes one way, so a name the list lacks cannot send it back and forth.
                        sorted = false;
                        (direction, false)
                    }
                },
            };
            // A fling moves nothing on the iOS runner, so a device without a frame stream scrolls one drag at a time.
            let g = if far && may_fling {
                gesture::fling(area, dir)
            } else {
                gesture::scroll(area, dir)
            };
            self.guard_touch(s, consent, &g, false, Some(&now), None)
                .await?;
            device.gesture(g).await?;
            gestures += 1;
            last = Some(self.settle(cfg, started).await);
            let next = self.fresh(s).await?;
            let moved = !matches!(diff(&now, &next), DiffOrFull::Diff(c) if c.is_empty());
            idle = if moved { 0 } else { idle + 1 };
            moved_any |= moved;
            now = next;
        }
    }
}
