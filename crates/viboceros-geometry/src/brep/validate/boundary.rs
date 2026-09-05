//! Sampled geometric correspondence, independent of edge/p-curve parameterization.

use super::*;
use crate::ParameterSide;

pub(super) fn validate(
    brep: &Brep,
    face: &BrepFace,
    trim: &BrepTrim,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    // Reuse the stable sided rational evaluator for (u,v,0). This temporary
    // representation never turns parameter-space coordinates into model space.
    let parameters = NurbsCurve::try_new_rational(
        trim.curve.degree(),
        trim.curve
            .control_points()
            .iter()
            .map(|cp| {
                WeightedPoint3::try_new(
                    Point3::try_new(cp.point().x(), cp.point().y(), 0.0)?,
                    cp.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        trim.curve.knots().to_vec(),
    )?;
    let image = LiftedTrim {
        curve: &parameters,
        surface: &face.surface,
    };
    let allowed_uv = [
        tolerance.absolute().max(trim.tolerance[0]),
        tolerance.absolute().max(trim.tolerance[1]),
        0.0,
    ];
    continuous(&parameters, |delta| {
        (0..3).all(|axis| delta[axis].abs() <= allowed_uv[axis])
    })?;
    let stations = samples(&parameters)?;
    let mut lifted = Vec::with_capacity(stations.len());
    for (parameter, side) in stations {
        lifted.push((parameter, image.point(parameter, side)?));
    }
    let Some(edge_index) = trim.edge else {
        let vertex = brep.vertices[trim.vertices[0]];
        let allowed = tolerance.absolute().max(vertex.tolerance);
        for &(_, point) in &lifted {
            if point.distance_to(vertex.point)? > allowed {
                return invalid("a singular trim interior leaves its model-space vertex");
            }
        }
        return Ok(());
    };
    let edge = &brep.edges[edge_index];
    let allowed = tolerance.absolute().max(edge.tolerance);
    let search_tolerance = Tolerance::try_new(
        (allowed * 0.125).max(Real::MIN_POSITIVE),
        tolerance.relative(),
        tolerance.angular(),
    )?;
    for &(parameter, point) in &lifted {
        let mut fraction = normalized(parameter, parameters.domain())?;
        if trim.reversed_3d {
            fraction = 1.0 - fraction;
        }
        let direct = edge.curve.evaluate(edge.curve.parameter_at(fraction)?)?;
        if point.distance_to(direct)? <= allowed {
            continue;
        }
        let closest = edge.curve.closest_parameter(point, search_tolerance)?;
        if point.distance_to(edge.curve.evaluate(closest)?)? > allowed {
            return invalid("a p-curve interior leaves its model-space edge");
        }
    }
    // The opposite direction catches extra edge excursions even when the
    // entire trim locus is contained in the edge's locus.
    for (parameter, side) in samples(&edge.curve)? {
        let point = edge.curve.evaluate_on_side(parameter, side)?;
        let mut fraction = normalized(parameter, edge.curve.domain())?;
        if trim.reversed_3d {
            fraction = 1.0 - fraction;
        }
        let direct = image.point(parameters.parameter_at(fraction)?, ParameterSide::Right)?;
        if point.distance_to(direct)? <= allowed {
            continue;
        }
        if image.closest_distance(point, &lifted, search_tolerance.absolute())? > allowed {
            return invalid("a model-space edge interior leaves its lifted p-curve");
        }
    }
    Ok(())
}

pub(super) fn continuous(
    curve: &NurbsCurve,
    near: impl Fn([Real; 3]) -> bool,
) -> Result<(), GeometryError> {
    let domain = curve.domain();
    for group in curve.knots().chunk_by(|a, b| a == b) {
        let parameter = group[0];
        if group.len() > curve.degree() && parameter > *domain.start() && parameter < *domain.end()
        {
            let left = curve
                .evaluate_on_side(parameter, ParameterSide::Left)?
                .to_array();
            let right = curve
                .evaluate_on_side(parameter, ParameterSide::Right)?
                .to_array();
            if !near(std::array::from_fn(|axis| left[axis] - right[axis])) {
                return invalid("a B-rep boundary curve contains a positional jump");
            }
        }
    }
    Ok(())
}

fn samples(curve: &NurbsCurve) -> Result<Vec<(Real, ParameterSide)>, GeometryError> {
    let mut samples = Vec::new();
    for (start, end) in curve.spans() {
        for i in 0..=8 {
            let fraction = i as Real / 8.0;
            for fraction in [
                fraction,
                0.5 * (1.0 - (std::f64::consts::PI * fraction).cos()),
            ] {
                let parameter = start.mul_add(1.0 - fraction, end * fraction);
                require_finite([parameter], "B-rep boundary validation parameter")?;
                let side = if parameter == end {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                };
                samples.push((parameter, side));
            }
        }
    }
    samples.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| (a.1 == ParameterSide::Left).cmp(&(b.1 == ParameterSide::Left)))
    });
    samples.dedup();
    Ok(samples)
}

fn normalized(parameter: Real, domain: RangeInclusive<Real>) -> Result<Real, GeometryError> {
    let (start, end) = (*domain.start(), *domain.end());
    let fraction = if (end - start).is_finite() {
        (parameter - start) / (end - start)
    } else {
        let scale = start.abs().max(end.abs());
        (parameter / scale - start / scale) / (end / scale - start / scale)
    };
    require_finite([fraction], "B-rep boundary parameter fraction")?;
    Ok(fraction.clamp(0.0, 1.0))
}

struct LiftedTrim<'a> {
    curve: &'a NurbsCurve,
    surface: &'a NurbsSurface,
}

impl LiftedTrim<'_> {
    fn point(&self, parameter: Real, side: ParameterSide) -> Result<Point3, GeometryError> {
        let uv = self.curve.evaluate_on_side(parameter, side)?;
        self.surface.evaluate(uv.x(), uv.y())
    }

    fn jet(&self, parameter: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (uv, derivative) = self.curve.evaluate_with_derivative(parameter)?;
        let (point, du, dv) = self.surface.evaluate_with_derivatives(uv.x(), uv.y())?;
        let tangent = Vector3::try_from(std::array::from_fn(|axis| {
            du.to_array()[axis].mul_add(derivative.x(), dv.to_array()[axis] * derivative.y())
        }))?;
        Ok((point, tangent))
    }

    fn closest_distance(
        &self,
        target: Point3,
        samples: &[(Real, Point3)],
        epsilon: Real,
    ) -> Result<Real, GeometryError> {
        let mut candidates = samples
            .iter()
            .map(|&(t, point)| Ok((point.distance_to(target)?, t)))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        candidates.truncate(16);
        let mut best = candidates[0].0;
        let domain = self.curve.domain();
        for (mut distance, mut parameter) in candidates {
            for _ in 0..64 {
                if distance <= epsilon {
                    return Ok(distance);
                }
                let (point, tangent) = self.jet(parameter)?;
                let speed = tangent.length()?;
                if speed == 0.0 {
                    break;
                }
                let projection = point.vector_to(target)?.dot(tangent)? / speed;
                if projection.abs() <= epsilon {
                    break;
                }
                let delta = projection / speed;
                if !delta.is_finite() {
                    break;
                }
                let mut accepted = None;
                let mut step: Real = 1.0;
                for _ in 0..24 {
                    let next = step
                        .mul_add(delta, parameter)
                        .clamp(*domain.start(), *domain.end());
                    if next == parameter {
                        break;
                    }
                    let next_distance = self
                        .point(next, ParameterSide::Right)?
                        .distance_to(target)?;
                    if next_distance < distance {
                        accepted = Some((next, next_distance));
                        break;
                    }
                    step *= 0.5;
                }
                let Some((next, next_distance)) = accepted else {
                    break;
                };
                parameter = next;
                distance = next_distance;
            }
            best = best.min(distance);
        }
        Ok(best)
    }
}
