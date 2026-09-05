use super::basis::interpolate_sections;
use super::*;
use crate::spline_collocation::{
    Axis, Break, control_count, error_fractions, knots, seed_breaks, stable_lerp,
};

pub(super) fn fit(sweep: &Sweep1) -> Result<NurbsSurface, GeometryError> {
    let rail = sweep
        .rail
        .as_ref()
        .to_nurbs()?
        .try_trimmed(sweep.domain())?;
    let mut breaks = seed_breaks(rail.degree(), rail.knots(), rail.domain());
    for section in sweep
        .sections
        .iter()
        .skip(1)
        .take(sweep.sections.len().saturating_sub(2))
    {
        let multiplicity = if sweep.blend == SweepBlend::Local {
            2
        } else {
            3
        };
        if let Some(b) = breaks.iter_mut().find(|b| b.parameter == section.parameter) {
            b.multiplicity = b.multiplicity.max(multiplicity);
        } else {
            breaks.push(Break {
                parameter: section.parameter,
                multiplicity,
            });
        }
    }
    breaks.sort_by(|a, b| a.parameter.total_cmp(&b.parameter));
    // Endpoint-only or coincident interpolation/validation lattices can alias.
    // Seed each natural span independently and audit two offset lattices.
    let mut seeds = Vec::new();
    for pair in breaks
        .windows(2)
        .filter(|_| !matches!(sweep.rail.as_ref(), CurveRef::Line(_)))
    {
        for i in 1..4 {
            seeds.push(Break {
                parameter: stable_lerp(pair[0].parameter, pair[1].parameter, i as Real / 4.0)?,
                multiplicity: 1,
            });
        }
    }
    breaks.extend(seeds);
    breaks.sort_by(|a, b| a.parameter.total_cmp(&b.parameter));
    let mut fractions = error_fractions(12);
    fractions.sort_by(Real::total_cmp);
    // The cosine and uniform grids share exact mathematical sites (1/4,
    // 1/2, 3/4), whose floating evaluations can differ by one ULP. They are
    // one validation site, not an interval to integrate between adjacent floats.
    fractions.dedup_by(|a, b| (*a - *b).abs() <= 8.0 * Real::EPSILON);
    loop {
        let count = control_count(&breaks);
        if count > MAX_AXIS_CONTROLS || count * sweep.local[0].len() > MAX_SURFACE_CONTROLS {
            return Err(invalid("surface fitting control budget exceeded"));
        }
        let surface = interpolate(sweep, knots(&breaks))?;
        let mut parameters = breaks
            .windows(2)
            .flat_map(|p| {
                fractions
                    .iter()
                    .map(|&f| stable_lerp(p[0].parameter, p[1].parameter, f))
            })
            .collect::<Result<Vec<_>, _>>()?;
        parameters.sort_by(Real::total_cmp);
        parameters.dedup();
        let sections = sweep.sections_at(&parameters)?;
        let mut refine = vec![false; breaks.len() - 1];
        for (&t, target) in parameters.iter().zip(sections) {
            let curve = surface.isocurve_v(t)?;
            let bounds = target.control_point_bounds();
            let diameter = bounds.min().distance_to(bounds.max())?;
            let mut point_error: Real = 0.0;
            let mut relative_weight_error: Real = 0.0;
            for (a, b) in curve.control_points().iter().zip(target.control_points()) {
                point_error = point_error.max(a.point().distance_to(b.point())?);
                relative_weight_error =
                    relative_weight_error.max((a.weight() - b.weight()).abs() / a.weight());
            }
            // Positive rational basis functions bound the entire profile, not
            // only sampled V parameters. Rail-direction auditing is sampled.
            if point_error + diameter * relative_weight_error > sweep.tolerance.absolute() * 0.8 {
                let index = breaks
                    .partition_point(|b| b.parameter < t)
                    .saturating_sub(1)
                    .min(refine.len() - 1);
                refine[index] = true;
            }
        }
        if !refine.iter().any(|r| *r) {
            return Ok(surface);
        }
        let mut added = Vec::new();
        for (index, needed) in refine.into_iter().enumerate() {
            if !needed {
                continue;
            }
            let a = breaks[index].parameter;
            let b = breaks[index + 1].parameter;
            let t = stable_lerp(a, b, 0.5)?;
            if !(a < t && t < b) {
                return Err(invalid("surface fit exhausted parameter resolution"));
            }
            added.push(Break {
                parameter: t,
                multiplicity: 1,
            });
        }
        breaks.extend(added);
        breaks.sort_by(|a, b| a.parameter.total_cmp(&b.parameter));
    }
}

fn interpolate(sweep: &Sweep1, knots: Vec<Real>) -> Result<NurbsSurface, GeometryError> {
    let axis = Axis::cubic(knots)?;
    let parameters = axis
        .stations
        .iter()
        .map(|s| s.parameter)
        .collect::<Vec<_>>();
    let sections = sweep.sections_at(&parameters)?;
    interpolate_sections(axis, &sections, &vec![1.0; parameters.len()])
}
