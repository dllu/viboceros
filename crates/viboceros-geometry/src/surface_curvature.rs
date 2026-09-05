//! Shape operator in an orthonormal tangent frame, without squaring the
//! parameter metric or subtracting two nearly equal metric determinants.

use crate::{
    GeometryError, NurbsSurface, ParameterSide, Point3, Real, SurfaceJet2, UnitVector3, Vector3,
    require_finite,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceCurvature {
    pub point: Point3,
    pub normal: UnitVector3,
    /// Largest absolute curvature first. Signs refer to `normal`.
    pub principal: [Real; 2],
    /// Orthonormal eigenvectors; directions[0], directions[1], normal is right handed.
    /// A direction's sign, and directions at umbilics, are not intrinsic.
    pub directions: [UnitVector3; 2],
}

impl SurfaceCurvature {
    pub fn mean(self) -> Real {
        self.principal[0] * 0.5 + self.principal[1] * 0.5
    }

    /// Gaussian curvature can overflow even when both principal curvatures
    /// are representable; this does not prevent querying the principal values.
    pub fn gaussian(self) -> Result<Real, GeometryError> {
        let value = self.principal[0] * self.principal[1];
        require_finite([value], "Gaussian curvature")?;
        Ok(value)
    }

    pub fn reversed(self) -> Self {
        Self {
            normal: self.normal.opposite(),
            principal: self.principal.map(|k| -k),
            directions: [self.directions[0], self.directions[1].opposite()],
            ..self
        }
    }
}

impl NurbsSurface {
    pub fn curvature_at(&self, u: Real, v: Real) -> Result<SurfaceCurvature, GeometryError> {
        self.curvature_at_on_sides(u, v, ParameterSide::Right, ParameterSide::Right)
    }

    pub fn curvature_at_on_sides(
        &self,
        u: Real,
        v: Real,
        side_u: ParameterSide,
        side_v: ParameterSide,
    ) -> Result<SurfaceCurvature, GeometryError> {
        self.evaluate_with_second_derivatives_on_sides(u, v, side_u, side_v)?
            .curvature()
    }
}

impl SurfaceJet2 {
    /// Evaluates curvature at a regular surface jet. This rejects a numerically
    /// singular tangent plane instead of inventing a limiting curvature.
    pub fn curvature(self) -> Result<SurfaceCurvature, GeometryError> {
        let [su, sv] = [
            max_component(self.derivative_u),
            max_component(self.derivative_v),
        ];
        let x = self.derivative_u.normalized_nonzero()?;
        let tv = self.derivative_v.normalized_nonzero()?;
        let cross = x.as_vector().cross(tv.as_vector())?;
        let sine = cross.length()?;
        if sine <= 64.0 * Real::EPSILON {
            return Err(GeometryError::Degenerate {
                context: "surface tangent plane",
            });
        }
        let normal = cross.normalized_nonzero()?;
        let y = normal
            .as_vector()
            .cross(x.as_vector())?
            .normalized_nonzero()?;
        let cosine = x.as_vector().dot(tv.as_vector())?.clamp(-1.0, 1.0);
        let nu = scaled_norm(self.derivative_u, su);
        let nv = scaled_norm(self.derivative_v, sv);
        let coefficient = |second: Vector3, a: Real, b: Real, na: Real, nb: Real| {
            let projection = second.dot(normal.as_vector())?;
            Ok::<_, GeometryError>(divide_product(projection, a, b)? / (na * nb))
        };
        let l = coefficient(self.derivative_uu, su, su, nu, nu)?;
        let m = coefficient(self.derivative_uv, su, sv, nu, nv)?;
        let n = coefficient(self.derivative_vv, sv, sv, nv, nv)?;
        // Normalize before orthogonalizing so an intermediate unscaled second
        // fundamental form cannot overflow when the shape operator is finite.
        let scale = l.abs().max(m.abs()).max(n.abs());
        let (principal, rotation) = if scale == 0.0 {
            ([0.0; 2], [1.0, 0.0])
        } else {
            let (l, m, n) = (l / scale, m / scale, n / scale);
            let off = (-cosine).mul_add(l, m) / sine;
            let last = cosine.mul_add(cosine * l, (-2.0 * cosine).mul_add(m, n)) / sine / sine;
            let (eigenvalues, rotation) = symmetric_eigen(l, off, last)?;
            let principal = eigenvalues.map(|k| k * scale);
            require_finite(principal, "principal curvature")?;
            (principal, rotation)
        };
        let [c, s] = rotation;
        let first = Vector3::try_from(std::array::from_fn(|i| {
            c.mul_add(x.as_vector().to_array()[i], s * y.as_vector().to_array()[i])
        }))?
        .normalized_nonzero()?;
        let second = normal
            .as_vector()
            .cross(first.as_vector())?
            .normalized_nonzero()?;
        Ok(SurfaceCurvature {
            point: self.point,
            normal,
            principal,
            directions: [first, second],
        })
    }
}

fn max_component(v: Vector3) -> Real {
    v.to_array().into_iter().map(Real::abs).fold(0.0, Real::max)
}

fn scaled_norm(v: Vector3, scale: Real) -> Real {
    let [x, y, z] = v.to_array().map(|a| a / scale);
    x.hypot(y).hypot(z)
}

fn divide_product(value: Real, a: Real, b: Real) -> Result<Real, GeometryError> {
    if value == 0.0 {
        return Ok(0.0);
    }
    // A subnormal product loses relative precision even when the quotient is
    // ordinary-sized. Do not use it merely because it is nonzero and finite.
    let product = a * b;
    if product.is_normal() {
        let result = value / product;
        if result.is_finite() {
            return Ok(result);
        }
    }
    for result in [(value / a.min(b)) / a.max(b), (value / a.max(b)) / a.min(b)] {
        if result.is_finite() && result != 0.0 {
            return Ok(result);
        }
    }
    let result = (value / a.min(b)) / a.max(b);
    require_finite([result], "surface curvature metric scaling")?;
    Ok(result)
}

fn symmetric_eigen(a: Real, b: Real, d: Real) -> Result<([Real; 2], [Real; 2]), GeometryError> {
    let scale = a.abs().max(b.abs()).max(d.abs());
    require_finite([scale], "surface shape operator")?;
    if scale == 0.0 {
        return Ok(([0.0; 2], [1.0, 0.0]));
    }
    let (a, b, d) = (a / scale, b / scale, d / scale);
    let center = a * 0.5 + d * 0.5;
    let radius = (a * 0.5 - d * 0.5).hypot(b);
    let theta = 0.5 * (2.0 * b).atan2(a - d);
    let (s, c) = theta.sin_cos();
    let major = if center >= 0.0 {
        center + radius
    } else {
        center - radius
    };
    // Recover the small eigenvalue from the determinant to avoid cancellation
    // of nearly equal center/radius terms on nearly developable surfaces.
    let bb = b * b;
    let determinant = a.mul_add(d, -bb) + (-b).mul_add(b, bb);
    let minor = if major == 0.0 {
        0.0
    } else {
        determinant / major
    };
    let rotation = if center >= 0.0 { [c, s] } else { [-s, c] };
    Ok(([major * scale, minor * scale], rotation))
}

#[cfg(test)]
mod tests;
