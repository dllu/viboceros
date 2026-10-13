//! Sweep representation and section-station collocation, separate from model fitting.

use super::*;
use crate::spline_collocation::Axis;
use faer::Mat;

pub(super) fn rail_basis<E: From<GeometryError>>(
    sweep: &Sweep1,
    refit: bool,
    operations: &mut SweepSurfaceOperations<'_, E>,
) -> Result<NurbsSurface, E> {
    let native = sweep.rail.try_trimmed(sweep.domain())?;
    let source = native.as_ref().to_nurbs()?;
    let mut rail = (operations.clamp)(&source)?;
    if rail.degree() != source.degree() || rail.domain() != source.domain() {
        return Err(invalid("rail clamping changed degree or domain").into());
    }
    let mut stations = sweep
        .sections
        .iter()
        .map(|section| native.as_ref().nurbs_parameter(section.parameter))
        .collect::<Result<Vec<_>, _>>()?;
    // Interior sections must be interpolation sites. Rhino can refit even
    // with RefitRail=No when the supplied stations are not existing Grevilles.
    let refit = refit || stations.iter().any(|&t| !is_station(&rail, t));
    let sampler = if refit {
        let mut sampler =
            crate::curve::ArcLengthSampler::try_new(native.as_ref(), sweep.tolerance)?;
        sampler.prepare_budgeted_repeated_sampling(32)?;
        stations = sweep
            .sections
            .iter()
            .map(|section| sampler.distance_at_parameter(section.parameter))
            .collect::<Result<Vec<_>, _>>()?;
        rail = (operations.fit)(
            native.as_ref(),
            3,
            sweep.tolerance.absolute() * 0.25,
            sweep.tolerance.angular(),
            sweep.tolerance,
        )?;
        if rail.degree() != 3 || rail.domain() != (0. ..=sampler.total_length()) {
            return Err(invalid("rail fit changed requested degree or arc-length domain").into());
        }
        Some(sampler)
    } else {
        None
    };
    rail = with_section_stations(rail, &stations, operations)?;
    if rail.control_points().len() > MAX_AXIS_CONTROLS
        || rail.control_points().len() * sweep.local[0].len() > MAX_SURFACE_CONTROLS
    {
        return Err(invalid("rail basis control budget exceeded").into());
    }
    let axis = Axis::sites(rail.degree(), rail.knots().to_vec())?;
    let parameters = axis
        .stations
        .iter()
        .map(|s| {
            // Pin original section constraints rather than letting arc-length
            // inversion round their native station to either side.
            if let Some(i) = stations
                .iter()
                .position(|&t| parameters_near(t, s.parameter, rail.domain()))
            {
                Ok(sweep.sections[i].parameter)
            } else if let Some(sampler) = &sampler {
                sampler.parameter_at_distance(s.parameter)
            } else {
                native.as_ref().parameter_from_nurbs(s.parameter)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sample_stations = axis
        .stations
        .iter()
        .map(|s| s.parameter)
        .collect::<Vec<_>>();
    let mut sections = sweep.sections_at_stations(
        &parameters,
        &sample_stations,
        &stations,
        if refit {
            sweep.blend
        } else {
            SweepBlend::Local
        },
        if refit {
            BlendCoordinates::Homogeneous
        } else {
            BlendCoordinates::Euclidean
        },
    )?;
    // Retained-basis construction blends raw relative profile weights first,
    // then applies the common-basis end-weight policy to every placed section.
    // Normalizing the inputs before blending is a different surface.
    let normalized_targets = if refit {
        None
    } else {
        sections = crate::section_basis::normalized_end_weights(&sections)?;
        Some(crate::section_basis::normalized_end_weights(
            sweep.sections.iter().map(|s| &s.curve),
        )?)
    };
    // A common projective scale does not change the rail or the surface.
    // Remove it before combining rail denominators with spatial coordinates.
    let weight_scale = rail
        .control_points()
        .iter()
        .map(|c| c.weight().abs())
        .fold(0.0, Real::max)
        .copysign(rail.control_points()[0].weight());
    let weights = axis
        .stations
        .iter()
        .map(|s| {
            let basis = crate::nurbs::bspline_basis_values(
                rail.knots(),
                rail.degree(),
                rail.control_points().len(),
                s.parameter,
            )?;
            Ok(basis
                .iter()
                .zip(rail.control_points())
                .map(|(b, c)| b * (c.weight() / weight_scale))
                .sum::<Real>())
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let surface = interpolate_sections(axis, &sections, &weights, operations)?;
    super::weights::require_positive_denominator(&surface)?;
    // Near-equality of parameter sites is only a numerical convenience, not
    // permission to discard a profile (notably on large shifted U domains).
    // Audit complete positive-weight section bases, up to common weight scale.
    for (i, (section, &station)) in sweep.sections.iter().zip(&stations).enumerate() {
        let actual = surface.isocurve_v(station)?;
        let target = normalized_targets
            .as_ref()
            .map_or(&section.curve, |curves| &curves[i]);
        let scale = |curve: &NurbsCurve| {
            curve
                .control_points()
                .iter()
                .map(|c| c.weight())
                .fold(0.0, Real::max)
        };
        let a_scale = scale(&actual);
        let b_scale = scale(target);
        let bounds = target.control_point_bounds();
        let diameter = bounds.min().distance_to(bounds.max())?;
        let mut point_error: Real = 0.0;
        let mut weight_error: Real = 0.0;
        for (a, b) in actual.control_points().iter().zip(target.control_points()) {
            point_error = point_error.max(a.point().distance_to(b.point())?);
            let a = a.weight() / a_scale;
            let b = b.weight() / b_scale;
            let relative = (a - b).abs() / a;
            if !relative.is_finite() {
                return Err(invalid("section interpolation lost a supplied profile").into());
            }
            weight_error = weight_error.max(relative);
        }
        if point_error + diameter * weight_error > sweep.tolerance.absolute() {
            return Err(invalid("section interpolation lost a supplied profile").into());
        }
    }
    Ok(surface)
}

fn parameters_near(a: Real, b: Real, domain: RangeInclusive<Real>) -> bool {
    let scale = domain.start().abs().max(domain.end().abs());
    (a - b).abs() <= 64.0 * Real::EPSILON * scale
}

fn is_station(rail: &NurbsCurve, t: Real) -> bool {
    (0..rail.control_points().len()).any(|i| {
        crate::nurbs::stable_knot_mean(&rail.knots()[i + 1..i + rail.degree() + 1])
            .is_ok_and(|g| parameters_near(g, t, rail.domain()))
    })
}

/// Insert a small symmetric knot neighborhood when an interior section is
/// not already a Greville site. Process sections in order, respecting existing
/// knots and the next section so earlier interpolation sites remain intact.
fn with_section_stations<E: From<GeometryError>>(
    mut rail: NurbsCurve,
    stations: &[Real],
    operations: &mut SweepSurfaceOperations<'_, E>,
) -> Result<NurbsCurve, E> {
    let degree = rail.degree();
    let domain = rail.domain();
    for index in 1..stations.len().saturating_sub(1) {
        let t = stations[index];
        if is_station(&rail, t) {
            continue;
        }
        let left = rail
            .knots()
            .iter()
            .copied()
            .rfind(|&k| k < t)
            .unwrap()
            .max(stations[index - 1]);
        let right = rail
            .knots()
            .iter()
            .copied()
            .find(|&k| k > t)
            .unwrap()
            .min(stations[index + 1]);
        let radius = ((t - left) * 0.5).min((right - t) * 0.5);
        let existing = rail.knots().iter().filter(|&&k| k == t).count();
        let add_center = (degree - existing) % 2;
        let pairs = (degree - existing - add_center) / 2;
        if rail.control_points().len() + degree - existing > MAX_AXIS_CONTROLS {
            return Err(invalid("section-station control budget exceeded").into());
        }
        if add_center != 0 {
            rail = (operations.refine)(&rail, t, existing + 1)?;
        }
        for i in 1..=pairs {
            let offset = radius * (i as Real / pairs as Real);
            let a = t - offset;
            let b = t + offset;
            if !(left < a && a < t && t < b && b < right) {
                return Err(invalid("section-station knots exhausted parameter resolution").into());
            }
            rail = (operations.refine)(&rail, a, 1)?;
            rail = (operations.refine)(&rail, b, 1)?;
        }
    }
    if rail.degree() != degree || rail.domain() != domain {
        return Err(invalid("section refinement changed rail degree or domain").into());
    }
    Ok(rail)
}

pub(super) fn interpolate_sections<E: From<GeometryError>>(
    axis: Axis,
    sections: &[NurbsCurve],
    weights: &[Real],
    operations: &mut SweepSurfaceOperations<'_, E>,
) -> Result<NurbsSurface, E> {
    let count = axis.stations.len();
    let origin = sections[0].control_points()[0].point().to_array();
    let width = sections[0].control_points().len();
    let targets = (0..count * width * 4)
        .map(|i| {
            let row = i / (width * 4);
            let column = i % (width * 4);
            let c = sections[row].control_points()[column / 4];
            if column % 4 == 3 {
                c.weight() * weights[row]
            } else {
                (c.point().to_array()[column % 4] - origin[column % 4]) * c.weight() * weights[row]
            }
        })
        .collect::<Vec<_>>();
    let parameters = axis
        .stations
        .iter()
        .map(|s| s.parameter)
        .collect::<Vec<_>>();
    let fixed_rows = axis
        .stations
        .iter()
        .map(|s| u8::from(s.fixed))
        .collect::<Vec<_>>();
    let input = crate::FixedInterpolationSystem {
        system: crate::NurbsInterpolationSystem {
            degree: axis.degree,
            knots: &axis.knots,
            parameters: &parameters,
            periodic: false,
            columns: width * 4,
            targets: &targets,
        },
        fixed_rows: &fixed_rows,
    };
    let solution =
        crate::surface_rebuild::checked_solve(&input.system, &mut |_| (operations.solve)(&input))?;
    let mut controls = Vec::with_capacity(count * width);
    for j in 0..width {
        for i in 0..count {
            let weight = solution[i * width * 4 + j * 4 + 3];
            let point = if axis.stations[i].fixed {
                sections[i].control_points()[j].point()
            } else {
                Point3::try_from(std::array::from_fn(|k| {
                    solution[i * width * 4 + j * 4 + k] / weight + origin[k]
                }))?
            };
            controls.push(WeightedPoint3::try_new(point, weight)?);
        }
    }
    Ok(NurbsSurface::try_new_rational(
        axis.degree,
        sections[0].degree(),
        count,
        width,
        controls,
        axis.knots,
        sections[0].knots().to_vec(),
    )?)
}

pub(super) fn portable_section_solve(
    input: &crate::FixedInterpolationSystem<'_>,
) -> Result<Vec<Real>, GeometryError> {
    let system = &input.system;
    let axis = Axis::new(system.degree, system.knots.to_vec())?;
    let n = axis.stations.len();
    let rhs = Mat::from_fn(n, system.columns, |i, j| {
        system.targets[i * system.columns + j]
    });
    let solution = axis.solve(rhs)?;
    Ok((0..n * system.columns)
        .map(|i| solution[(i / system.columns, i % system.columns)])
        .collect())
}
