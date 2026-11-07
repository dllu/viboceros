//! Normal components of the curve initially following a straight UV line.
use crate::exact_scalar::{rational, scalar};
use crate::{GeometryError, Real, SurfaceJet3, UnitVector3, Vector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceNormalJet3 {
    pub curvature: Vector3,
    pub third_derivative: Vector3,
}
impl SurfaceJet3 {
    /// Follow the straight parameter line initially tangent to `direction`,
    /// normalize its derivative targets to zero tangential components, then
    /// project them onto the surface normal. This is parameterization-dependent
    /// and generally differs from a geometric normal section's third jet.
    pub fn normal_parameter_line_jet(
        self,
        direction: UnitVector3,
    ) -> Result<SurfaceNormalJet3, GeometryError> {
        let s = self.second;
        let scales = [max_component(s.derivative_u), max_component(s.derivative_v)];
        let normalized = [s.derivative_u, s.derivative_v]
            .into_iter()
            .zip(scales)
            .map(|(v, scale)| Vector3::try_from(v.to_array().map(|x| x / scale)))
            .collect::<Result<Vec<_>, _>>()?;
        let lengths = [normalized[0].length()?, normalized[1].length()?];
        let x = s.derivative_u.normalized_nonzero()?;
        let v = s.derivative_v.normalized_nonzero()?;
        let cross = x.as_vector().cross(v.as_vector())?;
        let sine = cross.length()?;
        if sine <= 64. * Real::EPSILON {
            return Err(GeometryError::Degenerate {
                context: "surface tangent plane",
            });
        }
        let n = cross.normalized_nonzero()?;
        let y = n.as_vector().cross(x.as_vector())?.normalized_nonzero()?;
        let d = direction.as_vector();
        if d.dot(n.as_vector())?.abs() > 1e-10 {
            return Err(GeometryError::Degenerate {
                context: "surface parameter-line tangent direction",
            });
        }
        let b = d.dot(y.as_vector())? / sine;
        let a = d.dot(x.as_vector())? - b * x.as_vector().dot(v.as_vector())?;
        let normalized_partial = |v, u, w| divide(v, scales, lengths, [u, w]);
        let second = combine([
            (normalized_partial(s.derivative_uu, 2, 0)?, a * a),
            (normalized_partial(s.derivative_uv, 1, 1)?, 2. * a * b),
            (normalized_partial(s.derivative_vv, 0, 2)?, b * b),
        ])?;
        let third = combine([
            (normalized_partial(self.derivative_uuu, 3, 0)?, a * a * a),
            (
                normalized_partial(self.derivative_uuv, 2, 1)?,
                3. * a * a * b,
            ),
            (
                normalized_partial(self.derivative_uvv, 1, 2)?,
                3. * a * b * b,
            ),
            (normalized_partial(self.derivative_vvv, 0, 3)?, b * b * b),
        ])?;
        let curvature = second.dot(n.as_vector())?;
        let alpha = second.dot(d)?;
        let raw = third.dot(n.as_vector())?;
        let target = (-3. * alpha).mul_add(curvature, raw);
        let target = if target.is_finite() {
            target
        } else {
            scalar(&(rational(raw) - rational(3.) * rational(alpha) * rational(curvature)))?
        };
        Ok(SurfaceNormalJet3 {
            curvature: n.as_vector().scaled(curvature)?,
            third_derivative: n.as_vector().scaled(target)?,
        })
    }
}
fn max_component(v: Vector3) -> Real {
    v.to_array().into_iter().map(Real::abs).fold(0., Real::max)
}
fn divide(
    v: Vector3,
    scales: [Real; 2],
    lengths: [Real; 2],
    orders: [usize; 2],
) -> Result<Vector3, GeometryError> {
    let factors = orders
        .into_iter()
        .enumerate()
        .flat_map(|(axis, order)| (0..order).flat_map(move |_| [scales[axis], lengths[axis]]))
        .collect::<Vec<_>>();
    let mut result = [0.; 3];
    for (out, value) in result.iter_mut().zip(v.to_array()) {
        let mut current = value;
        let mut exact = false;
        for factor in &factors {
            current /= factor;
            if !current.is_finite() || (value != 0. && !current.is_normal()) {
                exact = true;
                break;
            }
        }
        *out = if exact {
            let mut n = rational(value);
            for factor in &factors {
                n /= rational(*factor);
            }
            scalar(&n)?
        } else {
            current
        };
    }
    Vector3::try_from(result)
}
fn combine<const N: usize>(terms: [(Vector3, Real); N]) -> Result<Vector3, GeometryError> {
    let mut out = [0.; 3];
    for (i, out) in out.iter_mut().enumerate() {
        let value = terms
            .iter()
            .fold(0., |sum, (v, c)| v.to_array()[i].mul_add(*c, sum));
        *out = if value.is_finite() {
            value
        } else {
            scalar(
                &terms
                    .iter()
                    .map(|(v, c)| rational(v.to_array()[i]) * rational(*c))
                    .sum(),
            )?
        };
    }
    Vector3::try_from(out)
}

#[cfg(test)]
mod tests;
