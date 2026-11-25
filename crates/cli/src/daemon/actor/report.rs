//! The reply after an action: the reads that wait for the screen to be worth reporting, and the diff.

use super::*;

impl Actor {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn finish(
        &mut self,
        s: &mut Session,
        settle: bool,
        cfg: Config,
        started: Duration,
        summary: String,
        via: Option<&'static str>,
        verify: Option<Verify>,
    ) -> Result<ActionResult, Error> {
        if !settle {
            self.unobserved = Some(started);
            return Ok(delivered(s, summary, via));
        }
        self.unobserved = None;
        let started = if self.device()?.frames_follow() {
            started
        } else {
            self.now()
        };
        let t = Instant::now();
        // Typed text is checked on the reply's tree, so a `type` waits for two whole reads to agree as it did. A
        // device that sends no frames cannot say when the screen last drew.
        let frames = self
            .device()?
            .info()
            .capabilities
            .contains(Capabilities::FRAMES);
        let next = self.ready.take().filter(|_| verify.is_none() && frames);
        let ready = next
            .as_ref()
            .map(|(next, arrives)| (&mut *s, next, *arrives));
        let (st, probe) = self.settle_probing(cfg, started, true, ready).await;
        self.lap("settle", t);
        let next = next.as_ref().map(|(next, _)| next);
        self.report(s, st, probe, summary, via, verify, next).await
    }

    /// The accessibility tree can lag the pixels: a dialog or an expanding card appears a moment after the screen went
    /// quiet, and a page keeps loading behind a progress bar. Reads again until two reads agree and nothing is loading.
    ///
    /// With `on`, two reads in a row that agree on the next step's target (`Actor::place`) end it too, and the answer
    /// says so (decision D127).
    pub(super) async fn stabilize(
        &self,
        mut tree: RawTree,
        read_at: Option<Duration>,
        until: Instant,
        mut on: Option<(&mut Session, &crate::wire::Next)>,
    ) -> Result<(RawTree, bool), Error> {
        let started = Instant::now();
        let mut held = self.held(&mut on, &tree);
        let mut wait = read_at.map_or(TREE_STEP, |at| {
            TREE_STEP.saturating_sub(self.now().saturating_sub(at))
        });
        loop {
            tokio::time::sleep(wait).await;
            wait = TREE_STEP;
            let next = self.device()?.tree().await?;
            let same = next == tree;
            // Observed in Files on the API 37 emulator, 1 cold open of the storage root in 6: cards were still being
            // added 1.2 s after the tap, and the reply listed 6 of 17. A clock or a counter changes without growing.
            let growing = next.nodes.len() > tree.nodes.len();
            tree = next;
            let before = held.take();
            held = self.held(&mut on, &tree);
            if before.is_some() && before == held {
                return Ok((tree, true));
            }
            let limit = if tree.web_view_pending() {
                WEB_VIEW_WAIT
            } else if tree.loading() || growing {
                LOADING_WAIT
            } else {
                TREE_STABLE_WAIT
            };
            if (same && !tree.loading()) || started.elapsed() >= limit || Instant::now() >= until {
                return Ok((tree, false));
            }
        }
    }

    fn held(
        &self,
        on: &mut Option<(&mut Session, &crate::wire::Next)>,
        tree: &RawTree,
    ) -> Option<(Rect, usize)> {
        let (s, next) = on.as_mut()?;
        self.place(s, next, tree)
            .filter(|(_, holds)| *holds)
            .map(|(bounds, _)| (bounds, tree.nodes.len()))
    }

    /// Reads the screen after a settle and answers with what changed since the agent last saw it.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn report(
        &mut self,
        s: &mut Session,
        st: State,
        probe: Option<Probe>,
        mut summary: String,
        via: Option<&'static str>,
        verify: Option<Verify>,
        next: Option<&crate::wire::Next>,
    ) -> Result<ActionResult, Error> {
        let t = Instant::now();
        let until = t + REPORT_WAIT;
        let open = || Instant::now() < until;
        let mut ready = probe.as_ref().is_some_and(|p| p.ready);
        let (mut tree, agreed, read_at) = match probe {
            Some(p) => (p.tree, p.stable, Some(p.at)),
            None => (self.device()?.tree().await?, false, None),
        };
        self.lap("tree", t);
        let agreed = ready || (agreed && !tree.loading() && !tree.web_view_pending());
        if !agreed
            && (matches!(st, State::Done { changed: true, .. })
                || verify.is_some()
                || tree.loading())
        {
            let t = Instant::now();
            let on = next.map(|next| (&mut *s, next));
            (tree, ready) = self.stabilize(tree, read_at, until, on).await?;
            self.lap("stabilize", t);
        }
        let render = Instant::now();
        let st = Some(st);
        let mut rendered = self.build(s, &tree, None);
        if !ready && only_lost_lines(s, &rendered) {
            let started = Instant::now();
            while started.elapsed() < CONTENT_WAIT && open() {
                tokio::time::sleep(TREE_STEP).await;
                let tree = self.device()?.tree().await?;
                rendered = self.build(s, &tree, None);
                if !only_lost_lines(s, &rendered) {
                    break;
                }
            }
        }
        let empty_since = Instant::now();
        while rendered.elements.is_empty() && empty_since.elapsed() < CONTENT_WAIT && open() {
            tokio::time::sleep(TREE_STEP).await;
            let tree = self.device()?.tree().await?;
            rendered = self.build(s, &tree, None);
        }
        let arrived = s
            .last
            .as_ref()
            .is_some_and(|prev| matches!(diff(prev, &rendered), DiffOrFull::Full));
        let bare_since = Instant::now();
        let mut filled = None;
        while !ready && arrived && bare(&rendered) {
            let limit = if rendered.loading() {
                SPINNER_WAIT
            } else {
                BARE_WAIT
            };
            if bare_since.elapsed() >= limit || !open() {
                break;
            }
            tokio::time::sleep(TREE_STEP).await;
            let tree = self.device()?.tree().await?;
            rendered = self.build(s, &tree, None);
            filled = Some(tree);
        }
        if let Some(tree) = filled.filter(|_| !bare(&rendered)) {
            let tree = self.stabilize(tree, None, until, None).await?.0;
            rendered = self.build(s, &tree, None);
        }
        self.read_checkboxes(&mut rendered, s.last.as_ref()).await?;
        let judge = |s: &Session, rendered: &Rendered| {
            verify
                .as_ref()
                .map(|v| v.check(rendered, v.submitted && moved_beyond(s, rendered, v.field)))
        };
        let mut verdict = judge(s, &rendered);
        let in_page = verify
            .as_ref()
            .and_then(|v| v.field)
            .is_some_and(|r| rendered.in_web_view(r));
        let waited = Instant::now();
        while in_page
            && matches!(verdict, Some(Verdict::Failed(_)))
            && waited.elapsed() < WEB_TEXT_WAIT
            && open()
        {
            tokio::time::sleep(TREE_STEP).await;
            let tree = self.device()?.tree().await?;
            rendered = self.build(s, &tree, None);
            self.read_checkboxes(&mut rendered, s.last.as_ref()).await?;
            verdict = judge(s, &rendered);
        }
        if let (Some(v), Some(Verdict::Landed)) = (&verify, &verdict)
            && !v.submitted
            && !v.secure
            && !matches!(&v.expect, Expect::Exactly(t) if t.is_empty())
            && let Some(field) = v.view(&rendered).target
        {
            let waited = Instant::now();
            let mut late = None;
            while search_unanswered(s.last.as_ref(), &rendered, field)
                && waited.elapsed() < SEARCH_WAIT
                && open()
            {
                tokio::time::sleep(TREE_STEP).await;
                let tree = self.device()?.tree().await?;
                rendered = self.build(s, &tree, None);
                late = Some(tree);
            }
            if let Some(tree) =
                late.filter(|_| !search_unanswered(s.last.as_ref(), &rendered, field))
            {
                let tree = self.stabilize(tree, None, until, None).await?.0;
                rendered = self.build(s, &tree, None);
            }
        }
        // Contacts names a new phone field "Phone (Mobile)" and, once it holds text, "Phone": a new ref.
        if let (Some(v), Some(Verdict::Landed)) = (&verify, &verdict)
            && let Some(old) = v.field.filter(|r| rendered.index_of(*r).is_none())
            && let Some(new) = v.view(&rendered).target
        {
            summary = summary.replace(&format!(" into @{old}"), &format!(" into @{new}"));
        }
        if let Some(v) = verify.as_ref().filter(|v| v.secure)
            && let Some(shown) = v.view(&rendered).value
        {
            self.secrets.shown_as(v.typed(), &shown);
        }
        let verified = verdict.as_ref().and_then(Verdict::verified);
        let unverified = match verdict {
            Some(Verdict::Failed(u)) => Some(u),
            _ => None,
        };
        let typed_into = verify
            .as_ref()
            .filter(|_| verified == Some(true))
            .and_then(|v| {
                let replaced = matches!(v.expect, Expect::Exactly(_));
                v.view(&rendered).target.map(|r| (r, replaced))
            });
        let mut echoed = false;
        let (kind, text) = match s.diff_for_reply(&rendered) {
            Some(DiffOrFull::Diff(mut changes)) => {
                if let Some((field, replaced)) = typed_into {
                    let before = changes.len();
                    drop_echo(&mut changes, &rendered, field, replaced);
                    echoed = changes.len() < before;
                }
                (Kind::Diff, DiffOrFull::Diff(changes).text(&rendered))
            }
            _ => (
                Kind::Full,
                rendered.text(&Filters::default()).unwrap_or_default(),
            ),
        };
        let (settled, changed_signal, ms, still) = match st {
            Some(State::Done {
                settled,
                changed,
                elapsed,
                still_changing,
            }) => (settled, changed, elapsed.as_millis() as u32, still_changing),
            _ => (false, false, 0, None),
        };
        let changed = changed_signal || kind == Kind::Full || !text.is_empty() || echoed;
        let observation = Observation {
            header: rendered.header.clone(),
            text,
            kind,
            changed,
            settled,
            settle_ms: ms,
            still_changing: still,
            loading: rendered.loading(),
            elements: None,
            capture: None,
            empty: false,
        };
        s.set_last(Some(rendered));
        self.lap("render", render);
        Ok(ActionResult {
            rows: Vec::new(),
            at: None,
            typed_into,
            unchanged_repeats: None,
            crashed: None,
            ok: true,
            summary,
            via: via.map(str::to_string),
            verified,
            unverified,
            settled,
            settle_ms: ms,
            changed,
            woke: false,
            unlocked: false,
            observation,
        })
    }
}
