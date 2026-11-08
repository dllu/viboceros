//! Compute once, then admit the same candidate atomically.
use super::*;

pub use crate::curve_pair_edit::Prepared;
pub(super) use crate::curve_pair_edit::{Plan, Target};
pub fn prepare(
    document: &Document,
    picks: [(ObjectId, Point3); 2],
    preferences: Preferences,
) -> Result<Prepared, CommandError> {
    let preferences = preferences.validate()?;
    let sources = picks.map(|(id, _)| {
        if !document.is_object_selectable(id) {
            return Err(CommandError::FilletRequiresTwoCurves);
        }
        document
            .object(id)
            .cloned()
            .ok_or(CommandError::FilletRequiresTwoCurves)
    });
    let [first, second] = sources;
    let sources = [first?, second?];
    let curve = |index: usize| {
        sources[index]
            .geometry()
            .curve_ref()
            .ok_or(CommandError::FilletRequiresTwoCurves)
            .map(|c| c.to_owned())
    };
    let first = curve(0)?;
    let second = curve(1)?;
    let closed = [first.as_ref().is_closed()?, second.as_ref().is_closed()?];
    let plan = if picks[0].0 == picks[1].0 {
        super::same_source::prepare(
            &first,
            picks.map(|(_, p)| p),
            preferences,
            document.tolerance(),
        )?
    } else if preferences.radius == 0. {
        super::zero::prepare(
            [&first, &second],
            picks.map(|(_, p)| p),
            closed,
            preferences,
            document.tolerance(),
        )?
    } else if closed.iter().any(|c| *c) {
        super::closed::prepare(
            [&first, &second],
            picks.map(|(_, p)| p),
            closed,
            preferences,
            document.tolerance(),
        )?
    } else {
        super::open::prepare(
            [&first, &second],
            picks.map(|(_, p)| p),
            preferences,
            document.tolerance(),
        )?
    };
    Ok(Prepared::new(document, sources, plan, "Fillet"))
}
