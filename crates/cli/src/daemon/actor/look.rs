//! A `tap` on text that is not on screen looks for it where the screen scrolls, and puts the screen back when the
//! text is nowhere (decision D140).

use std::collections::HashMap;

use phonedriver_core::element::Role;
use phonedriver_core::gesture::PULL_LEAD;
use phonedriver_core::refs::RefTable;

use super::*;
use crate::daemon::session::{Way, looks};

/// How many drags a `tap` makes in all, looking for text that is not on screen. Six show as much of a list as the
/// eight drags of a `scroll` that this began with, which was twice the most a recorded agent's `scroll` needed
/// after such a tap was refused (1 or 4 drags, 8 runs). A `scroll` with `until` looks further.
const LOOK_DRAGS: usize = 6;

/// How far the finger travels on each drag, as a share of what scrolls. More than a scroll's 60%, since the rows
/// are read after every drag and not by the agent, and it leaves a fifth of what scrolls on screen from one read to
/// the next, which is what tells how far the rows moved. The finger stays a tenth away from both ends: 41 points on
/// a row of tabs as wide as a phone, outside the strip at the screen's edge where Android takes a drag for Back.
const LOOK_DISTANCE: f32 = 0.8;

/// How often the way back is dragged and then read: the drags back themselves, and two more rounds for what they
/// left. With one round after the drags back, the testbed's row of tabs ended 16 points off in 1 run of 25; with
/// two, 49 runs ended within two pixels of where the row began, 44 of them on it (API 37 emulator).
const BACK_ROUNDS: u32 = 3;

/// One way a look went: the screen before each drag and after the last, and for each drag how far the finger
/// travelled and how far that moved the rows.
struct Leg {
    way: Way,
    screens: Vec<Rendered>,
    drags: Vec<(f32, Option<f32>)>,
}

/// How far the rows under `holder` moved against `direction` from one read to the next: the shift most of their
/// edges share. A row is told by its role and label, where one element under `holder` has them in each read. Its
/// ref does not tell it: a row that comes back to the screen at another place than it left is given a new one. An
/// edge counts when it lies inside what scrolls in both reads. One that lies on its border may belong to a row
/// that is cut off there, which is the only part of a row 150 points tall that two reads share. `None` when no edge
/// is inside in both, as with cards a screen tall.
fn shifted(before: &Rendered, after: &Rendered, holder: Ref, direction: Direction) -> Option<f32> {
    let ((was_in, was), (is_in, is)) = (
        rows(before, holder, direction)?,
        rows(after, holder, direction)?,
    );
    let inside = |edge: f32, [from, to]: [f32; 2]| edge > from + 0.5 && edge < to - 0.5;
    let mut shifts: Vec<(f32, u32)> = Vec::new();
    for (row, was) in &was {
        let (Some(was), Some(Some(is))) = (was, is.get(row)) else {
            continue;
        };
        for (was, is) in was.iter().zip(is) {
            if !inside(*was, was_in) || !inside(*is, is_in) {
                continue;
            }
            let shift = match direction {
                Direction::Down | Direction::Right => was - is,
                Direction::Up | Direction::Left => is - was,
            };
            match shifts.iter_mut().find(|(s, _)| (*s - shift).abs() <= 0.5) {
                Some((_, n)) => *n += 1,
                None => shifts.push((shift, 1)),
            }
        }
    }
    shifts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.abs().total_cmp(&a.0.abs())))
        .map(|(shift, _)| shift)
}

fn edges(b: Rect, direction: Direction) -> [f32; 2] {
    match direction {
        Direction::Down | Direction::Up => [b.y, b.y + b.h],
        Direction::Right | Direction::Left => [b.x, b.x + b.w],
    }
}

type Rows<'a> = HashMap<(Role, &'a str), Option<[f32; 2]>>;

/// The edges of what scrolls, and of each labelled element under it whose role and label no other one there has.
fn rows(now: &Rendered, holder: Ref, direction: Direction) -> Option<([f32; 2], Rows<'_>)> {
    let top = now.index_of(holder)?;
    let mut rows = Rows::new();
    for (i, e) in now.elements.iter().enumerate() {
        let under = std::iter::successors(now.parents[i], |&p| now.parents[p]).any(|p| p == top);
        if let Some(label) = e.label.as_deref().filter(|_| under) {
            rows.entry((e.role, label))
                .and_modify(|twice| *twice = None)
                .or_insert(Some(edges(e.bounds, direction)));
        }
    }
    Some((edges(now.elements[top].bounds, direction), rows))
}

fn extent(area: Rect, direction: Direction) -> f32 {
    match direction {
        Direction::Down | Direction::Up => area.h,
        Direction::Right | Direction::Left => area.w,
    }
}

impl Leg {
    fn area(&self, screen: &Rendered) -> Option<Rect> {
        let holder = screen.index_of(self.way.holder)?;
        Some(above_keyboard(
            screen.elements[holder].bounds,
            screen.header.keyboard,
        ))
    }

    /// How far the rows are from where the leg began, when every drag's shift could be read.
    fn gone(&self) -> Option<f32> {
        self.drags.iter().map(|(_, shift)| *shift).sum()
    }

    /// How far the rows were from where the leg began at each of its screens.
    fn offsets(&self) -> impl Iterator<Item = f32> + '_ {
        let mut at = 0.0;
        std::iter::once(0.0).chain(self.drags.iter().map(move |(_, shift)| {
            at += shift.unwrap_or_default();
            at
        }))
    }

    /// How far the rows on `now` are from where the leg began: read against the first of the leg's screens that
    /// shares an edge with it, which is the first one only when the rows are nearly back.
    fn off(&self, now: &Rendered) -> Option<f32> {
        let (holder, direction) = (self.way.holder, self.way.direction);
        self.screens
            .iter()
            .zip(self.offsets())
            .find_map(|(screen, at)| Some(at + shifted(screen, now, holder, direction)?))
    }

    /// The length of a drag that moves nothing: what at least two drags lost alike, among those the next drag shows
    /// were not cut short by the end of the list. An Android list loses a pull's first move, `PULL_LEAD` (15.8 of
    /// its 16 points on the API 37 emulator, where a point is 2.625 pixels), and a list that follows the finger
    /// from where it went down would lose nothing. One drag alone does not say: of the testbed's six drags down,
    /// one in ten lost a step more (552.8 points moved where the others moved 622.1) because the app was late, and
    /// some moved 10.7 more. The first drag's loss sent each drag back 34.7 points too far, so the row of tabs was
    /// pulled into its start and the tap that followed did nothing while the row gave that back; the least loss
    /// left the list 10.7 short.
    fn lost(&self) -> Option<f32> {
        let losses: Vec<f32> = self
            .drags
            .windows(2)
            .filter(|pair| pair[1].1.is_some_and(|next| next > 1.0))
            .filter_map(|pair| Some(pair[0].0 - pair[0].1?))
            .collect();
        let alike = |loss: f32| losses.iter().filter(|l| (*l - loss).abs() <= 1.0).count();
        losses
            .iter()
            .filter(|loss| alike(**loss) >= 2)
            .max_by(|a, b| {
                alike(**a)
                    .cmp(&alike(**b))
                    .then((*b - PULL_LEAD).abs().total_cmp(&(*a - PULL_LEAD).abs()))
            })
            .map(|loss| loss.clamp(0.0, PULL_LEAD))
    }
}

impl Actor {
    /// Looks for the text a `tap` named where the screen shows no such text, nothing close to it, and something that
    /// scrolls: the drags of the `scroll` the refusal used to name, each way in turn, until the text resolves, the
    /// way ends (what scrolls says it goes no further, or two drags change nothing) or `LOOK_DRAGS`.
    ///
    /// A look that finds nothing puts the screen back, so a refused tap leaves it as the agent saw it. The rows get
    /// their refs from the table as it was before the look: left to the reads in between, a row that came back by
    /// way of another place kept the ref it was given there (Settings, API 37 emulator: every row but the first
    /// had a new one).
    pub(super) async fn look_for(
        &mut self,
        s: &mut Session,
        consent: &Consent,
        cfg: Config,
        target: &Target,
    ) -> Result<(Resolved, String), Error> {
        let device = self.device()?;
        let held = s.refs.clone();
        let mut now = self.fresh(s).await?;
        let mut looked = Vec::new();
        let mut over = Vec::new();
        let mut legs: Vec<Leg> = Vec::new();
        let mut left = LOOK_DRAGS;
        for way in looks(&now) {
            if left == 0 {
                break;
            }
            let mut leg = Leg {
                way,
                screens: Vec::new(),
                drags: Vec::new(),
            };
            let (mut moved, mut idle) = (0u32, 0u32);
            let found = loop {
                match s.resolve(target, &now) {
                    Ok(found) if found.covered.is_none() || idle >= 2 => break Some(found),
                    Ok(_) | Err(Error::NotFound { .. }) => {}
                    Err(e) => return Err(e),
                }
                let Some(area) = leg.area(&now).filter(|_| way.goes_on(&now)) else {
                    break None;
                };
                if idle >= 2 || left == 0 {
                    break None;
                }
                left -= 1;
                let finger = LOOK_DISTANCE * extent(area, way.direction);
                let drag = gesture::pull(area, way.direction.opposite(), finger);
                self.guard_touch(s, consent, &drag, false, Some(&now), None)
                    .await?;
                let started = self.begin_action();
                self.watching(device.gesture(drag)).await?;
                self.settle(cfg, started).await;
                let next = self.fresh(s).await?;
                let changed = !matches!(diff(&now, &next), DiffOrFull::Diff(c) if c.is_empty());
                idle = if changed { 0 } else { idle + 1 };
                moved += u32::from(changed);
                let shift = if changed {
                    shifted(&now, &next, way.holder, way.direction)
                } else {
                    Some(0.0)
                };
                leg.drags.push((finger, shift));
                leg.screens.push(std::mem::replace(&mut now, next));
            };
            let name = way.direction.name();
            let made = plural(leg.drags.len() as u64, "time");
            if let Some(found) = found {
                looked.push(format!("{name} {made}"));
                return Ok((found, format!("scrolled {}, then ", looked.join(" and "))));
            }
            if idle >= 2 || !way.goes_on(&now) {
                over.push(way.words());
                if moved > 0 {
                    looked.push(format!("{name} to the end"));
                }
            } else {
                looked.push(format!("{name} {made}"));
            }
            leg.screens.push(now.clone());
            legs.push(leg);
        }
        let mut stayed = false;
        for leg in legs.iter().rev() {
            stayed |= !self.back(s, consent, cfg, leg).await?;
        }
        let now = self.reread(s, held).await?;
        match s.resolve(target, &now) {
            Err(Error::NotFound {
                query,
                similar,
                mut scrolls,
                screen,
                ..
            }) => {
                scrolls.retain(|way| !over.contains(way));
                Err(Error::NotFound {
                    query,
                    similar,
                    scrolls,
                    looked: looked.into(),
                    stayed,
                    screen,
                })
            }
            found => found.map(|found| (found, String::new())),
        }
    }

    async fn reread(&mut self, s: &mut Session, refs: RefTable) -> Result<Rendered, Error> {
        s.refs = refs;
        self.fresh(s).await
    }

    /// Drags the rows of `leg` back to where they were when it began, and says whether they are there. The distance
    /// is what the screen showed, not a count of drags: the last drag forward is cut short where the list ends, and
    /// as many whole drags back left a list short of where it began from its middle, and at its top went on
    /// pulling, which is how a feed is told to refresh.
    ///
    /// A round is equal pulls that follow each other with no read between, each checked for the guard against the
    /// screen the leg read nearest to where it starts, then a read that says how far off the rows still are. They
    /// are off after the first round about one time in ten on the emulator: the first pull back from the end of
    /// the testbed's row of tabs moved 20 to 68 points less than its length while the row gave back the stretch of
    /// the drag that had reached its end, and a pull the app is late for loses a step (API 37 emulator). A pull
    /// shorter than twice the touch slop is not made, since a touch that hardly moves is a tap.
    async fn back(
        &mut self,
        s: &mut Session,
        consent: &Consent,
        cfg: Config,
        leg: &Leg,
    ) -> Result<bool, Error> {
        let Some(mut off) = leg.gone() else {
            return Ok(false);
        };
        let direction = leg.way.direction;
        let lost = leg.lost().unwrap_or(PULL_LEAD);
        let device = self.device()?;
        let mut seen = leg.screens[leg.screens.len() - 1].clone();
        for _ in 0..BACK_ROUNDS {
            let Some(area) = leg.area(&seen) else {
                return Ok(false);
            };
            let reach = LOOK_DISTANCE * extent(area, direction) - lost;
            let pulls = (off.abs() / reach - 0.01).ceil().max(1.0);
            let each = off.abs() / pulls;
            if off.abs() < 1.0 || each + lost < 2.0 * guard::TOUCH_SLOP {
                return Ok(true);
            }
            let toward = match off > 0.0 {
                true => direction,
                false => direction.opposite(),
            };
            let mut started = self.begin_action();
            for n in 0..pulls as usize {
                let from = off - off.signum() * each * n as f32;
                let nearest = leg
                    .offsets()
                    .enumerate()
                    .min_by(|a, b| (a.1 - from).abs().total_cmp(&(b.1 - from).abs()))
                    .map_or(0, |(i, _)| i);
                let pull = gesture::pull(area, toward, each + lost);
                self.guard_touch(s, consent, &pull, false, Some(&leg.screens[nearest]), None)
                    .await?;
                started = self.begin_action();
                self.watching(device.gesture(pull)).await?;
            }
            self.settle(cfg, started).await;
            seen = self.fresh(s).await?;
            let Some(now) = leg.off(&seen) else {
                return Ok(false);
            };
            off = now;
        }
        Ok(off.abs() < PULL_LEAD)
    }
}
