//! Prepare geometry once and preserve source identity through shared admission.
use super::*;
pub use crate::curve_pair_edit::Prepared;
use crate::curve_pair_edit::{Plan, Target, retain_line_domain};

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
    let outputs = if preferences.join && preferences.trim {
        let joined = crate::curve_connections::chamfer_joined(
            (&first, picks[0].1),
            (&second, picks[1].1),
            preferences.distances,
            preferences.styles,
            document.tolerance(),
        )?;
        let source = first.as_ref();
        let domain = source.domain();
        let origin = if picks[0].1.distance_to(source.evaluate(*domain.end())?)?
            < picks[0].1.distance_to(source.evaluate(*domain.start())?)?
        {
            *domain.start()
        } else {
            -*domain.end()
        };
        let width = *joined.domain().end() - *joined.domain().start();
        vec![Geometry::PolyCurve(
            joined.try_reparameterized(origin..=origin + width)?,
        )]
    } else {
        let mut parts = crate::curve_connections::chamfer_parts(
            (&first, picks[0].1),
            (&second, picks[1].1),
            preferences.distances,
            preferences.trim,
            preferences.styles,
            document.tolerance(),
        )?;
        if preferences.trim {
            for (source, result) in [&first, &second].into_iter().zip(parts.iter_mut()) {
                retain_line_domain(source, result, document.tolerance())?;
            }
        }
        parts.into_iter().map(Geometry::from).collect()
    };
    let targets = if preferences.trim {
        if preferences.join {
            vec![Target::Source(0)]
        } else {
            (0..outputs.len())
                .map(|i| {
                    if i < 2 {
                        Target::Source(i)
                    } else {
                        Target::New
                    }
                })
                .collect()
        }
    } else {
        vec![Target::New; outputs.len()]
    };
    let deleted = if preferences.trim && preferences.join {
        vec![1]
    } else {
        vec![]
    };
    Ok(Prepared::new(
        document,
        sources,
        Plan {
            outputs,
            targets,
            deleted,
        },
        "Chamfer",
    ))
}
