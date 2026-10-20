//! Unit-speed geodesic shooting on regular NURBS charts.
//!
//! This is a surface-distance building block, not a complete offset-curve
//! producer. Natural boundaries, singularities and nonsmooth chart transitions
//! fail explicitly. Embedded integration estimates are not global certificates.
use crate::{
    GeometryError, NurbsSurface, ParameterSide, Point2, Point3, Real, SurfaceJet2, Tolerance,
    UnitVector3, Vector3, WeightedPoint3, integration::cash_karp,
};

#[derive(Clone, Copy, Debug)]
pub struct SurfaceGeodesicPoint {
    pub point: Point3,
    pub uv: Point2,
    /// Direction of travel; negative distances reverse the initial direction.
    pub tangent: UnitVector3,
    /// Local embedded estimates plus endpoint encoding residuals, in model units.
    pub estimated_error: Real,
    pub accepted_steps: usize,
    pub attempted_steps: usize,
    /// Maximum arc-length speed drift on the normalized travel interval [0,1].
    pub maximum_speed_error: Real,
}

/// Prepare once for repeated shots. Both parameter axes and physical coordinates
/// are normalized before differentiating; the original surface stays immutable.
pub struct PreparedSurfaceGeodesic<'a> {
    original: &'a NurbsSurface,
    surface: NurbsSurface,
    scale: Real,
    closed: [bool; 2],
    breaks: [Vec<Real>; 2],
}

fn invalid(context: &'static str) -> GeometryError {
    GeometryError::InvalidSurfaceGeodesic { context }
}
fn v(values: [Real; 3]) -> Result<Vector3, GeometryError> {
    Vector3::try_from(values)
}
fn combine(a: Vector3, x: Real, b: Vector3, y: Real) -> Result<Vector3, GeometryError> {
    let a = a.to_array();
    let b = b.to_array();
    v(std::array::from_fn(|i| a[i] * x + b[i] * y))
}

fn chart_closes(surface: &NurbsSurface, axis: usize) -> Result<bool, GeometryError> {
    let (a, b) = if axis == 0 {
        (surface.isocurve_v(0.)?, surface.isocurve_v(1.)?)
    } else {
        (surface.isocurve_u(0.)?, surface.isocurve_u(1.)?)
    };
    if a.degree() != b.degree()
        || a.knots() != b.knots()
        || a.control_points().len() != b.control_points().len()
    {
        return Ok(false);
    }
    // Object closure predicates admit small gaps. A geodesic chart must not
    // turn a thin open strip into a periodic surface based on its model size.
    Ok(a.control_points()
        .iter()
        .zip(b.control_points())
        .all(|(x, y)| x.point() == y.point() && x.weight() == y.weight()))
}

/// A thin QR solve avoids squaring the surface metric's condition number.
fn solve(jet: SurfaceJet2, target: Vector3) -> Result<[Real; 2], GeometryError> {
    let x = jet.derivative_u.normalized_nonzero()?;
    let n = x
        .as_vector()
        .cross(jet.derivative_v.normalized_nonzero()?.as_vector())?
        .normalized_nonzero()?;
    let y = n.as_vector().cross(x.as_vector())?;
    let c = jet.derivative_v.dot(y)?;
    if c == 0. {
        return Err(invalid("singular surface metric"));
    }
    let dv = target.dot(y)? / c;
    let du = (target.dot(x.as_vector())? - jet.derivative_v.dot(x.as_vector())? * dv)
        / jet.derivative_u.length()?;
    if !du.is_finite() || !dv.is_finite() {
        return Err(invalid("surface metric solve overflow"));
    }
    Ok([du, dv])
}

impl<'a> PreparedSurfaceGeodesic<'a> {
    pub fn new(original: &'a NurbsSurface) -> Result<Self, GeometryError> {
        let chart = original.try_reparameterized(0.0..=1.0, 0.0..=1.0)?;
        for (old, new) in [
            (original.knots_u(), chart.knots_u()),
            (original.knots_v(), chart.knots_v()),
        ] {
            if old[1..old.len() - 1]
                .windows(2)
                .zip(new[1..new.len() - 1].windows(2))
                .any(|(a, b)| a[0] < a[1] && b[0] >= b[1])
            {
                return Err(invalid("parameter normalization collapsed a knot span"));
            }
        }
        let origin = original.control_points()[0].point();
        let mut scale: Real = 0.;
        for p in original.control_points() {
            scale = scale.max(origin.distance_to(p.point())?);
        }
        if scale == 0. {
            return Err(invalid("constant surface"));
        }
        let controls = chart
            .control_points()
            .iter()
            .map(|p| {
                let q = p.point().to_array();
                let o = origin.to_array();
                WeightedPoint3::try_new(
                    Point3::try_from(std::array::from_fn(|i| (q[i] - o[i]) / scale))?,
                    p.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let surface = NurbsSurface::try_new_rational(
            chart.degree_u(),
            chart.degree_v(),
            chart.control_point_count_u(),
            chart.control_point_count_v(),
            controls,
            chart.knots_u().to_vec(),
            chart.knots_v().to_vec(),
        )?;
        let breaks = [surface.knots_u(), surface.knots_v()].map(|knots| {
            let mut values = knots
                .iter()
                .copied()
                .filter(|k| *k > 0. && *k < 1.)
                .collect::<Vec<_>>();
            values.dedup();
            values
        });
        let closed = [chart_closes(&surface, 0)?, chart_closes(&surface, 1)?];
        Ok(Self {
            original,
            surface,
            scale,
            closed,
            breaks,
        })
    }

    fn canonical(&self, uv: [Real; 2]) -> Result<[Real; 2], GeometryError> {
        let mut result = uv;
        for axis in 0..2 {
            if !uv[axis].is_finite() {
                return Err(invalid("nonfinite parameter state"));
            }
            if self.closed[axis] {
                result[axis] = uv[axis].rem_euclid(1.);
            } else if (-32. * Real::EPSILON..=1. + 32. * Real::EPSILON).contains(&uv[axis]) {
                result[axis] = uv[axis].clamp(0., 1.);
            } else {
                return Err(GeometryError::ParameterCurveOutsideSurfaceDomain);
            }
        }
        Ok(result)
    }

    fn jet(&self, uv: [Real; 2]) -> Result<SurfaceJet2, GeometryError> {
        let [u, v] = self.canonical(uv)?;
        self.surface.evaluate_with_second_derivatives(u, v)
    }

    fn check_transition(
        &self,
        start: [Real; 2],
        end: [Real; 2],
        threshold: Real,
    ) -> Result<(), GeometryError> {
        let uv = self.canonical(end)?;
        let extended = std::array::from_fn(|axis| {
            if self.closed[axis] {
                uv[axis]
            } else {
                end[axis]
            }
        });
        if extended != uv
            && self
                .surface
                .evaluate_extended(extended[0], extended[1])?
                .distance_to(self.surface.evaluate(uv[0], uv[1])?)?
                > threshold
        {
            return Err(GeometryError::ParameterCurveOutsideSurfaceDomain);
        }
        for axis in 0..2 {
            let lo = start[axis].min(end[axis]);
            let hi = start[axis].max(end[axis]);
            if hi - lo > 2. {
                return Err(GeometryError::ParameterCurveOutsideSurfaceDomain);
            }
            for lap in lo.floor() as i64..=hi.floor() as i64 {
                let a = lo - lap as Real;
                let b = hi - lap as Real;
                let knots = &self.breaks[axis];
                let first = knots.partition_point(|k| *k < a);
                let last = knots.partition_point(|k| *k <= b);
                let seam = (self.closed[axis] && a <= 0. && 0. <= b).then_some(0.);
                for k in knots[first..last].iter().copied().chain(seam) {
                    let value = lap as Real + k;
                    if !(lo <= value && value <= hi) {
                        continue;
                    }
                    let mut a = uv;
                    let mut b = uv;
                    a[axis] = if k == 0. { 1. } else { k };
                    b[axis] = k;
                    let mut left = [ParameterSide::Right; 2];
                    left[axis] = ParameterSide::Left;
                    let ja = self
                        .surface
                        .evaluate_with_derivatives_on_sides(a[0], a[1], left[0], left[1])?;
                    let jb = self.surface.evaluate_with_derivatives(b[0], b[1])?;
                    if ja.0.distance_to(jb.0)? > threshold {
                        return Err(invalid("discontinuous surface chart"));
                    }
                    for (x, y) in [(ja.1, jb.1), (ja.2, jb.2)] {
                        let gap = combine(x, 1., y, -1.)?.length()?;
                        if gap > threshold * x.length()?.max(y.length()?) {
                            return Err(invalid("non-C1 surface chart transition"));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn rhs(&self, state: [Real; 4]) -> Result<[Real; 4], GeometryError> {
        let mut uv = self.canonical([state[0], state[1]])?;
        let sides = std::array::from_fn::<_, 2, _>(|axis| {
            if state[axis + 2] < 0. {
                if self.closed[axis] && uv[axis] == 0. {
                    uv[axis] = 1.;
                }
                ParameterSide::Left
            } else {
                ParameterSide::Right
            }
        });
        let j = self
            .surface
            .evaluate_with_second_derivatives_on_sides(uv[0], uv[1], sides[0], sides[1])?;
        let [du, dv] = [state[2], state[3]];
        let first = combine(j.derivative_uu, du * du, j.derivative_uv, 2. * du * dv)?;
        let second = combine(first, 1., j.derivative_vv, dv * dv)?;
        let acceleration = solve(j, second)?;
        Ok([du, dv, -acceleration[0], -acceleration[1]])
    }

    /// Travel a signed distance from a native UV location in a tangent direction.
    /// No partial endpoint is returned when a natural boundary is reached.
    pub fn point(
        &self,
        start: Point2,
        direction: UnitVector3,
        distance: Real,
        tolerance: Tolerance,
    ) -> Result<SurfaceGeodesicPoint, GeometryError> {
        if !distance.is_finite() {
            return Err(invalid("nonfinite travel distance"));
        }
        let length = distance / self.scale;
        if !length.is_finite() {
            return Err(invalid("unrepresentable integration distance"));
        }
        let threshold = (tolerance.absolute() / self.scale * 0.01 / length.abs().max(1.))
            .min(512. * Real::EPSILON);
        let uv = self.original.normalized_parameters(start.x(), start.y())?;
        self.check_transition(uv, uv, threshold)?;
        let j = self.jet(uv)?;
        let direction = direction.as_vector();
        let mut components = solve(j, direction)?;
        let projected = combine(j.derivative_u, components[0], j.derivative_v, components[1])?;
        if combine(projected, 1., direction, -1.)?.length()?
            > tolerance.angular().sin().abs().max(128. * Real::EPSILON)
        {
            return Err(invalid("initial direction is not tangent to the surface"));
        }
        let tangent = projected.normalized_nonzero()?;
        let projected_length = projected.length()?;
        for value in &mut components {
            *value /= projected_length;
        }
        let mut state = [uv[0], uv[1], components[0] * length, components[1] * length];
        let mut x: Real = 0.;
        let mut h: Real = 1. / 32.;
        let mut accepted = 0;
        let mut attempted = 0;
        let mut estimate = 0.;
        let mut speed_error: Real = 0.;
        let budget = (tolerance.absolute() / self.scale * 0.01).min(length.abs() * 1e-9);
        if distance != 0. && (!budget.is_finite() || budget <= 0. || length == 0.) {
            return Err(invalid("unrepresentable integration tolerance or distance"));
        }
        while distance != 0. && x < 1. {
            let j0 = self.jet([state[0], state[1]])?;
            for axis in 0..2 {
                let outward = (state[axis] >= 1. - 32. * Real::EPSILON && state[axis + 2] > 0.)
                    || (state[axis] <= 32. * Real::EPSILON && state[axis + 2] < 0.);
                let derivative = if axis == 0 {
                    j0.derivative_u
                } else {
                    j0.derivative_v
                };
                if !self.closed[axis]
                    && outward
                    && (1. - x) * state[axis + 2].abs() * derivative.length()? * self.scale
                        > tolerance.absolute() * 0.1
                {
                    return Err(GeometryError::ParameterCurveOutsideSurfaceDomain);
                }
            }
            if attempted >= 4096 || x + h == x {
                return Err(invalid(
                    "step budget exhausted or step below parameter resolution",
                ));
            }
            attempted += 1;
            h = h.min(1. - x);
            let previous = [state[0], state[1]];
            let step = cash_karp::system_step(x, h, state, |_, s| {
                self.check_transition(previous, [s[0], s[1]], threshold)?;
                self.rhs(s)
            })
            .and_then(|(next, error)| {
                self.check_transition(previous, [next[0], next[1]], threshold)?;
                Ok((next, error, self.jet([next[0], next[1]])?))
            });
            let (next, error, j) = match step {
                Ok(value) => value,
                Err(GeometryError::ParameterCurveOutsideSurfaceDomain) => {
                    h *= 0.5;
                    continue;
                }
                Err(e) => return Err(e),
            };
            let e = combine(j.derivative_u, error[0], j.derivative_v, error[1])?
                .length()?
                .max(combine(j.derivative_u, error[2], j.derivative_v, error[3])?.length()?);
            // C1 NURBS charts can have jumps in their second partials. A local
            // allowance proportional to h cannot cross such a jump: its error
            // is O(h), so further subdivision would never satisfy that rule.
            // Retain a bounded sum of the accepted embedded estimates instead.
            let allowance = budget;
            if allowance == 0. {
                return Err(invalid("integration error allowance underflow"));
            }
            let ratio = e / allowance;
            if ratio > 1. {
                h *= (0.9 * ratio.powf(-0.25)).clamp(0.1, 0.5);
                continue;
            }
            if next[0] == state[0]
                && next[1] == state[1]
                && h * distance.abs() > tolerance.absolute() * 0.1
            {
                return Err(invalid(
                    "parameter state cannot resolve the requested motion",
                ));
            }
            let drift = (combine(j.derivative_u, next[2], j.derivative_v, next[3])?.length()?
                - length.abs())
            .abs()
                * self.scale;
            speed_error = speed_error.max(drift);
            if drift
                > tolerance
                    .absolute()
                    .max(512. * Real::EPSILON * distance.abs())
            {
                return Err(invalid("arc-length speed drift exceeded tolerance"));
            }
            state = next;
            x = if h == 1. - x { 1. } else { x + h };
            accepted += 1;
            estimate += e;
            if estimate * self.scale > tolerance.absolute() * 0.25 {
                return Err(invalid("accumulated integration error exceeded tolerance"));
            }
            h = (h * if ratio > 1.89e-4 {
                0.9 * ratio.powf(-0.2)
            } else {
                5.
            })
            .min(0.125);
        }
        let canonical = self.canonical([state[0], state[1]])?;
        let extended = std::array::from_fn::<_, 2, _>(|axis| {
            if self.closed[axis] {
                canonical[axis]
            } else {
                state[axis]
            }
        });
        let clamping_error = if extended == canonical {
            0.
        } else {
            self.surface
                .evaluate_extended(extended[0], extended[1])?
                .distance_to(self.surface.evaluate(canonical[0], canonical[1])?)?
        };
        let uv = Point2::try_new(
            self.original.parameter_at_u(canonical[0])?,
            self.original.parameter_at_v(canonical[1])?,
        )?;
        let encoded = self.original.normalized_parameters(uv.x(), uv.y())?;
        let endpoint = self.surface.evaluate(canonical[0], canonical[1])?;
        let encoding_error =
            endpoint.distance_to(self.surface.evaluate(encoded[0], encoded[1])?)?;
        if encoding_error * self.scale > tolerance.absolute() * 0.1 {
            return Err(invalid(
                "native parameter chart cannot represent the endpoint accurately",
            ));
        }
        let point = self.original.evaluate(uv.x(), uv.y())?;
        let origin = self.original.control_points()[0].point().to_array();
        let q = point.to_array();
        let represented =
            Point3::try_from(std::array::from_fn(|i| (q[i] - origin[i]) / self.scale))?;
        let world_error = endpoint.distance_to(represented)?;
        if world_error * self.scale > tolerance.absolute() * 0.1 {
            return Err(invalid(
                "world coordinates cannot represent the endpoint accurately",
            ));
        }
        let tangent = if distance == 0. {
            tangent
        } else {
            let j = self.jet(canonical)?;
            combine(j.derivative_u, state[2], j.derivative_v, state[3])?.normalized_nonzero()?
        };
        Ok(SurfaceGeodesicPoint {
            point,
            uv,
            tangent,
            estimated_error: (estimate + encoding_error + world_error + clamping_error)
                * self.scale,
            accepted_steps: accepted,
            attempted_steps: attempted,
            maximum_speed_error: speed_error,
        })
    }
}

#[cfg(test)]
mod tests;
