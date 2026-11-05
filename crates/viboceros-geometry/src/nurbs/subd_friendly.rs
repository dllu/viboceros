//! Uniform cubic profile fitting with natural or periodic boundary conditions.
use super::*;

impl NurbsCurve {
    /// Fit the native automatic SubD-friendly layout through Greville stations.
    /// Cubic profiles omit their two endpoint handles; other degrees use every
    /// station. Periodic output includes three repeated controls. This is a
    /// sampled approximation, not an exact
    /// conversion or a continuous deviation certificate.
    pub fn try_subd_friendly(&self, periodic: bool) -> Result<Self, GeometryError> {
        let curve = self.clamped_to_active_domain()?;
        let count = curve.control_points().len();
        let mut parameters = (0..count)
            .filter(|&i| curve.degree() != 3 || (i != 1 && i != count - 2))
            .map(|i| curve.control_greville_parameter(i))
            .collect::<Result<Vec<_>, _>>()?;
        if periodic {
            parameters.pop();
            if parameters.len() < 3 {
                parameters = (0..3)
                    .map(|i| curve.parameter_at(i as Real / 3.))
                    .collect::<Result<Vec<_>, _>>()?;
            }
        }
        if parameters.len() > 99_997 {
            return Err(GeometryError::SubDInterpolationFailed);
        }
        let samples = parameters
            .into_iter()
            .map(|t| curve.evaluate(t))
            .collect::<Result<Vec<_>, _>>()?;
        interpolate_natural_samples(&samples, periodic)
    }
}

pub(crate) fn interpolate_natural_samples(
    samples: &[Point3],
    periodic: bool,
) -> Result<NurbsCurve, GeometryError> {
    if samples.len() < if periodic { 3 } else { 2 } || samples.len() > 100_000 {
        return Err(GeometryError::SubDInterpolationFailed);
    }
    let bounds = BoundingBox3::from_points(samples.iter().copied())?;
    let min = bounds.min().to_array();
    let max = bounds.max().to_array();
    let origin: [Real; 3] = std::array::from_fn(|i| min[i] * 0.5 + max[i] * 0.5);
    let scale = samples
        .iter()
        .flat_map(|p| {
            p.to_array()
                .into_iter()
                .zip(origin)
                .map(|(p, o)| (p - o).abs())
        })
        .fold(0_f64, Real::max)
        .max(Real::MIN_POSITIVE);
    let normalized = samples
        .iter()
        .map(|p| std::array::from_fn(|i| (p.to_array()[i] - origin[i]) / scale))
        .collect::<Vec<_>>();
    let solved = crate::cubic_grid::solve_axis(&normalized, [samples.len(), 1], 0, periodic, &[])?;
    let mut controls = solved
        .iter()
        .map(|p| {
            Point3::try_from(std::array::from_fn::<_, 3, _>(|i| {
                p[i].mul_add(scale, origin[i])
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let knots = if periodic {
        controls.rotate_right(1);
        controls.extend_from_within(..3);
        crate::curve_rebuild::periodic_uniform_knots(3, samples.len())
    } else {
        let last = controls.len() - 1;
        let blend = |a: Point3, b: Point3| {
            Point3::try_from(std::array::from_fn::<_, 3, _>(|i| {
                a.to_array()[i] * (2. / 3.) + b.to_array()[i] / 3.
            }))
        };
        let first_handle = blend(controls[0], controls[1])?;
        let last_handle = blend(controls[last], controls[last - 1])?;
        controls.insert(last, last_handle);
        controls.insert(1, first_handle);
        clamped_uniform_knots(3, controls.len())?
    };
    let curve = NurbsCurve::try_new(3, controls, knots)?;
    if periodic {
        Ok(curve)
    } else {
        curve.try_reparameterized(0. ..=(samples.len() - 1) as Real)
    }
}
