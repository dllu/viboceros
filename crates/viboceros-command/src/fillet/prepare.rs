//! Compute once, then admit the same candidate atomically.
use super::*;

pub use crate::curve_pair_edit::Prepared;
use crate::curve_pair_edit::retain_line_domain;
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
    } else if closed.iter().any(|c| *c) {
        super::closed::prepare(
            [&first, &second],
            picks.map(|(_, p)| p),
            closed,
            preferences,
            document.tolerance(),
        )?
    } else {
        let outputs = if preferences.join && preferences.trim {
            let joined = super::super::curve_connections::fillet_joined(
                (&first, picks[0].1),
                (&second, picks[1].1),
                preferences.radius,
                preferences.styles,
                document.tolerance(),
            )?;
            let source = first.as_ref();
            let domain = source.domain();
            let a = source.evaluate(*domain.start())?;
            let b = source.evaluate(*domain.end())?;
            let origin = if picks[0].1.distance_to(b)? < picks[0].1.distance_to(a)? {
                *domain.start()
            } else {
                -*domain.end()
            };
            let width = *joined.domain().end() - *joined.domain().start();
            vec![Geometry::PolyCurve(
                joined.try_reparameterized(origin..=origin + width)?,
            )]
        } else {
            let mut parts = super::super::curve_connections::fillet_parts(
                (&first, picks[0].1),
                (&second, picks[1].1),
                preferences.radius,
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
        Plan {
            outputs,
            targets,
            deleted: if preferences.trim && preferences.join {
                vec![1]
            } else {
                vec![]
            },
        }
    };
    Ok(Prepared::new(document, sources, plan, "Fillet"))
}
