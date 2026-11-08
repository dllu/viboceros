//! Positive-radius connected-end construction for two open sources.
use super::{
    prepare::{Plan, Target},
    *,
};
use crate::curve_pair_edit::retain_line_domain;

pub(super) fn prepare(
    curves: [&Curve3; 2],
    picks: [Point3; 2],
    preferences: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    let [first, second] = curves;
    let outputs = if preferences.join && preferences.trim {
        let joined = super::super::curve_connections::fillet_joined(
            (first, picks[0]),
            (second, picks[1]),
            preferences.radius,
            preferences.styles,
            tolerance,
        )?;
        let source = first.as_ref();
        let domain = source.domain();
        let a = source.evaluate(*domain.start())?;
        let b = source.evaluate(*domain.end())?;
        let origin = if picks[0].distance_to(b)? < picks[0].distance_to(a)? {
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
            (first, picks[0]),
            (second, picks[1]),
            preferences.radius,
            preferences.trim,
            preferences.styles,
            tolerance,
        )?;
        if preferences.trim {
            for (source, result) in [first, second].into_iter().zip(parts.iter_mut()) {
                retain_line_domain(source, result, tolerance)?;
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
    Ok(Plan {
        outputs,
        targets,
        deleted: if preferences.trim && preferences.join {
            vec![1]
        } else {
            vec![]
        },
    })
}
