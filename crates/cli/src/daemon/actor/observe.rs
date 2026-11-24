//! Reading the screen: the start of an action, settling, snapshots and the reads that fill a snapshot in.

use super::*;

impl Actor {
    /// Starts an action's clock. What the device reported before it is not the action's doing, but a rotation or a
    /// lost connection still holds afterwards, so those are kept.
    pub(super) fn begin_action(&mut self) -> Duration {
        self.during.clear();
        let mut gone = false;
        if let Some(l) = &mut self.live {
            l.frames.borrow_and_update();
            while let Ok(event) = l.events.try_recv() {
                match event {
                    DeviceEvent::Locked(_) => self.awake = None,
                    DeviceEvent::DisplayChanged { screen, .. } => {
                        l.screen = screen;
                        self.awake = None;
                    }
                    DeviceEvent::Gone | DeviceEvent::DensityChanged => gone = true,
                    DeviceEvent::WindowChanged { .. } => {}
                }
            }
        }
        if gone {
            self.live = None;
            self.awake = None;
        }
        self.now()
    }

    /// Awaits an input while following the screen, so a change it makes before it returns counts from when it showed.
    /// XCTest confirms typed text about 220 ms after the app shows it (iPhone 17 Simulator, iOS 26.5), and the settle
    /// that followed first saw the change then and waited out its quiet window again.
    pub(super) async fn watching<T>(&mut self, input: impl std::future::Future<Output = T>) -> T {
        let mut input = std::pin::pin!(input);
        loop {
            tokio::select! {
                biased;
                out = &mut input => return out,
                sig = next_signal(&mut self.live) => {
                    let at = self.now();
                    if let Some(seen) = self.absorb(sig, at) {
                        self.during.push((at, seen));
                    }
                }
            }
        }
    }

    pub(super) async fn settle(&mut self, cfg: Config, started: Duration) -> State {
        self.settle_probing(cfg, started, false).await.0
    }

    /// With `probe`, reads the tree while the screen is quiet but the quiet period is not over, so a reply that needs
    /// the tree after the settle finds it already read. Two equal reads that both started after the last significant
    /// frame are what `stabilize` would have gathered after the settle.
    pub(super) async fn settle_probing(
        &mut self,
        cfg: Config,
        started: Duration,
        probe: bool,
    ) -> (State, Option<Probe>) {
        let mut det = Detector::new(cfg, started);
        if let Some((t, input)) = self.recent
            && (cfg.expect_change.is_none() || t >= started)
        {
            det.feed(input, t);
        }
        for (t, input) in std::mem::take(&mut self.during) {
            det.feed(input, t.max(started));
        }
        if std::mem::take(&mut self.change_seen) {
            det.feed(Input::WindowChanged, started.max(self.now()));
        }
        let device = if probe { self.device().ok() } else { None };
        let mut reads: Vec<Read> = Vec::new();
        let mut inflight: Option<Inflight> = None;
        let mut probed_at = Duration::ZERO;
        loop {
            let st = det.poll(self.now());
            if matches!(st, State::Done { .. }) {
                let last = det.last_significant();
                if let (Some((at, read)), Some(last)) = (inflight.take(), last)
                    && at >= last + READ_LAG
                    && let Ok(tree) = read.await
                {
                    reads.push(Read { at, tree });
                }
                let settled = matches!(
                    st,
                    State::Done {
                        settled: true,
                        changed: true,
                        ..
                    }
                );
                let probed = last.filter(|_| settled).and_then(|l| probe_of(reads, l));
                return (st, probed);
            }
            if let (Some(device), None) = (&device, &inflight) {
                let now = self.now();
                let wanted = match det.last_significant() {
                    Some(last) => {
                        let due = last + READ_LAG;
                        now >= due && reads.iter().filter(|r| r.at >= due).count() < 2
                    }
                    None => {
                        cfg.expect_change
                            .is_some_and(|w| w >= FIRST_READ_MIN_WINDOW)
                            && now >= started + FIRST_READ
                            && now >= probed_at + FIRST_READ
                    }
                };
                if wanted {
                    let device = device.clone();
                    inflight = Some((now, Box::pin(async move { device.tree().await })));
                }
            }
            tokio::select! {
                sig = next_signal(&mut self.live) => {
                    let at = self.now();
                    if let Some(input) = self.absorb(sig, at) {
                        det.feed(input, at);
                    }
                }
                (at, read) = finished(&mut inflight) => {
                    if let Ok(tree) = read {
                        if det.last_significant().is_none() {
                            probed_at = self.now();
                            // A first call (a launch right after connecting) has no screen to compare against, and
                            // a polled device has no frames, so the read itself is the change.
                            if self.shown().is_none_or(|shown| shown != tree) {
                                det.feed(Input::WindowChanged, probed_at);
                            }
                        } else {
                            reads.push(Read { at, tree });
                        }
                    }
                }
                () = tokio::time::sleep(TICK) => {}
            }
        }
    }

    pub(super) async fn snapshot(
        &mut self,
        s: &mut Session,
        p: SnapshotParams,
    ) -> Result<Observation, Error> {
        let device = self.device()?;
        let settle = if p.no_settle {
            None
        } else {
            let mut cfg = Config::snapshot();
            if let Some(t) = p.timeout_ms {
                cfg.timeout = Duration::from_millis(t);
            }
            let (cfg, started) = self.observe(cfg, None);
            let t = Instant::now();
            let st = self.settle(cfg, started).await;
            self.lap("settle", t);
            Some(st)
        };
        let t = Instant::now();
        let mut tree = device.tree().await?;
        // Run a7: a snapshot right after `shell am start` read the screen between two activities and printed a
        // header with no lines.
        let empty_since = Instant::now();
        while settle.is_some()
            && !tree
                .nodes
                .iter()
                .any(|n| n.label.is_some() || !n.actions.is_empty())
            && empty_since.elapsed() < CONTENT_WAIT
        {
            tokio::time::sleep(TREE_STEP).await;
            tree = device.tree().await?;
        }
        self.lap("tree", t);
        let t = Instant::now();
        let line = settle.map(settle_line);
        let mut rendered = self.build(s, &tree, line);
        self.lap("render", t);
        s.ocr = false;
        if p.pixels || tree_is_hollow(&rendered) {
            match self.merge_pixels(s, &mut rendered).await {
                Ok(()) => s.ocr = true,
                Err(e) if p.pixels => return Err(e),
                Err(_) => {}
            }
        }
        self.read_checkboxes(&mut rendered, s.last.as_ref()).await?;
        let filters = Filters {
            interactive: p.interactive,
            within: p.within,
            max_lines: p.max_lines,
        };
        let text = if p.diff && s.last.is_some() {
            match s.diff_for_reply(&rendered).ok_or_else(unreachable_state)? {
                DiffOrFull::Diff(changes) => {
                    (Kind::Diff, DiffOrFull::Diff(changes).text(&rendered))
                }
                DiffOrFull::Full => (
                    Kind::Full,
                    rendered.text(&filters).map_err(|r| stale(r, s))?,
                ),
            }
        } else {
            (
                Kind::Full,
                rendered.text(&filters).map_err(|r| stale(r, s))?,
            )
        };
        let mut text = text;
        if p.audit {
            text.1.push_str(&audited(&rendered));
        }
        let capture = if p.capture {
            device.capture_tree().await?
        } else {
            None
        };
        let (settled, ms, still) = match settle {
            Some(State::Done {
                settled,
                elapsed,
                still_changing,
                ..
            }) => (settled, elapsed.as_millis() as u32, still_changing),
            _ => (false, 0, None),
        };
        let obs = Observation {
            header: rendered.header.clone(),
            text: text.1,
            kind: text.0,
            changed: true,
            settled,
            settle_ms: ms,
            still_changing: still,
            loading: rendered.loading(),
            elements: p.elements.then(|| rendered.elements.clone()),
            capture,
            empty: false,
        };
        s.set_last(Some(rendered));
        Ok(obs)
    }

    /// Fetches the screen as it is now for resolving a target. It does not replace what the agent was last shown.
    pub(super) async fn fresh(&mut self, s: &mut Session) -> Result<Rendered, Error> {
        let tree = self.device()?.tree().await?;
        let mut rendered = self.build(s, &tree, None);
        if s.ocr {
            self.merge_pixels(s, &mut rendered).await?;
        }
        self.read_checkboxes(&mut rendered, s.last.as_ref()).await?;
        self.guard_view = Some((self.now(), rendered.clone()));
        Ok(rendered)
    }

    /// An action that found no element for its target answers with the screen as it is, against the one the agent
    /// last saw, so the agent need not ask for it (issue #37: the turn that fetched it cost about 8.5k tokens). The
    /// screen is the one the target was looked for on, and the session holds it from here on.
    pub(super) async fn with_screen(&mut self, s: &mut Session, mut e: Error) -> Error {
        let (Error::StaleRef {
            unseen: false,
            screen,
            ..
        }
        | Error::NotFound { screen, .. }) = &mut e
        else {
            return e;
        };
        let now = match self.guard_view.take() {
            Some((_, seen)) => seen,
            None => match self.fresh(s).await {
                Ok(seen) => seen,
                Err(_) => return e,
            },
        };
        *screen = Some(match s.diff_for_reply(&now) {
            Some(DiffOrFull::Diff(changes)) => DiffOrFull::Diff(changes).text(&now),
            _ => now.text(&Filters::default()).unwrap_or_default(),
        });
        s.set_last(Some(now));
        e
    }

    /// A web checkbox or radio repaints after the screen has looked settled (up to 3 s on a page that has just loaded in the
    /// emulator), so a read right away still shows the old state and the reply says nothing changed.
    pub(super) async fn await_toggle(
        &mut self,
        s: &mut Session,
        r: Ref,
        was: bool,
    ) -> Result<(), Error> {
        let started = Instant::now();
        while started.elapsed() < TOGGLE_WAIT {
            tokio::time::sleep(TREE_STEP).await;
            let now = self.fresh(s).await?;
            let state = now.index_of(r).map(|i| now.elements[i].state);
            if state.is_none_or(|st| st.contains(ElementState::CHECKED) != was) {
                // Observed on the iOS Simulator: the settle that follows compares against the tree read here, saw no
                // first change, and waited its whole window (2,006 ms within 30 s of an `open`, 500 ms later).
                self.change_seen = true;
                break;
            }
        }
        Ok(())
    }

    /// A web checkbox or radio says neither checkable nor checked in the tree, so its state is read from the screenshot.
    pub(super) async fn read_checkboxes(
        &self,
        rendered: &mut Rendered,
        prev: Option<&Rendered>,
    ) -> Result<(), Error> {
        let screen = self.screen_rect();
        let keyboard = rendered.header.keyboard;
        let unread: Vec<usize> = rendered
            .elements
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                matches!(e.role, Role::Checkbox | Role::Radio)
                    && !e.state.contains(ElementState::CHECKABLE)
                    && !checkbox::readable(e.bounds, screen, keyboard)
                    && rendered.in_web_view(e.r#ref)
            })
            .map(|(i, _)| i)
            .collect();
        for i in unread {
            let known = prev
                .and_then(|p| {
                    p.index_of(rendered.elements[i].r#ref)
                        .map(|j| p.elements[j].state)
                })
                .filter(|st| st.contains(ElementState::CHECKABLE));
            if let Some(st) = known {
                let state = &mut rendered.elements[i].state;
                state.insert(ElementState::CHECKABLE);
                state.set(ElementState::CHECKED, st.contains(ElementState::CHECKED));
            }
        }
        let boxes: Vec<usize> = rendered
            .elements
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                matches!(e.role, Role::Checkbox | Role::Radio)
                    && !e.state.contains(ElementState::CHECKABLE)
                    && e.bounds.w > 0.0
                    && e.bounds.h > 0.0
                    && checkbox::readable(e.bounds, screen, keyboard)
                    && rendered.in_web_view(e.r#ref)
            })
            .map(|(i, _)| i)
            .take(MAX_CHECKBOXES)
            .collect();
        let device = self.device()?;
        if boxes.is_empty()
            || !device
                .info()
                .capabilities
                .contains(Capabilities::SCREENSHOT)
        {
            return Ok(());
        }
        let image = device.screenshot(Quality(100)).await?;
        let rects: Vec<Rect> = boxes.iter().map(|&i| rendered.elements[i].bounds).collect();
        let ticked = tokio::task::spawn_blocking(move || checkbox::ticked(&image.bytes, &rects))
            .await
            .map_err(|e| Error::Transport {
                detail: format!("checkbox task failed: {e}"),
            })??;
        for (&i, on) in boxes.iter().zip(ticked) {
            let state = &mut rendered.elements[i].state;
            state.insert(ElementState::CHECKABLE);
            state.set(ElementState::CHECKED, on);
        }
        Ok(())
    }

    /// Adds the text a screenshot shows and the tree does not. A box keeps the ref of the same text in the last
    /// snapshot, so a ref the agent was shown still names it.
    pub(super) async fn merge_pixels(
        &mut self,
        s: &mut Session,
        rendered: &mut Rendered,
    ) -> Result<(), Error> {
        let Some(screen) = rendered.header.device.as_ref().map(|d| d.screen) else {
            return Ok(());
        };
        let image = self.device()?.screenshot(Quality(100)).await?;
        let boxes =
            tokio::task::spawn_blocking(move || crate::ocr::recognize(&image.bytes, screen))
                .await
                .map_err(|e| Error::Transport {
                    detail: format!("OCR task failed: {e}"),
                })??;
        let mut taken = Vec::new();
        let last: Vec<(Ref, String)> = s
            .last
            .iter()
            .flat_map(|l| l.elements.iter())
            .filter(|e| e.source == Source::Ocr)
            .map(|e| (e.r#ref, e.label.clone().unwrap_or_default()))
            .collect();
        merge(rendered, boxes, |b| {
            match last
                .iter()
                .find(|(r, text)| *text == b.text && !taken.contains(r))
            {
                Some((r, _)) => {
                    taken.push(*r);
                    *r
                }
                None => s.refs.fresh(),
            }
        });
        Ok(())
    }
}
