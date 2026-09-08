use crate::{GeometryError, Real, Tolerance, require_finite};

/// A finite vector in three-dimensional model space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector3(nalgebra::Vector3<Real>);

impl Vector3 {
    pub fn try_new(x: Real, y: Real, z: Real) -> Result<Self, GeometryError> {
        require_finite([x, y, z], "vector")?;
        Ok(Self(nalgebra::Vector3::new(x, y, z)))
    }

    #[inline]
    pub fn x(self) -> Real {
        self.0.x
    }

    #[inline]
    pub fn y(self) -> Real {
        self.0.y
    }

    #[inline]
    pub fn z(self) -> Real {
        self.0.z
    }

    #[inline]
    pub fn to_array(self) -> [Real; 3] {
        [self.x(), self.y(), self.z()]
    }

    /// A scaled norm that avoids intermediate overflow and reports a result
    /// whose true magnitude is not representable by [`Real`].
    pub fn length(self) -> Result<Real, GeometryError> {
        let value = self.x().hypot(self.y()).hypot(self.z());
        require_finite([value], "vector length")?;
        Ok(value)
    }

    pub fn dot(self, other: Self) -> Result<Real, GeometryError> {
        // Common-scale normalization can erase a small component even when
        // the large component is multiplied by zero. Prefer fused evaluation
        // when the individual products and the result are representable.
        let left_components = self.to_array();
        let right_components = other.to_array();
        let ordinary_products = left_components
            .into_iter()
            .zip(right_components)
            .all(|(a, b)| {
                let product = a * b;
                product.is_finite() && (product != 0.0 || a == 0.0 || b == 0.0)
            });
        if ordinary_products {
            let direct = self
                .x()
                .mul_add(other.x(), self.y().mul_add(other.y(), self.z() * other.z()));
            if direct.is_finite() {
                return Ok(direct);
            }
        }
        let left_scale = self.x().abs().max(self.y().abs()).max(self.z().abs());
        let right_scale = other.x().abs().max(other.y().abs()).max(other.z().abs());
        if left_scale == 0.0 || right_scale == 0.0 {
            return Ok(0.0);
        }

        // Scaling avoids overflowing individual products when large terms
        // cancel. Trying every association avoids both spurious overflow and
        // spurious underflow in an otherwise representable three-factor result.
        let left = self.to_array().map(|value| value / left_scale);
        let right = other.to_array().map(|value| value / right_scale);
        let normalized = left[0].mul_add(right[0], left[1].mul_add(right[1], left[2] * right[2]));
        let magnitude = product_three(normalized.abs(), left_scale, right_scale, "dot product")?;
        Ok(normalized.signum() * magnitude)
    }

    pub fn cross(self, other: Self) -> Result<Self, GeometryError> {
        let left_scale = self.x().abs().max(self.y().abs()).max(self.z().abs());
        let right_scale = other.x().abs().max(other.y().abs()).max(other.z().abs());
        if left_scale == 0.0 || right_scale == 0.0 {
            return Self::try_new(0.0, 0.0, 0.0);
        }

        let direct = [
            direct_determinant(self.y(), other.z(), self.z(), other.y()),
            direct_determinant(self.z(), other.x(), self.x(), other.z()),
            direct_determinant(self.x(), other.y(), self.y(), other.x()),
        ];
        if direct.iter().all(Option::is_some) {
            return Self::try_new(direct[0].unwrap(), direct[1].unwrap(), direct[2].unwrap());
        }

        // Binary scaling preserves significands before near-cancelling products
        // are subtracted. Dividing by arbitrary maxima would round the inputs
        // first, losing accuracy even with a compensated determinant.
        let left_scale =
            Real::from_bits(left_scale.to_bits() & 0x7ff0_0000_0000_0000).max(Real::MIN_POSITIVE);
        let right_scale =
            Real::from_bits(right_scale.to_bits() & 0x7ff0_0000_0000_0000).max(Real::MIN_POSITIVE);
        let left = self.to_array().map(|value| value / left_scale);
        let right = other.to_array().map(|value| value / right_scale);
        let normalized = [
            direct_determinant(left[1], right[2], left[2], right[1]).unwrap(),
            direct_determinant(left[2], right[0], left[0], right[2]).unwrap(),
            direct_determinant(left[0], right[1], left[1], right[0]).unwrap(),
        ];
        let mut result = [0.0; 3];
        for (index, component) in normalized.into_iter().enumerate() {
            // Normalizing the complete vectors may underflow a small coordinate
            // even when its product with another large coordinate is representable.
            // Only replace determinants that actually needed overflow recovery.
            result[index] = if let Some(value) = direct[index] {
                value
            } else {
                component.signum()
                    * product_three(component.abs(), left_scale, right_scale, "cross product")?
            };
        }
        Self::try_from(result)
    }

    pub fn scaled(self, scale: Real) -> Result<Self, GeometryError> {
        require_finite([scale], "scale")?;
        Self::try_new(self.x() * scale, self.y() * scale, self.z() * scale)
    }

    pub fn normalized(self, tolerance: Tolerance) -> Result<UnitVector3, GeometryError> {
        let scale = self.x().abs().max(self.y().abs()).max(self.z().abs());
        if scale == 0.0 {
            return Err(GeometryError::Degenerate { context: "vector" });
        }

        // Divide before taking the norm, so very large vectors remain safe.
        let x = self.x() / scale;
        let y = self.y() / scale;
        let z = self.z() / scale;
        let scaled_length = x.hypot(y).hypot(z);
        if scale <= tolerance.absolute() / scaled_length {
            return Err(GeometryError::Degenerate { context: "vector" });
        }
        self.normalized_with_scale(scale, scaled_length)
    }

    /// Returns the direction of any mathematically non-zero finite vector.
    /// Geometry constructors should normally use [`Self::normalized`] so they
    /// honor model tolerance; this is for recomputing derived data from an
    /// object that has already been validated.
    pub fn normalized_nonzero(self) -> Result<UnitVector3, GeometryError> {
        let scale = self.x().abs().max(self.y().abs()).max(self.z().abs());
        if scale == 0.0 {
            return Err(GeometryError::Degenerate { context: "vector" });
        }
        let scaled_length = (self.x() / scale)
            .hypot(self.y() / scale)
            .hypot(self.z() / scale);
        self.normalized_with_scale(scale, scaled_length)
    }

    fn normalized_with_scale(
        self,
        scale: Real,
        scaled_length: Real,
    ) -> Result<UnitVector3, GeometryError> {
        let x = self.x() / scale;
        let y = self.y() / scale;
        let z = self.z() / scale;
        let inverse_length = 1.0 / scaled_length;
        let vector = Self::try_new(x * inverse_length, y * inverse_length, z * inverse_length)?;
        Ok(UnitVector3(vector))
    }
}

fn direct_determinant(left_a: Real, right_a: Real, left_b: Real, right_b: Real) -> Option<Real> {
    let second = left_b * right_b;
    if second.is_finite() {
        // Compensate the rounded product too. FMA(a,b,-c*d) alone
        // leaves a nonzero product-rounding residual even when a*b == c*d.
        let error = left_b.mul_add(right_b, -second);
        let value = left_a.mul_add(right_a, -second) - error;
        if value.is_finite() {
            return Some(value);
        }
    }
    let first = left_a * right_a;
    if first.is_finite() {
        let error = left_a.mul_add(right_a, -first);
        let value = (-left_b).mul_add(right_b, first) + error;
        if value.is_finite() {
            return Some(value);
        }
    }
    None
}

pub(crate) fn product_three(
    first: Real,
    second: Real,
    third: Real,
    context: &'static str,
) -> Result<Real, GeometryError> {
    if first == 0.0 || second == 0.0 || third == 0.0 {
        return Ok(0.0);
    }

    let mut underflowed = false;
    for (left, right, remaining) in [
        (first, second, third),
        (first, third, second),
        (second, third, first),
    ] {
        let pair = left * right;
        if !pair.is_finite() {
            continue;
        }
        let product = pair * remaining;
        if product.is_finite() {
            if product != 0.0 {
                return Ok(product);
            }
            underflowed = true;
        }
    }

    if underflowed {
        Ok(0.0)
    } else {
        Err(GeometryError::NonFinite { context })
    }
}

/// A finite vector normalized to unit length at construction time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitVector3(pub(crate) Vector3);

impl UnitVector3 {
    pub fn try_new(x: Real, y: Real, z: Real, tolerance: Tolerance) -> Result<Self, GeometryError> {
        Vector3::try_new(x, y, z)?.normalized(tolerance)
    }

    #[inline]
    pub fn x(self) -> Real {
        self.0.x()
    }

    #[inline]
    pub fn y(self) -> Real {
        self.0.y()
    }

    #[inline]
    pub fn z(self) -> Real {
        self.0.z()
    }

    #[inline]
    pub const fn as_vector(self) -> Vector3 {
        self.0
    }

    #[inline]
    pub fn opposite(self) -> Self {
        Self(Vector3(-self.0.0))
    }
}

impl TryFrom<[Real; 3]> for Vector3 {
    type Error = GeometryError;

    fn try_from(value: [Real; 3]) -> Result<Self, Self::Error> {
        Self::try_new(value[0], value[1], value[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_integer_determinants_keep_exact_near_cancellation() {
        for n in [1_i64 << 26, 1_i64 << 27] {
            for (left_exponent, right_exponent) in
                [(0, 0), (-500, -500), (-500, 500), (400, 400), (500, 500)]
            {
                let left_scale = 2.0_f64.powi(left_exponent);
                let right_scale = 2.0_f64.powi(right_exponent);
                let a = Vector3::try_new(n as f64 * left_scale, (n - 1) as f64 * left_scale, 0.0)
                    .unwrap();
                let b = Vector3::try_new((n + 1) as f64 * right_scale, n as f64 * right_scale, 0.0)
                    .unwrap();
                // Integer determinant n*n - (n-1)*(n+1) is exactly one.
                let expected = 2.0_f64.powi(left_exponent + right_exponent);
                assert_eq!(
                    a.cross(b).unwrap().z(),
                    expected,
                    "n={n}, scales={left_exponent},{right_exponent}"
                );
                assert_eq!(b.cross(a).unwrap().z(), -expected);
            }
        }
    }

    #[test]
    fn overflow_fallback_preserves_other_representable_cross_components() {
        for huge in [1e160, 1e200, 1e300, f64::MAX] {
            let small = 1.0 / huge;
            for axis in 0..3 {
                let mut left = [huge, huge, small];
                let mut right = [huge, huge, 0.0];
                let mut expected = [-huge * small, huge * small, 0.0];
                left.rotate_left(axis);
                right.rotate_left(axis);
                expected.rotate_left(axis);
                let left = Vector3::try_from(left).unwrap();
                let right = Vector3::try_from(right).unwrap();
                assert_eq!(left.cross(right).unwrap().to_array(), expected);
                assert_eq!(right.cross(left).unwrap().to_array(), expected.map(|x| -x));
            }
        }
    }

    #[test]
    fn cross_product_still_rejects_a_genuinely_unrepresentable_component() {
        let a = Vector3::try_new(f64::MAX, 0.0, 0.0).unwrap();
        let b = Vector3::try_new(0.0, f64::MAX, 0.0).unwrap();
        assert!(a.cross(b).is_err());
        assert!(b.cross(a).is_err());
    }

    #[test]
    fn cross_product_of_identical_or_opposite_vectors_is_exactly_zero() {
        for coordinates in [
            [0.1, 0.3, 0.7],
            [std::f64::consts::FRAC_1_SQRT_2; 3],
            [1e200, -3e200, 7e200],
            [1e-200, 3e-200, -7e-200],
        ] {
            let vector = Vector3::try_from(coordinates).unwrap();
            assert_eq!(vector.cross(vector).unwrap().to_array(), [0.0; 3]);
            assert_eq!(
                vector
                    .cross(vector.scaled(-1.0).unwrap())
                    .unwrap()
                    .to_array(),
                [0.0; 3]
            );
        }
    }

    #[test]
    fn cross_product_retains_the_difference_of_rounded_near_equal_products() {
        let e = 2.0_f64.powi(-27);
        let a = Vector3::try_new(1.0, 1.0 + e, 0.0).unwrap();
        let b = Vector3::try_new(1.0 + e, 1.0 + 2.0 * e, 0.0).unwrap();
        // Exact binary arithmetic: (1+2e) - (1+e)^2 = -e^2.
        assert_eq!(a.cross(b).unwrap().z(), -e * e);
        assert_eq!(b.cross(a).unwrap().z(), e * e);
    }

    #[test]
    fn normalizes_huge_vectors_without_overflow() {
        let unit = Vector3::try_new(Real::MAX, Real::MAX, 0.0)
            .unwrap()
            .normalized(Tolerance::DEFAULT)
            .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(unit.as_vector().length().unwrap(), 1.0));
    }

    #[test]
    fn rejects_vectors_below_model_tolerance() {
        let vector = Vector3::try_new(1.0e-12, 0.0, 0.0).unwrap();
        assert_eq!(
            vector.normalized(Tolerance::DEFAULT),
            Err(GeometryError::Degenerate { context: "vector" })
        );
    }

    #[test]
    fn degeneracy_uses_vector_length_not_largest_component() {
        let absolute = Tolerance::DEFAULT.absolute();
        let vector = Vector3::try_new(0.8 * absolute, 0.8 * absolute, 0.8 * absolute).unwrap();
        assert!(vector.normalized(Tolerance::DEFAULT).is_ok());
    }

    #[test]
    fn dot_product_handles_large_cancelling_terms() {
        let left = Vector3::try_new(1.0, 1.0, -1.0).unwrap();
        let right = Vector3::try_new(Real::MAX, Real::MAX, Real::MAX).unwrap();
        assert_eq!(left.dot(right).unwrap(), Real::MAX);
    }

    #[test]
    fn dot_product_avoids_spurious_intermediate_underflow() {
        let left = Vector3::try_new(1.0e-200, 0.0, 0.0).unwrap();
        let right = Vector3::try_new(1.0e-108, 0.0, 0.0).unwrap();
        assert_eq!(left.dot(right).unwrap(), 1.0e-308);
    }

    #[test]
    fn dot_product_preserves_small_components_orthogonal_to_large_ones() {
        let left = Vector3::try_new(1e300, 1e-30, -2.0).unwrap();
        let right = Vector3::try_new(0.0, 1.0, 0.0).unwrap();
        assert_eq!(left.dot(right).unwrap(), 1e-30);
        assert_eq!(right.dot(left).unwrap(), 1e-30);
        assert_eq!(
            left.dot(Vector3::try_new(0.0, 0.0, 1.0).unwrap()).unwrap(),
            -2.0
        );
    }

    #[test]
    fn cross_product_is_oriented_and_scaled_without_intermediate_overflow() {
        let x = Vector3::try_new(Real::MAX, 0.0, 0.0).unwrap();
        let y = Vector3::try_new(0.0, 1.0, 0.0).unwrap();
        assert_eq!(
            x.cross(y).unwrap(),
            Vector3::try_new(0.0, 0.0, Real::MAX).unwrap()
        );
        assert_eq!(
            y.cross(x).unwrap(),
            Vector3::try_new(0.0, 0.0, -Real::MAX).unwrap()
        );
        let slender_left = Vector3::try_new(1.0e307, 0.0, 0.0).unwrap();
        let slender_right = Vector3::try_new(1.0e307, 2.0e-9, 0.0).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(
            slender_left.cross(slender_right).unwrap().z() / 1.0e298,
            2.0
        ));
    }
}
