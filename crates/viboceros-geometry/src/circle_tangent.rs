//! Contact-based circular constraints. Curves retain their analytic parameters.
use crate::{
    Circle3, CircularArc3, CurveRef, Frame3, GeometryError, Point3, Real, Tolerance, Vector3,
};
use nalgebra::{DMatrix, DVector, Vector3 as V3};

#[derive(Clone, Copy, Debug)]
pub enum CircleTangentConstraint<'a> {
    Point(Point3),
    Curve {
        curve: CurveRef<'a>,
        parameter: Real,
        fixed: bool,
    },
}
#[derive(Clone, Debug)]
pub struct CircleTangentSolution {
    pub circle: Circle3,
    /// Contact parameters in input order; free points have no parameter.
    pub parameters: Vec<Option<Real>>,
}
fn failure() -> GeometryError {
    GeometryError::Degenerate {
        context: "tangent circle constraints did not converge",
    }
}
fn v(p: Point3) -> V3<Real> {
    V3::from(p.to_array())
}
fn vector(v: V3<Real>) -> Result<Vector3, GeometryError> {
    Vector3::try_from([v.x, v.y, v.z])
}

fn closed_parameter(t: Real) -> Real {
    if t == 1. { 1. } else { t.rem_euclid(1.) }
}

struct Sample {
    point: V3<Real>,
    tangent: V3<Real>,
    derivative: V3<Real>,
    tangent_derivative: V3<Real>,
}

struct Contact<'a> {
    source: CircleTangentConstraint<'a>,
    seed: Real,
    parameter_slot: Option<usize>,
}
struct Problem<'a> {
    contacts: Vec<Contact<'a>>,
    origin: Point3,
    scale: Real,
    radius: Option<Real>,
}
impl Problem<'_> {
    fn sample(
        &self,
        contact: &Contact<'_>,
        state: &DVector<Real>,
    ) -> Result<Sample, GeometryError> {
        let CircleTangentConstraint::Curve { curve, .. } = contact.source else {
            let CircleTangentConstraint::Point(p) = contact.source else {
                unreachable!()
            };
            return Ok(Sample {
                point: (v(p) - v(self.origin)) / self.scale,
                tangent: V3::zeros(),
                derivative: V3::zeros(),
                tangent_derivative: V3::zeros(),
            });
        };
        let t = contact.parameter_slot.map_or(contact.seed, |i| state[i]);
        let domain = curve.domain();
        let width = *domain.end() - *domain.start();
        let parameter = if curve.is_closed()? {
            curve.parameter_at(closed_parameter(t))?
        } else {
            (*domain.start()) + width * t
        };
        let (point, first, second) = if let CurveRef::Line(line) = curve {
            let d = line.start().vector_to(line.end())?;
            (
                line.start().translated(d.scaled(t)?)?,
                d,
                Vector3::try_new(0., 0., 0.)?,
            )
        } else {
            let (p, a, b) = curve.evaluate_with_second_derivative(parameter)?;
            (p, a.scaled(width)?, b.scaled(width)?.scaled(width)?)
        };
        let d = V3::from(first.to_array()) / self.scale;
        let speed = d.norm();
        if speed == 0. || !speed.is_finite() {
            return Err(failure());
        }
        let tangent = d / speed;
        let dd = V3::from(second.to_array()) / self.scale;
        let dt = (dd - tangent * tangent.dot(&dd)) / speed;
        Ok(Sample {
            point: (v(point) - v(self.origin)) / self.scale,
            tangent,
            derivative: d,
            tangent_derivative: dt,
        })
    }
    fn system(
        &self,
        state: &DVector<Real>,
    ) -> Result<(DVector<Real>, DMatrix<Real>), GeometryError> {
        let center = V3::new(state[0], state[1], state[2]);
        let r = state[3];
        let n = V3::new(state[4], state[5], state[6]);
        let count = 1
            + usize::from(self.radius.is_some())
            + self
                .contacts
                .iter()
                .map(|c| {
                    if matches!(c.source, CircleTangentConstraint::Curve { .. }) {
                        4
                    } else {
                        2
                    }
                })
                .sum::<usize>();
        let mut f = DVector::zeros(count);
        let mut j = DMatrix::zeros(count, state.len());
        let mut row = 0;
        f[row] = (n.norm_squared() - 1.) * 0.5;
        for k in 0..3 {
            j[(row, 4 + k)] = n[k];
        }
        row += 1;
        if let Some(radius) = self.radius {
            f[row] = r - radius / self.scale;
            j[(row, 3)] = 1.;
            row += 1;
        }
        for contact in &self.contacts {
            let Sample {
                point,
                tangent,
                derivative: d,
                tangent_derivative: dt,
            } = self.sample(contact, state)?;
            let radial = center - point;
            f[row] = (radial.norm_squared() - r * r) * 0.5;
            for k in 0..3 {
                j[(row, k)] = radial[k];
            }
            j[(row, 3)] = -r;
            if let Some(i) = contact.parameter_slot {
                j[(row, i)] = -radial.dot(&d);
            }
            row += 1;
            f[row] = radial.dot(&n);
            for k in 0..3 {
                j[(row, k)] = n[k];
                j[(row, 4 + k)] = radial[k];
            }
            if let Some(i) = contact.parameter_slot {
                j[(row, i)] = -d.dot(&n);
            }
            row += 1;
            if matches!(contact.source, CircleTangentConstraint::Curve { .. }) {
                f[row] = radial.dot(&tangent);
                for k in 0..3 {
                    j[(row, k)] = tangent[k];
                }
                if let Some(i) = contact.parameter_slot {
                    j[(row, i)] = -d.dot(&tangent) + radial.dot(&dt);
                }
                row += 1;
                f[row] = tangent.dot(&n);
                for k in 0..3 {
                    j[(row, 4 + k)] = tangent[k];
                }
                if let Some(i) = contact.parameter_slot {
                    j[(row, i)] = dt.dot(&n);
                }
                row += 1;
            }
        }
        Ok((f, j))
    }
    fn solve(&self, mut state: DVector<Real>) -> Result<Option<DVector<Real>>, GeometryError> {
        let mut current = self.system(&state)?;
        for _ in 0..80 {
            if current.0.amax() < 2e-13 {
                return Ok(Some(state));
            }
            let mut chart = DVector::from_element(state.len(), 1.);
            for c in &self.contacts {
                if let Some(i) = c.parameter_slot {
                    chart[i] = 1. / self.sample(c, &state)?.derivative.norm();
                }
            }
            let scaled = &current.1 * DMatrix::from_diagonal(&chart);
            let decomposition = scaled.svd(true, true);
            let Ok(mut step) = decomposition.solve(&(-&current.0), 1e-12) else {
                return Ok(None);
            };
            let length = step.norm();
            if !length.is_finite() {
                return Ok(None);
            }
            if length > 2. {
                step *= 2. / length;
            }
            step.component_mul_assign(&chart);
            let cost = current.0.norm_squared();
            let mut improved = false;
            for _ in 0..20 {
                let candidate = &state + &step;
                if candidate.iter().all(|x| x.is_finite())
                    && candidate[3] > 1e-14
                    && let Ok(next) = self.system(&candidate)
                    && next.0.norm_squared() < cost
                {
                    state = candidate;
                    current = next;
                    improved = true;
                    break;
                }
                step *= 0.5;
            }
            if !improved {
                return Ok(None);
            }
        }
        Ok(None)
    }
}

/// Solve two or three point/tangent contacts without changing source curves.
/// `fixed` keeps the supplied native contact parameter. A locked radius and
/// all contact equations are verified before returning a circle. Open curves
/// stay in their domains; analytic line supports may extend beyond their ends.
pub fn try_circle_tangent(
    constraints: &[CircleTangentConstraint<'_>],
    radius: Option<Real>,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Option<CircleTangentSolution>, GeometryError> {
    if !(2..=3).contains(&constraints.len()) {
        return Err(failure());
    }
    if radius.is_some_and(|r| !r.is_finite() || r <= tolerance.absolute()) {
        return Err(failure());
    }
    let mut constraints = constraints.to_vec();
    let parallel_lines = if let [
        CircleTangentConstraint::Curve { curve: a, .. },
        CircleTangentConstraint::Curve { curve: b, .. },
    ] = constraints.as_slice()
    {
        if a.to_nurbs()?.is_linear(Tolerance::NUMERICAL_VALIDATION)?
            && b.to_nurbs()?.is_linear(Tolerance::NUMERICAL_VALIDATION)?
        {
            let ta = a
                .evaluate_with_tangent(*a.domain().start())?
                .tangent()
                .as_vector();
            let tb = b
                .evaluate_with_tangent(*b.domain().start())?
                .tangent()
                .as_vector();
            ta.cross(tb)?.length()? < tolerance.angular()
        } else {
            false
        }
    } else {
        false
    };
    if parallel_lines
        && radius.is_some()
        && let Some(CircleTangentConstraint::Curve { fixed, .. }) = constraints.last_mut()
    {
        *fixed = true;
    }
    if constraints.len() == 2
        && radius.is_none()
        && !constraints
            .iter()
            .any(|c| matches!(c, CircleTangentConstraint::Curve { fixed: true, .. }))
    {
        match constraints.as_mut_slice() {
            [
                CircleTangentConstraint::Curve {
                    curve,
                    parameter,
                    fixed,
                },
                CircleTangentConstraint::Point(p),
            ] => {
                *parameter = if let CurveRef::Line(line) = *curve {
                    let d = line.start().vector_to(line.end())?;
                    let f = line.start().vector_to(*p)?.dot(d)? / d.dot(d)?;
                    let domain = line.domain();
                    *domain.start() + f * (*domain.end() - *domain.start())
                } else {
                    curve.closest_parameter(*p, tolerance)?
                };
                *fixed = true;
            }
            [_, CircleTangentConstraint::Curve { fixed, .. }] => *fixed = true,
            _ => {}
        }
    }
    let mut points = Vec::new();
    let mut tangents = Vec::new();
    for c in &constraints {
        match *c {
            CircleTangentConstraint::Point(p) => {
                points.push(p);
                tangents.push(None);
            }
            CircleTangentConstraint::Curve {
                curve, parameter, ..
            } => {
                if let CurveRef::Line(line) = curve {
                    let d = line.start().vector_to(line.end())?;
                    let domain = line.domain();
                    points.push(line.start().translated(d.scaled(
                        (parameter - *domain.start()) / (*domain.end() - *domain.start()),
                    )?)?);
                    tangents.push(Some(d.normalized_nonzero()?.as_vector()));
                } else {
                    let p = curve.evaluate_with_tangent(parameter)?;
                    points.push(p.point());
                    tangents.push(Some(p.tangent().as_vector()));
                }
            }
        }
    }
    if constraints
        .iter()
        .all(|c| matches!(c, CircleTangentConstraint::Point(_)))
    {
        let c = if let [a, b, c] = points.as_slice() {
            Circle3::try_from_three_points(*a, *b, *c, tolerance)?
        } else {
            let diameter =
                Circle3::try_from_diameter_on_plane(points[0], points[1], plane, tolerance)?;
            let c = if let Some(radius) = radius {
                let chord = points[0].vector_to(points[1])?.normalized_nonzero()?;
                let side = diameter.normal()?.as_vector().cross(chord.as_vector())?;
                let direction = diameter.center().translated(side.scaled(radius)?)?;
                Circle3::try_from_two_points_radius_direction(
                    points[0], points[1], radius, direction, tolerance,
                )?
            } else {
                diameter
            };
            Circle3::try_from_frame(
                c.center(),
                c.radius(),
                c.center().vector_to(points[0])?.normalized_nonzero()?,
                c.normal()?,
                tolerance,
            )?
        };
        if radius.is_some_and(|r| (r - c.radius()).abs() > tolerance.absolute()) {
            return Ok(None);
        }
        return Ok(Some(CircleTangentSolution {
            circle: c,
            parameters: vec![None; constraints.len()],
        }));
    }
    let origin = points[0];
    let scale = points
        .iter()
        .map(|p| origin.distance_to(*p))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .fold(radius.unwrap_or(0.), Real::max);
    if scale <= tolerance.absolute() || !scale.is_finite() {
        return Ok(None);
    }
    let normalized = points
        .iter()
        .map(|p| (v(*p) - v(origin)) / scale)
        .collect::<Vec<_>>();
    let mut normal = if let [a, b, c] = normalized.as_slice() {
        (b - a).cross(&(c - a))
    } else {
        V3::zeros()
    };
    if normal.norm() < 1e-12 {
        for t in tangents.iter().flatten() {
            normal = (normalized[1] - normalized[0]).cross(&V3::from(t.to_array()));
            if normal.norm() > 1e-12 {
                break;
            }
        }
    }
    if normal.norm() < 1e-12 {
        normal = V3::from(plane.z_axis().as_vector().to_array());
    }
    normal.normalize_mut();
    let seed = if let [a, b, c] = points.as_slice() {
        Circle3::try_from_three_points(*a, *b, *c, Tolerance::NUMERICAL_VALIDATION).ok()
    } else {
        constraints
            .iter()
            .position(|c| matches!(c, CircleTangentConstraint::Curve { fixed: true, .. }))
            .and_then(|i| {
                CircularArc3::try_from_start_tangent_end(
                    points[i],
                    tangents[i]?.normalized_nonzero().ok()?,
                    points[1 - i],
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .ok()
                .and_then(|a| {
                    Circle3::try_from_frame(
                        a.center(),
                        a.radius(),
                        a.x_axis(),
                        a.normal().ok()?,
                        Tolerance::NUMERICAL_VALIDATION,
                    )
                    .ok()
                })
            })
    };
    let mut center = seed.map_or((normalized[0] + normalized[1]) * 0.5, |c| {
        (v(c.center()) - v(origin)) / scale
    });
    // Intersect the seed normal lines in least squares when all contacts are
    // tangencies. This also gives a stable start for parallel supports.
    if seed.is_none() && tangents.iter().all(Option::is_some) {
        let mut a = DMatrix::zeros(3 * points.len(), 3);
        let mut b = DVector::zeros(3 * points.len());
        for (i, (p, t)) in normalized.iter().zip(&tangents).enumerate() {
            let direction = normal.cross(&V3::from(t.unwrap().to_array())).normalize();
            let projector = nalgebra::Matrix3::identity() - direction * direction.transpose();
            for j in 0..3 {
                for k in 0..3 {
                    a[(3 * i + j, k)] = projector[(j, k)];
                }
                b[3 * i + j] = (projector * p)[j];
            }
        }
        if let Ok(c) = a.svd(true, true).solve(&b, 1e-12) {
            center = V3::new(c[0], c[1], c[2]);
        }
    }
    // A radius-locked corner pick can seed the center exactly on both
    // supports. Its distance Jacobian then has zero center columns. Move
    // into a tangent-normal sector before Newton; acceptance still requires
    // every original contact and radius equation.
    if let Some(radius) = radius
        && tangents.iter().all(Option::is_some)
        && normalized.iter().any(|p| (center - p).norm() < 1e-10)
    {
        let side = tangents
            .iter()
            .flatten()
            .map(|t| normal.cross(&V3::from(t.to_array())))
            .sum::<V3<Real>>();
        if side.norm() > 1e-10 {
            center += side.normalize() * (radius / scale);
        }
    }
    let mut slots = 7;
    let contacts = constraints
        .iter()
        .map(|source| {
            let (seed, variable) = match *source {
                CircleTangentConstraint::Curve {
                    curve,
                    parameter,
                    fixed,
                } => {
                    let d = curve.domain();
                    ((parameter - *d.start()) / (*d.end() - *d.start()), !fixed)
                }
                CircleTangentConstraint::Point(_) => (0., false),
            };
            let parameter_slot = variable.then(|| {
                let slot = slots;
                slots += 1;
                slot
            });
            Contact {
                source: *source,
                seed,
                parameter_slot,
            }
        })
        .collect::<Vec<_>>();
    let mut state = DVector::zeros(slots);
    for k in 0..3 {
        state[k] = center[k];
        state[4 + k] = normal[k];
    }
    state[3] = radius
        .map_or_else(
            || normalized.iter().map(|p| (center - p).norm()).sum::<Real>() / points.len() as Real,
            |r| r / scale,
        )
        .max(1e-6);
    for c in &contacts {
        if let Some(i) = c.parameter_slot {
            state[i] = c.seed;
        }
    }
    let problem = Problem {
        contacts,
        origin,
        scale,
        radius,
    };
    let Some(state) = problem.solve(state)? else {
        return Ok(None);
    };
    let center = origin.translated(vector(V3::new(state[0], state[1], state[2]) * scale)?)?;
    let r = state[3] * scale;
    let samples = problem
        .contacts
        .iter()
        .map(|c| problem.sample(c, &state))
        .collect::<Result<Vec<_>, _>>()?;
    let contacts = samples
        .iter()
        .map(|sample| origin.translated(vector(sample.point * scale)?))
        .collect::<Result<Vec<_>, _>>()?;
    let fixed = constraints
        .iter()
        .position(|c| matches!(c, CircleTangentConstraint::Curve { fixed: true, .. }));
    let seam = if !parallel_lines
        && constraints.len() == 2
        && constraints
            .iter()
            .all(|c| matches!(c, CircleTangentConstraint::Curve { .. }))
    {
        fixed.unwrap_or(0)
    } else {
        0
    };
    let x = center.vector_to(contacts[seam])?.normalized_nonzero()?;
    let n = if parallel_lines {
        center
            .vector_to(contacts[0])?
            .cross(vector(samples[0].tangent)?)?
    } else if let Some(i) = fixed {
        center
            .vector_to(contacts[i])?
            .cross(vector(samples[i].tangent)?)?
    } else if contacts.len() == 3 {
        contacts[0]
            .vector_to(contacts[1])?
            .cross(contacts[0].vector_to(contacts[2])?)?
    } else {
        center
            .vector_to(contacts[0])?
            .cross(center.vector_to(contacts[1])?)?
    };
    let n = n
        .normalized_nonzero()
        .map(|n| n.as_vector())
        .unwrap_or(vector(V3::new(state[4], state[5], state[6]))?);
    let circle = Circle3::try_from_frame(center, r, x, n.normalized_nonzero()?, tolerance)?;
    let normal = V3::from(circle.normal()?.as_vector().to_array());
    let distance_tolerance = tolerance.absolute().max(128. * Real::EPSILON * scale);
    if radius.is_some_and(|target| (target - r).abs() > distance_tolerance) {
        return Ok(None);
    }
    for (sample, contact) in samples.iter().zip(&problem.contacts) {
        let tangent = &sample.tangent;
        let radial = (v(center) - v(origin)) / scale - sample.point;
        if ((radial.norm() - r / scale) * scale).abs() > distance_tolerance
            || radial.dot(&normal).abs() * scale > distance_tolerance
        {
            return Ok(None);
        }
        if matches!(contact.source, CircleTangentConstraint::Curve { .. })
            && (radial.dot(tangent).abs() * scale > distance_tolerance
                || tangent.dot(&normal).abs() > tolerance.angular().max(128. * Real::EPSILON))
        {
            return Ok(None);
        }
    }
    let parameters = problem
        .contacts
        .iter()
        .map(|c| match c.source {
            CircleTangentConstraint::Point(_) => None,
            CircleTangentConstraint::Curve { curve, .. } => {
                let d = curve.domain();
                let t = c.parameter_slot.map_or(c.seed, |i| state[i]);
                Some(
                    (*d.start())
                        + (*d.end() - *d.start())
                            * if curve.is_closed().unwrap_or(false) {
                                closed_parameter(t)
                            } else {
                                t
                            },
                )
            }
        })
        .collect();
    Ok(Some(CircleTangentSolution { circle, parameters }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LineSegment, UnitVector3};
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn line(a: Point3, b: Point3) -> LineSegment {
        LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap()
    }
    fn plane() -> Frame3 {
        Frame3::try_from_normal(
            p(0., 0., 0.),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }
    fn curve(c: &LineSegment, t: Real, fixed: bool) -> CircleTangentConstraint<'_> {
        CircleTangentConstraint::Curve {
            curve: CurveRef::Line(c),
            parameter: t,
            fixed,
        }
    }
    #[test]
    fn tangent_circle_line_contacts_radius_fixed_last_and_three_constraints() {
        let a = line(p(0., 0., 0.), p(12., 0., 0.));
        let b = line(p(0., 0., 0.), p(0., 12., 0.));
        let c = line(p(0., 10., 0.), p(10., 0., 0.));
        let cases = [
            (
                vec![curve(&a, 3., false), curve(&b, 4., false)],
                None,
                p(4., 4., 0.),
                4.,
            ),
            (
                vec![curve(&a, 3., true), curve(&b, 4., false)],
                None,
                p(3., 3., 0.),
                3.,
            ),
            (
                vec![curve(&a, 3., false), curve(&b, 4., false)],
                Some(3.),
                p(3., 3., 0.),
                3.,
            ),
            (
                vec![
                    curve(&a, 3., false),
                    curve(&b, 4., false),
                    curve(&c, 4., false),
                ],
                None,
                p(10. - 5. * 2f64.sqrt(), 10. - 5. * 2f64.sqrt(), 0.),
                10. - 5. * 2f64.sqrt(),
            ),
            (
                vec![
                    curve(&a, 3., false),
                    CircleTangentConstraint::Point(p(0., 4., 0.)),
                ],
                None,
                p(0., 2., 0.),
                2.,
            ),
            (
                vec![
                    CircleTangentConstraint::Point(p(3., 0., 0.)),
                    curve(&b, 4., false),
                ],
                None,
                p(25. / 6., 4., 0.),
                25. / 6.,
            ),
        ];
        for (constraints, radius, center, r) in cases {
            let sol = try_circle_tangent(&constraints, radius, plane(), Tolerance::DEFAULT)
                .unwrap()
                .expect("circle");
            assert!(
                sol.circle.center().distance_to(center).unwrap() < 1e-8,
                "{:?} != {center:?}",
                sol.circle
            );
            assert!((sol.circle.radius() - r).abs() < 1e-8);
        }
    }
    #[test]
    fn tangent_circle_spatial_three_line_contacts_and_parameter_scaling() {
        let frame = Frame3::try_from_normal(
            p(7., 5., 9.),
            Vector3::try_new(2., -3., 4.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let circle = Circle3::try_from_frame(
            frame.origin(),
            3.,
            frame.x_axis(),
            frame.z_axis(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let lines = (0..3)
            .map(|i| {
                let a = std::f64::consts::TAU * i as Real / 3.;
                let point = circle.point_at_angle(a).unwrap();
                let t = CurveRef::Circle(&circle)
                    .evaluate_with_tangent(a * 3.)
                    .unwrap()
                    .tangent()
                    .as_vector();
                line(
                    point.translated(t.scaled(-5.).unwrap()).unwrap(),
                    point.translated(t.scaled(5.).unwrap()).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let constraints = lines
            .iter()
            .map(|l| curve(l, 6., false))
            .collect::<Vec<_>>();
        let sol = try_circle_tangent(&constraints, None, frame, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(sol.circle.center().distance_to(circle.center()).unwrap() < 1e-8);
        assert!((sol.circle.radius() - 3.).abs() < 1e-8);
    }
    #[test]
    fn tangent_circle_curved_supports_and_a_free_point() {
        let a = Circle3::try_new(
            p(-4., 0., 0.),
            1.,
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let b =
            Circle3::try_new(p(4., 0., 0.), 1., a.normal().unwrap(), Tolerance::DEFAULT).unwrap();
        let t1 = CurveRef::Circle(&a)
            .closest_parameter(p(-3.2, 0.6, 0.), Tolerance::DEFAULT)
            .unwrap();
        let t2 = CurveRef::Circle(&b)
            .closest_parameter(p(3.2, 0.6, 0.), Tolerance::DEFAULT)
            .unwrap();
        let constraints = [
            CircleTangentConstraint::Curve {
                curve: CurveRef::Circle(&a),
                parameter: t1,
                fixed: false,
            },
            CircleTangentConstraint::Curve {
                curve: CurveRef::Circle(&b),
                parameter: t2,
                fixed: false,
            },
        ];
        let sol = try_circle_tangent(&constraints, Some(4.), plane(), Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(sol.circle.center().distance_to(p(0., 3., 0.)).unwrap() < 1e-8);
        let parabola = crate::NurbsCurve::try_clamped_uniform(
            2,
            vec![p(-4., 4., 0.), p(0., -4., 0.), p(4., 4., 0.)],
        )
        .unwrap();
        let constraints = [
            CircleTangentConstraint::Curve {
                curve: CurveRef::NurbsCurve(&parabola),
                parameter: 0.4,
                fixed: false,
            },
            CircleTangentConstraint::Point(p(1.5, 1.5, 0.)),
        ];
        let sol = try_circle_tangent(&constraints, Some(1.5), plane(), Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(sol.circle.center().distance_to(p(0., 1.5, 0.)).unwrap() < 1e-8);
    }

    #[test]
    fn tangent_circle_rejects_inconsistent_radius_and_preserves_closed_end_tangent() {
        let points =
            [p(3., 0., 0.), p(0., 3., 0.), p(-3., 0., 0.)].map(CircleTangentConstraint::Point);
        assert!(
            try_circle_tangent(&points, Some(2.), plane(), Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
        let a = line(p(0., 0., 0.), p(12., 0., 0.));
        let b = line(p(0., 6., 0.), p(12., 6., 0.));
        assert!(
            try_circle_tangent(
                &[curve(&a, 3., false), curve(&b, 4., false)],
                Some(2.),
                plane(),
                Tolerance::DEFAULT
            )
            .unwrap()
            .is_none()
        );
        let polyline = crate::Polyline3::try_new(
            vec![p(0., 0., 0.), p(6., 0., 0.), p(0., 6., 0.), p(0., 0., 0.)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = CurveRef::Polyline(&polyline);
        let end = *source.domain().end();
        let solution = try_circle_tangent(
            &[
                CircleTangentConstraint::Curve {
                    curve: source,
                    parameter: end,
                    fixed: true,
                },
                CircleTangentConstraint::Point(p(4., 0., 0.)),
            ],
            None,
            plane(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .unwrap();
        assert!(solution.circle.center().distance_to(p(2., 0., 0.)).unwrap() < 1e-8);
        assert_eq!(solution.parameters[0], Some(end));
        assert!(
            solution
                .circle
                .normal()
                .unwrap()
                .as_vector()
                .dot(Vector3::try_new(0., 0., 1.).unwrap())
                .unwrap()
                > 1. - 1e-12
        );
    }

    #[test]
    fn tangent_circle_radius_locked_corner_picks_escape_singular_seed() {
        let a = line(p(0., 0., 0.), p(12., 0., 0.));
        let b = line(p(0., 0., 0.), p(0., 12., 0.));
        let solution = try_circle_tangent(
            &[curve(&a, 0., false), curve(&b, 0., false)],
            Some(3.),
            plane(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .unwrap();
        let center = solution.circle.center();
        assert!((center.x().abs() - 3.).abs() < 1e-8);
        assert!((center.y().abs() - 3.).abs() < 1e-8);
        assert!((solution.circle.radius() - 3.).abs() < 1e-8);
    }

    #[test]
    fn tangent_circle_normalizes_small_and_large_translated_problems() {
        for (scale, offset, tolerance) in [(1e-5, 0., 1e-10), (1., 0., 1e-9), (1e4, 1e7, 1e-6)] {
            let p = |x, y| p(offset + x * scale, -offset + y * scale, 0.);
            let tolerance = Tolerance::try_new(tolerance, 1e-12, 1e-10).unwrap();
            let a = line(p(0., 0.), p(12., 0.));
            let b = line(p(0., 0.), p(0., 12.));
            let sol = try_circle_tangent(
                &[curve(&a, 3. * scale, false), curve(&b, 4. * scale, false)],
                Some(3. * scale),
                plane(),
                tolerance,
            )
            .unwrap()
            .unwrap();
            assert!(sol.circle.center().distance_to(p(3., 3.)).unwrap() <= tolerance.absolute());
            assert!((sol.circle.radius() - 3. * scale).abs() <= tolerance.absolute());
        }
    }
}
