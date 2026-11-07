//! Assemble retained parent intervals and an optional inserted arc or bevel.
use super::*;
use viboceros_geometry::PolyCurve3;

pub(crate) fn prepare(
    source: &Curve3,
    mut parameters: [Real; 2],
    mut insertion: Option<Curve3>,
    trim: bool,
    join: bool,
    tolerance: Tolerance,
    closed_wrap: Option<bool>,
) -> Result<Plan, CommandError> {
    let mut plan = Plan {
        outputs: Vec::new(),
        targets: Vec::new(),
        deleted: Vec::new(),
    };
    if parameters[0] > parameters[1] {
        parameters.swap(0, 1);
        insertion = insertion.map(|c| c.reversed(tolerance)).transpose()?;
    }
    let [low, high] = parameters;
    insertion = insertion
        .map(|c| c.try_reparameterized(0. ..=c.as_ref().length(tolerance)?))
        .transpose()?;
    if !trim {
        if let Some(c) = insertion {
            plan.outputs.push(c.into());
            plan.targets.push(Target::New);
        }
        return Ok(plan);
    }
    let domain = source.as_ref().domain();
    let retained = if source.as_ref().is_closed()? {
        let mut retained = if let Some(c) = &insertion {
            let tangent = source
                .as_ref()
                .evaluate_with_tangent(low)?
                .tangent()
                .as_vector();
            let inserted = c
                .as_ref()
                .evaluate_with_tangent(*c.as_ref().domain().start())?
                .tangent()
                .as_vector();
            if closed_wrap.unwrap_or(tangent.dot(inserted)? > 0.) {
                crate::curve_segments::directed(source, high, low)?
            } else {
                crate::curve_segments::trimmed(source, low..=high)?
            }
        } else {
            crate::curve_segments::seam(source, low)?
        };
        if join
            && let Some(c) = &insertion
            && retained
                .as_ref()
                .start_point()?
                .distance_to(c.as_ref().end_point()?)?
                > tolerance.absolute()
        {
            retained = retained.reversed(tolerance)?;
        }
        vec![retained]
    } else {
        vec![
            crate::curve_segments::trimmed(source, *domain.start()..=low)?,
            crate::curve_segments::trimmed(source, high..=*domain.end())?,
        ]
    };
    if join {
        let mut leaves = Vec::new();
        for (index, piece) in retained.iter().enumerate() {
            if index == 1
                && let Some(c) = &insertion
            {
                leaves.extend(c.to_polycurve()?.segments().iter().cloned());
            }
            leaves.extend(piece.to_polycurve()?.segments().iter().cloned());
        }
        if retained.len() == 1
            && let Some(c) = &insertion
        {
            leaves.extend(c.to_polycurve()?.segments().iter().cloned());
        }
        plan.outputs
            .push(Geometry::PolyCurve(PolyCurve3::try_new(leaves)?));
        plan.targets.push(Target::Source(0));
    } else {
        for (index, piece) in retained.into_iter().enumerate() {
            plan.outputs.push(piece.into());
            plan.targets.push(if index == 0 {
                Target::Source(0)
            } else {
                Target::FirstAttributes
            });
        }
        if let Some(c) = insertion {
            plan.outputs.push(c.into());
            plan.targets.push(Target::New);
        }
    }
    Ok(plan)
}
