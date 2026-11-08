//! Prepare geometry once and preserve source identity through shared admission.
use super::*;
pub use crate::curve_pair_edit::Prepared;

pub fn prepare(
    document: &Document,
    picks: [(ObjectId, Point3); 2],
    preferences: Preferences,
) -> Result<Prepared, CommandError> {
    let preferences = preferences.validate()?;
    let sources = picks.map(|(id, _)| {
        document
            .object(id)
            .filter(|_| document.is_object_selectable(id))
            .cloned()
            .ok_or(CommandError::ChamferRequiresTwoCurves)
    });
    let [a, b] = sources;
    let sources = [a?, b?];
    let curve = |i: usize| {
        sources[i]
            .geometry()
            .curve_ref()
            .map(|c| c.to_owned())
            .ok_or(CommandError::ChamferRequiresTwoCurves)
    };
    let (first, second) = (curve(0)?, curve(1)?);
    if picks[0].0 == picks[1].0 {
        let plan = super::same_source::prepare(
            &first,
            picks.map(|(_, p)| p),
            preferences,
            document.tolerance(),
        )?;
        return Ok(Prepared::new(document, sources, plan, "Chamfer"));
    }
    let closed = [first.as_ref().is_closed()?, second.as_ref().is_closed()?];
    if closed.iter().any(|c| *c) {
        let plan = super::closed::prepare(
            [&first, &second],
            picks.map(|(_, p)| p),
            closed,
            preferences,
            document.tolerance(),
        )?;
        return Ok(Prepared::new(document, sources, plan, "Chamfer"));
    }
    let plan = super::open::prepare(
        [&first, &second],
        picks.map(|(_, p)| p),
        preferences,
        document.tolerance(),
    )?;
    Ok(Prepared::new(document, sources, plan, "Chamfer"))
}
