//! Boundary-value geodesics with exact primitive proposals and a continuous
//! surface-image admission gate. General shots find local geodesics; they do
//! not certify a global minimum across cut loci or boundary-following routes.
use crate::{
    Circle3, CircularArc3, GeometryError, MAX_CURVE_FIT_CONTROL_POINTS, MorphInterpolationSolver,
    NurbsCurve, NurbsSurface, Point2, Point3, PreparedSurfaceGeodesic, Real, SurfaceGeodesicPoint,
    Tolerance, UnitVector3, Vector3,
    morph::fit_parameterized_curve_with_derivatives,
    spline_collocation::seed_breaks,
    surface_geodesic::{chart_closes, solve_tangent},
};
use std::f64::consts::{PI, TAU};

const MAX_QUERIES: usize = 8192;

#[derive(Clone, Debug)]
pub struct SurfaceShortPath {
    pub curve: NurbsCurve,
    /// Analytic length or numerical unit-speed shooting length.
    pub distance: Real,
    /// Continuous bound for the entire curve's correspondence to the source.
    pub surface_image_bound: Real,
    pub geodesic_queries: usize,
    pub integration_steps: usize,
    /// Embedded integration estimates, not a global path-error certificate.
    pub maximum_integration_estimate: Real,
}
fn invalid(context: &'static str) -> GeometryError {
    GeometryError::InvalidSurfaceShortPath { context }
}
fn scaled_tolerance(t: Tolerance, scale: Real) -> Result<Tolerance, GeometryError> {
    Tolerance::try_new(t.absolute() * scale, t.relative(), t.angular())
}
fn sum(a: Vector3, x: Real, b: Vector3, y: Real) -> Result<Vector3, GeometryError> {
    let a = a.to_array();
    let b = b.to_array();
    Vector3::try_from(std::array::from_fn(|i| a[i] * x + b[i] * y))
}
fn line(a: Point3, b: Point3) -> Result<NurbsCurve, GeometryError> {
    NurbsCurve::try_new(1, vec![a, b], vec![0., 0., 1., 1.])
}
fn fit(
    endpoints: [Point3; 2],
    derivatives: [Vector3; 2],
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
    mut evaluate: impl FnMut(Real) -> Result<Point3, GeometryError>,
) -> Result<NurbsCurve, GeometryError> {
    let seed = line(endpoints[0], endpoints[1])?;
    fit_parameterized_curve_with_derivatives(
        &seed,
        seed_breaks(seed.degree(), seed.knots(), seed.domain()),
        tolerance,
        MAX_CURVE_FIT_CONTROL_POINTS,
        solver,
        Some(derivatives),
        |t, _| {
            if t == 0. {
                Ok(endpoints[0])
            } else if t == 1. {
                Ok(endpoints[1])
            } else {
                evaluate(t)
            }
        },
    )
}

/// Create a path on the natural, untrimmed surface between native UV points.
/// Planes, canonical cylinders and spheres use shortest analytic loci. Other
/// regular charts use bounded, multiple-seed shooting and select the shortest
/// converged candidate. Boundary detours and general singularities may fail.
/// The optional interpolation backend is shared with the other curve producers.
pub fn try_short_path_on_surface(
    surface: &NurbsSurface,
    start: Point2,
    end: Point2,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
) -> Result<SurfaceShortPath, GeometryError> {
    let normalized = [start, end].map(|p| surface.normalized_parameters(p.x(), p.y()));
    let uv = [normalized[0].clone()?, normalized[1].clone()?];
    if uv.iter().flatten().any(|p| !(0. ..=1.).contains(p)) {
        return Err(invalid("endpoints must lie in the natural surface domain"));
    }
    let endpoints = [
        surface.evaluate(start.x(), start.y())?,
        surface.evaluate(end.x(), end.y())?,
    ];
    let numerical = scaled_tolerance(tolerance, 0.02)?;
    let admission = scaled_tolerance(tolerance, 0.5)?;
    let sphere = surface.canonical_sphere(numerical)?;
    let mut pullback_endpoints = [start, end];
    if sphere.is_some() {
        // Longitude is redundant at an exact collapsed pole. Retain the
        // other endpoint's meridian rather than imposing an arbitrary UV
        // jump on an otherwise continuous isocurve.
        let nu = surface.control_point_count_u();
        let cp = surface.control_points();
        for i in 0..2 {
            let p = pullback_endpoints[i];
            let row = if p.y() == *surface.domain_v().start() {
                Some(&cp[..nu])
            } else if p.y() == *surface.domain_v().end() {
                Some(&cp[cp.len() - nu..])
            } else {
                None
            };
            if row.is_some_and(|r| r.iter().all(|c| c.point() == r[0].point())) {
                pullback_endpoints[i] = Point2::try_new(pullback_endpoints[1 - i].x(), p.y())?;
            }
        }
    }
    let mut endpoint_charts = vec![pullback_endpoints];
    for axis in 0..2 {
        if !chart_closes(surface, axis)? {
            continue;
        }
        let domain = if axis == 0 {
            surface.domain_u()
        } else {
            surface.domain_v()
        };
        for endpoint in 0..2 {
            for mut pair in endpoint_charts.clone() {
                let p = pair[endpoint].to_array();
                let other = if p[axis] == *domain.start() {
                    *domain.end()
                } else if p[axis] == *domain.end() {
                    *domain.start()
                } else {
                    continue;
                };
                let mut q = p;
                q[axis] = other;
                pair[endpoint] = Point2::try_from(q)?;
                endpoint_charts.push(pair);
            }
        }
    }
    let admit = |curve: NurbsCurve, distance: Real, diagnostics: [Real; 3]| {
        let mut proof = surface.try_pullback_curve_certified_with_bound(
            &curve,
            Some(endpoint_charts[0]),
            admission,
        );
        for &endpoints in &endpoint_charts[1..] {
            if proof.is_ok() {
                break;
            }
            proof =
                surface.try_pullback_curve_certified_with_bound(&curve, Some(endpoints), admission);
        }
        let (_, bound) = proof?;
        Ok(SurfaceShortPath {
            curve: if distance > 0. {
                curve.try_reparameterized(0. ..=distance)?
            } else {
                curve
            },
            distance,
            surface_image_bound: bound,
            geodesic_queries: diagnostics[0] as usize,
            integration_steps: diagnostics[1] as usize,
            maximum_integration_estimate: diagnostics[2],
        })
    };
    if start == end {
        return admit(line(endpoints[0], endpoints[1])?, 0., [0.; 3]);
    }
    if endpoints[0] == endpoints[1] {
        return Err(invalid("distinct parameters have coincident images"));
    }
    // A line is shortest whenever its complete image stays in this patch.
    if surface.plane(numerical)?.is_some()
        && let Ok(path) = admit(
            line(endpoints[0], endpoints[1])?,
            endpoints[0].distance_to(endpoints[1])?,
            [0.; 3],
        )
    {
        return Ok(path);
    }
    if let Some((frame, radius, _)) = surface.canonical_cylinder(numerical)? {
        let a = frame.coordinates_of(endpoints[0])?;
        let b = frame.coordinates_of(endpoints[1])?;
        let theta = a[1].atan2(a[0]);
        let mut sweep = b[1].atan2(b[0]) - theta;
        if sweep > PI {
            sweep -= TAU;
        }
        if sweep < -PI {
            sweep += TAU;
        }
        let dz = b[2] - a[2];
        let tangent = |t: Real| {
            frame.vector_at([
                -radius * sweep * (theta + t * sweep).sin(),
                radius * sweep * (theta + t * sweep).cos(),
                dz,
            ])
        };
        let curve = fit(
            endpoints,
            [tangent(0.)?, tangent(1.)?],
            numerical,
            solver,
            |t| {
                frame.point_at([
                    radius * (theta + t * sweep).cos(),
                    radius * (theta + t * sweep).sin(),
                    a[2] + t * dz,
                ])
            },
        )?;
        return admit(curve, (radius * sweep).hypot(dz), [0.; 3]);
    }
    if let Some((center, radius)) = sphere {
        let a = center.direction_to(endpoints[0])?;
        let b = center.direction_to(endpoints[1])?;
        let cross = a.as_vector().cross(b.as_vector())?;
        let sine = cross.length()?;
        let cosine = a.as_vector().dot(b.as_vector())?.clamp(-1., 1.);
        let (normal, angle) = if sine <= 128. * Real::EPSILON && cosine < 0. {
            // A half great circle is nonunique. Choose decreasing source U,
            // matching the captured canonical Rhino antipodal convention.
            let (_, du, dv) = surface.evaluate_with_derivatives(start.x(), start.y())?;
            let tangent = if du.length()? > numerical.absolute() {
                du
            } else {
                dv
            };
            (
                a.as_vector()
                    .cross(tangent.scaled(-1.)?)?
                    .normalized_nonzero()?,
                PI,
            )
        } else {
            (cross.normalized_nonzero()?, sine.atan2(cosine))
        };
        let circle = Circle3::try_from_center_point(center, endpoints[0], normal, numerical)?;
        let curve = CircularArc3::try_from_circle_sweep(circle, angle)?.to_nurbs()?;
        return admit(curve, radius * angle, [0.; 3]);
    }
    let chart = surface.try_reparameterized(0. ..=1., 0. ..=1.)?;
    for (old, new) in [
        (surface.knots_u(), chart.knots_u()),
        (surface.knots_v(), chart.knots_v()),
    ] {
        if old
            .windows(2)
            .zip(new.windows(2))
            .any(|(a, b)| a[0] < a[1] && b[0] >= b[1])
        {
            return Err(invalid("parameter normalization collapsed a knot span"));
        }
    }
    // No shooting calculation uses native UV magnitudes.
    let mut shots = Shots::new(&chart, uv[0], numerical)?;
    let (_, du, dv) = chart.evaluate_with_derivatives(uv[0][0], uv[0][1])?;
    let x = du.normalized_nonzero()?;
    let y = du
        .cross(dv)?
        .normalized_nonzero()?
        .as_vector()
        .cross(x.as_vector())?
        .normalized_nonzero()?;
    let basis = [x, y];
    let mut best: Option<(Real, Real, SurfaceGeodesicPoint)> = None;
    for ku in if shots.closed[0] { -1..=1 } else { 0..=0 } {
        for kv in if shots.closed[1] { -1..=1 } else { 0..=0 } {
            let delta = [
                uv[1][0] + ku as Real - uv[0][0],
                uv[1][1] + kv as Real - uv[0][1],
            ];
            let direction = sum(du, delta[0], dv, delta[1])?;
            let angle = direction
                .dot(y.as_vector())?
                .atan2(direction.dot(x.as_vector())?);
            let mut length = 0.;
            let mut previous = endpoints[0];
            for i in 1..=32 {
                let t = i as Real / 32.;
                let q = shots.canonical([uv[0][0] + t * delta[0], uv[0][1] + t * delta[1]]);
                let point = chart.evaluate(q[0], q[1])?;
                length += previous.distance_to(point)?;
                previous = point;
            }
            // Extra angular seeds help strongly bent regular patches without
            // allowing an unbounded global search through arbitrary topology.
            for shift in [0., -0.25, 0.25] {
                if let Some(candidate) =
                    shots.solve(basis, angle + shift, length, uv[1], endpoints[1])?
                    && best.as_ref().is_none_or(|old| candidate.1 < old.1)
                {
                    best = Some(candidate);
                }
            }
        }
    }
    let (angle, length, last) = best.ok_or(invalid("no boundary-value geodesic converged"))?;
    let direction = direction(basis, angle)?;
    let curve = fit(
        endpoints,
        [
            direction.as_vector().scaled(length)?,
            last.tangent.as_vector().scaled(length)?,
        ],
        numerical,
        solver,
        |t| shots.point(direction, length * t).map(|p| p.point),
    )?;
    admit(
        curve,
        length,
        [shots.queries as Real, shots.steps as Real, shots.estimate],
    )
}

fn direction(basis: [UnitVector3; 2], angle: Real) -> Result<UnitVector3, GeometryError> {
    sum(
        basis[0].as_vector(),
        angle.cos(),
        basis[1].as_vector(),
        angle.sin(),
    )?
    .normalized_nonzero()
}
struct Shots<'a> {
    surface: &'a NurbsSurface,
    integrator: PreparedSurfaceGeodesic<'a>,
    start: Point2,
    closed: [bool; 2],
    tolerance: Tolerance,
    queries: usize,
    steps: usize,
    estimate: Real,
}
impl<'a> Shots<'a> {
    fn new(
        surface: &'a NurbsSurface,
        start: [Real; 2],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Ok(Self {
            surface,
            integrator: PreparedSurfaceGeodesic::new(surface)?,
            start: Point2::try_from(start)?,
            closed: [chart_closes(surface, 0)?, chart_closes(surface, 1)?],
            tolerance,
            queries: 0,
            steps: 0,
            estimate: 0.,
        })
    }
    fn canonical(&self, mut uv: [Real; 2]) -> [Real; 2] {
        for (axis, p) in uv.iter_mut().enumerate() {
            if self.closed[axis] {
                *p = p.rem_euclid(1.);
            }
        }
        uv
    }
    fn delta(&self, a: [Real; 2], b: [Real; 2]) -> [Real; 2] {
        std::array::from_fn(|i| {
            let d = b[i] - a[i];
            if self.closed[i] { d - d.round() } else { d }
        })
    }
    fn point(
        &mut self,
        direction: UnitVector3,
        length: Real,
    ) -> Result<SurfaceGeodesicPoint, GeometryError> {
        if self.queries >= MAX_QUERIES {
            return Err(GeometryError::SurfaceShortPathWorkLimit {
                maximum: MAX_QUERIES,
            });
        }
        self.queries += 1;
        let p = self
            .integrator
            .point(self.start, direction, length, self.tolerance)?;
        self.steps += p.attempted_steps;
        self.estimate = self.estimate.max(p.estimated_error);
        Ok(p)
    }
    fn residual(
        &self,
        shot: SurfaceGeodesicPoint,
        end: [Real; 2],
    ) -> Result<Vector3, GeometryError> {
        let d = self.delta(shot.uv.to_array(), end);
        let (_, du, dv) = self
            .surface
            .evaluate_with_derivatives(shot.uv.x(), shot.uv.y())?;
        sum(du, d[0], dv, d[1])
    }
    fn attempt(
        &mut self,
        basis: [UnitVector3; 2],
        angle: Real,
        length: Real,
    ) -> Result<Option<SurfaceGeodesicPoint>, GeometryError> {
        match self.point(direction(basis, angle)?, length) {
            Ok(p) => Ok(Some(p)),
            Err(e @ GeometryError::SurfaceShortPathWorkLimit { .. }) => Err(e),
            Err(_) => Ok(None),
        }
    }
    fn solve(
        &mut self,
        basis: [UnitVector3; 2],
        mut angle: Real,
        mut length: Real,
        end: [Real; 2],
        endpoint: Point3,
    ) -> Result<Option<(Real, Real, SurfaceGeodesicPoint)>, GeometryError> {
        for _ in 0..40 {
            let Some(shot) = self.attempt(basis, angle, length)? else {
                return Ok(None);
            };
            let residual = self.residual(shot, end)?;
            let error = residual.length()?;
            if error <= self.tolerance.absolute()
                && shot.point.distance_to(endpoint)? <= self.tolerance.absolute()
            {
                return Ok(Some((angle, length, shot)));
            }
            let h = Real::EPSILON.cbrt();
            let plus = self.attempt(basis, angle + h, length)?;
            let minus = self.attempt(basis, angle - h, length)?;
            let (a, b, width) = match (minus, plus) {
                (Some(a), Some(b)) => (a, b, 2. * h),
                (None, Some(b)) => (shot, b, h),
                (Some(a), None) => (a, shot, h),
                _ => return Ok(None),
            };
            let delta = self.delta(a.uv.to_array(), b.uv.to_array());
            let (_, du, dv) = self
                .surface
                .evaluate_with_derivatives(shot.uv.x(), shot.uv.y())?;
            let derivative = sum(du, delta[0] / width, dv, delta[1] / width)?;
            let Ok([mut da, mut dl]) =
                solve_tangent(derivative, shot.tangent.as_vector(), residual)
            else {
                return Ok(None);
            };
            let scale = if da.abs() > 0.5 { 0.5 / da.abs() } else { 1. };
            da *= scale;
            dl *= scale;
            let mut improved = false;
            for i in 0..24 {
                let scale = 0.5_f64.powi(i);
                let candidate = length + scale * dl;
                if candidate <= 0. || !candidate.is_finite() {
                    continue;
                }
                if let Some(p) = self.attempt(basis, angle + scale * da, candidate)?
                    && self.residual(p, end)?.length()? < error
                {
                    angle += scale * da;
                    length = candidate;
                    improved = true;
                    break;
                }
            }
            if !improved {
                return Ok(None);
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests;
