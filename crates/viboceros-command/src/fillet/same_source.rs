//! A same-parent fillet edits one source, preserving the unconsumed intervals.
use super::{
    prepare::{Plan, Target},
    *,
};
use viboceros_geometry::{CurveRef, CurveSegment3, PolyCurve3};
mod leaf;
mod select;

pub(super) fn prepare(
    source: &Curve3,
    picks: [Point3; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    if options.radius == 0. {
        return Err(CommandError::Usage(
            "Unable to intersect curves or curve extensions",
        ));
    }
    let mut plan = Plan {
        outputs: Vec::new(),
        targets: Vec::new(),
        deleted: Vec::new(),
    };
    let Some((mut parameters, mut arc)) = select::candidate(source, picks, options, tolerance)?
    else {
        return Ok(plan);
    };
    if parameters[0] > parameters[1] {
        parameters.swap(0, 1);
        arc = arc.reversed(tolerance)?;
    }
    let [low, high] = parameters;
    arc = arc.try_reparameterized(0. ..=arc.length()?)?;
    if !options.trim {
        plan.outputs.push(Geometry::Arc(arc));
        plan.targets.push(Target::New);
        return Ok(plan);
    }
    let domain = source.as_ref().domain();
    let retained = if source.as_ref().is_closed()? {
        // The arc's tangent determines which cyclic interval it replaces.
        let tangent = source
            .as_ref()
            .evaluate_with_tangent(low)?
            .tangent()
            .as_vector();
        let arc_tangent = CurveRef::Arc(&arc)
            .evaluate_with_tangent(*arc.domain().start())?
            .tangent()
            .as_vector();
        let mut retained = if tangent.dot(arc_tangent)? > 0. {
            super::super::curve_segments::directed(source, high, low)?
        } else {
            super::super::curve_segments::trimmed(source, low..=high)?
        };
        if options.join
            && retained.as_ref().start_point()?.distance_to(arc.end()?)? > tolerance.absolute()
        {
            retained = retained.reversed(tolerance)?;
        }
        vec![retained]
    } else {
        vec![
            super::super::curve_segments::trimmed(source, *domain.start()..=low)?,
            super::super::curve_segments::trimmed(source, high..=*domain.end())?,
        ]
    };
    if options.join {
        let mut leaves = Vec::new();
        for (index, piece) in retained.iter().enumerate() {
            if index == 1 {
                leaves.push(CurveSegment3::Arc(arc));
            }
            leaves.extend(piece.to_polycurve()?.segments().iter().cloned());
        }
        if retained.len() == 1 {
            leaves.push(CurveSegment3::Arc(arc));
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
        plan.outputs.push(Geometry::Arc(arc));
        plan.targets.push(Target::New);
    }
    Ok(plan)
}
