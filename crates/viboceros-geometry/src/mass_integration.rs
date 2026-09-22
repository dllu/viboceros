//! Shared spatial conditioning and bounded rectangular mass-density quadrature.
use crate::{
    BoundingBox3, FiniteSum, GeometryError, NurbsSurface, Point3, Real, Tolerance, Vector3,
    WeightedPoint3, integration::integrate_adaptive,
};

pub(crate) struct SpatialFrame {
    pub(crate) origin: Point3,
    pub(crate) scale: Real,
}

impl SpatialFrame {
    pub(crate) fn new(bounds: BoundingBox3) -> Result<Self, GeometryError> {
        let origin = bounds.center()?;
        let scale = [bounds.min(), bounds.max()]
            .into_iter()
            .map(|p| origin.vector_to(p).map(|v| v.to_array()))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .map(Real::abs)
            .fold(0., Real::max);
        if scale == 0. {
            return Err(GeometryError::Degenerate {
                context: "mass integration frame",
            });
        }
        Ok(Self { origin, scale })
    }

    pub(crate) fn tolerance(&self, tolerance: Tolerance) -> Real {
        (tolerance.absolute() / self.scale)
            .max(tolerance.relative())
            .min(Real::MAX)
    }

    pub(crate) fn surface(&self, surface: &NurbsSurface) -> Result<NurbsSurface, GeometryError> {
        let controls = surface
            .control_points()
            .iter()
            .map(|c| {
                let values = self.origin.vector_to(c.point())?.to_array();
                let normalized = values.map(|v| v / self.scale);
                if values
                    .into_iter()
                    .zip(normalized)
                    .any(|(a, b)| a != 0. && b == 0.)
                {
                    return Err(GeometryError::NumericalIntegrationDidNotConverge);
                }
                WeightedPoint3::try_new(Point3::try_from(normalized)?, c.weight())
            })
            .collect::<Result<Vec<_>, _>>()?;
        NurbsSurface::try_new_rational(
            surface.degree_u(),
            surface.degree_v(),
            surface.control_point_count_u(),
            surface.control_point_count_v(),
            controls,
            surface.knots_u().to_vec(),
            surface.knots_v().to_vec(),
        )
    }
}

/// `normal` is one quarter of the oriented span-scaled surface Jacobian.
/// Components share a two-million-evaluation budget, not an unbounded retry.
pub(crate) fn rectangle_density(
    surface: &NurbsSurface,
    absolute: Real,
    relative: Real,
    mut density: impl FnMut(Point3, Vector3, usize) -> Result<Real, GeometryError>,
) -> Result<[Real; 4], GeometryError> {
    let count = surface
        .spans_u()
        .count()
        .checked_mul(surface.spans_v().count())
        .ok_or(GeometryError::NumericalIntegrationDidNotConverge)?;
    let tolerance = (absolute / count as Real).max(Real::MIN_POSITIVE);
    let mut sums: [FiniteSum; 4] = std::array::from_fn(|_| FiniteSum::default());
    let mut remaining_evaluations = 2_000_000usize;
    for (component, sum) in sums.iter_mut().enumerate() {
        for u in surface.spans_u() {
            for v in surface.spans_v() {
                let half_u = u.1 * 0.5 - u.0 * 0.5;
                let half_v = v.1 * 0.5 - v.0 * 0.5;
                if half_u <= 0. || half_v <= 0. || !half_u.is_finite() || !half_v.is_finite() {
                    return Err(GeometryError::NumericalIntegrationDidNotConverge);
                }
                let value = integrate_adaptive(0., 1., tolerance, relative, |a| {
                    let u = u.0.mul_add(1. - a, u.1 * a);
                    integrate_adaptive(
                        0.,
                        1.,
                        (tolerance * 0.25).max(Real::MIN_POSITIVE),
                        relative,
                        |b| {
                            remaining_evaluations = remaining_evaluations
                                .checked_sub(1)
                                .ok_or(GeometryError::NumericalIntegrationDidNotConverge)?;
                            let v = v.0.mul_add(1. - b, v.1 * b);
                            let (p, du, dv) = surface.evaluate_with_derivatives(u, v)?;
                            density(p, du.scaled(half_u)?.cross(dv.scaled(half_v)?)?, component)
                        },
                    )
                })?;
                sum.add(value)?;
            }
        }
    }
    Ok([
        sums[0].total()?,
        sums[1].total()?,
        sums[2].total()?,
        sums[3].total()?,
    ])
}
