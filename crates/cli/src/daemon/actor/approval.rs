//! The approval guard's checks before a touch (decision D27).

use super::*;

impl Actor {
    /// A tap on a button that spends money or cannot be undone needs the human's word (decision D27). A point is
    /// matched to the element under it on a fresh read; an element that takes no touch of its own counts as the row
    /// it sits in.
    pub(super) async fn guard_tap(
        &mut self,
        s: &mut Session,
        c: &Consent,
        target: &Target,
        found: Option<&Element>,
        at: Point,
    ) -> Result<Option<&'static str>, Error> {
        if c.approval == Some(Approval::Autonomous) {
            return Ok(None);
        }
        let button = match (found, target) {
            (Some(e), _) => {
                let now = self.screen_for_guard(s).await?;
                match now.index_of(e.r#ref) {
                    Some(i) => guard::receiver(&now, &now.elements[i], &c.ask).cloned(),
                    None => guard::guarded(e, &c.ask).map(|_| e.clone()),
                }
            }
            (None, Target::Point { .. }) => {
                let now = self.screen_for_guard(s).await?;
                guard::clicked(&now, at, &c.ask).cloned()
            }
            _ => None,
        };
        self.guard_button(s, c, button).await
    }

    /// Every other touch a verb sends (decision D104): a finger that goes down and lifts on a guarded element, or a
    /// stroke along a "Slide to pay" track, commits it as a tap would. It is checked against a screen already in
    /// hand (`on`, or the session's last screen when it is sure), so an ordinary swipe costs no read; only a gesture
    /// shaped like a tap, or a drop, reads the screen when none is.
    pub(super) async fn guard_touch(
        &mut self,
        s: &mut Session,
        c: &Consent,
        g: &Gesture,
        drop: bool,
        on: Option<&Rendered>,
        start: Option<&Element>,
    ) -> Result<Option<&'static str>, Error> {
        if c.approval == Some(Approval::Autonomous) {
            return Ok(None);
        }
        let known = on.or(s.last.as_ref().filter(|_| !s.unsure));
        let button = match known {
            Some(now) => guard::touched(now, g, drop, &c.ask).cloned(),
            None if start.is_some() && !drop => {
                start.filter(|e| guard::strokes(e, g, &c.ask)).cloned()
            }
            None if drop || guard::tap_like(g) => {
                let now = self.screen_for_guard(s).await?;
                guard::touched(&now, g, drop, &c.ask).cloned()
            }
            None => None,
        };
        self.guard_button(s, c, button).await
    }

    pub(super) async fn screen_for_guard(&mut self, s: &mut Session) -> Result<Rendered, Error> {
        if let Some((read, view)) = &self.guard_view
            && self.now().saturating_sub(*read) < GUARD_VIEW_REUSE
            && self.reshaped.is_none_or(|t| t <= *read)
        {
            return Ok(view.clone());
        }
        self.fresh(s).await
    }

    /// Enter sends the form: guarded when the field's own form holds such a button, enabled or not yet (a Pay
    /// button that the form turns on once the field is filled). The session's last screen is used when it is sure.
    pub(super) async fn guard_submit(
        &mut self,
        s: &mut Session,
        c: &Consent,
        field: Option<&Element>,
    ) -> Result<Option<&'static str>, Error> {
        if c.approval == Some(Approval::Autonomous) {
            return Ok(None);
        }
        let now = match s.last.clone().filter(|_| !s.unsure) {
            Some(last) => last,
            None => self.fresh(s).await?,
        };
        let button = guard::submit_button(&now, field, &c.ask).cloned();
        self.guard_button(s, c, button).await
    }

    /// `press enter` sends what has focus: a focused guarded button, or the focused field's form.
    pub(super) async fn guard_enter(
        &mut self,
        s: &mut Session,
        c: &Consent,
    ) -> Result<Option<&'static str>, Error> {
        if c.approval == Some(Approval::Autonomous) {
            return Ok(None);
        }
        let now = match s.last.clone().filter(|_| !s.unsure) {
            Some(last) => last,
            None => self.fresh(s).await?,
        };
        let button = guard::entered(&now, &c.ask).cloned();
        self.guard_button(s, c, button).await
    }

    pub(super) async fn guard_button(
        &mut self,
        s: &mut Session,
        c: &Consent,
        button: Option<Element>,
    ) -> Result<Option<&'static str>, Error> {
        let Some((button, category)) =
            button.and_then(|b| guard::category(&b, &c.ask).map(|k| (b, k)))
        else {
            return Ok(None);
        };
        let label = button
            .label
            .clone()
            .or(button.value.clone())
            .unwrap_or_default();
        let ask = |now: &Rendered, details: Vec<String>| Error::NeedsApproval {
            label: label.clone(),
            category,
            app: guard::app_name(now),
            details,
        };
        if let Some(shown) = c
            .approved
            .as_ref()
            .filter(|_| matches!(c.approval, Some(Approval::Once | Approval::Session)))
        {
            let now = self.fresh(s).await?;
            let details = guard::context(&now, &button);
            if shown.label != label || shown.details != details {
                return Err(ask(&now, details));
            }
        }
        match guard::stance(s, c.approval, category) {
            guard::Stance::Run(note) => Ok(note),
            guard::Stance::Ask => {
                let now = self.fresh(s).await?;
                let details = guard::context(&now, &button);
                Err(ask(&now, details))
            }
        }
    }
}
