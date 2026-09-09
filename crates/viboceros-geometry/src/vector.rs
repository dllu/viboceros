use crate::{GeometryError, Real, Tolerance, require_finite};

mod exact_dot;

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
        // the large component is multiplied by zero. Prefer compensated direct
        // evaluation when the products and running sum are representable.
        if let Some(direct) = direct_dot(self.to_array(), other.to_array()) {
            return Ok(direct);
        }
        let result = exact_dot::dot(self.to_array(), other.to_array());
        require_finite([result], "dot product")?;
        Ok(result)
    }

    /// Includes translation in the compensated/exact sum rather than rounding
    /// or rejecting the dot product before adding a cancelling offset.
    pub(crate) fn dot_with_offset(self, other: Self, offset: Real) -> Result<Real, GeometryError> {
        require_finite([offset], "dot product offset")?;
        let left = [self.x(), self.y(), self.z(), offset];
        let right = [other.x(), other.y(), other.z(), 1.];
        let value = direct_dot(left, right).unwrap_or_else(|| exact_dot::dot(left, right));
        require_finite([value], "translated dot product")?;
        Ok(value)
    }

    /// Projection of a point difference without requiring the displacement
    /// itself to be representable. May return signed infinity for callers that
    /// clamp to a finite interval; all input coordinates are validated finite.
    pub(crate) fn dot_point_difference(self, end: crate::Point3, start: crate::Point3) -> Real {
        if let Ok(offset) = start.vector_to(end) {
            // TwoDiff recovers subtraction rounding error. A compensated dot
            // cannot recover coordinate bits already lost in the displacement.
            let exact = end
                .to_array()
                .into_iter()
                .zip(start.to_array())
                .zip(offset.to_array())
                .zip(self.to_array())
                .all(|(((a, b), difference), weight)| {
                    let b_virtual = a - difference;
                    let error = (a - (difference + b_virtual)) + (b_virtual - b);
                    weight == 0. || error == 0.
                });
            if exact && let Ok(value) = offset.dot(self) {
                return value;
            }
        }
        let [x, y, z] = self.to_array();
        exact_dot::dot(
            [
                end.x(),
                end.y(),
                end.z(),
                -start.x(),
                -start.y(),
                -start.z(),
            ],
            [x, y, z, x, y, z],
        )
    }

    pub fn cross(self, other: Self) -> Result<Self, GeometryError> {
        // Each determinant chooses its own fast or exact path. No normalization
        // of unrelated coordinates can erase a component's small remainder.
        let result = [
            determinant(self.y(), other.z(), self.z(), other.y()),
            determinant(self.z(), other.x(), self.x(), other.z()),
            determinant(self.x(), other.y(), self.y(), other.x()),
        ];
        require_finite(result, "cross product")?;
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

// A product has up to 106 significant bits. Below 2^-969 its exact
// low-order bits may lie below 2^-1074, where FMA cannot recover them.
// Include equality because a product may round up to the boundary.
const MIN_COMPENSATED_PRODUCT: Real = 2.0 * Real::MIN_POSITIVE / Real::EPSILON;

fn product_needs_exact_underflow_recovery(product: Real, a: Real, b: Real) -> bool {
    product.abs() <= MIN_COMPENSATED_PRODUCT && a != 0.0 && b != 0.0
}

fn direct_dot<const N: usize>(left: [Real; N], right: [Real; N]) -> Option<Real> {
    let mut sum: Real = 0.0;
    let mut correction = 0.0;
    for (a, b) in left.into_iter().zip(right) {
        let product = a * b;
        if !product.is_finite() || product_needs_exact_underflow_recovery(product, a, b) {
            return None;
        }
        let next = sum + product;
        if !next.is_finite() {
            return None;
        }
        // Recover the rounded product with FMA and the addition with a
        // magnitude-ordered two-sum. Compensating only one of these operations
        // can leave a residual for exactly cancelling products or erase a
        // small term between large opposite terms.
        let addition_error = if sum.abs() >= product.abs() {
            (sum - next) + product
        } else {
            (product - next) + sum
        };
        correction += addition_error + a.mul_add(b, -product);
        sum = next;
    }
    let result = sum + correction;
    result.is_finite().then_some(result)
}

fn determinant(left_a: Real, right_a: Real, left_b: Real, right_b: Real) -> Real {
    direct_determinant(left_a, right_a, left_b, right_b)
        .unwrap_or_else(|| exact_dot::dot([left_a, -left_b, 0.0], [right_a, right_b, 0.0]))
}

fn direct_determinant(left_a: Real, right_a: Real, left_b: Real, right_b: Real) -> Option<Real> {
    let first = left_a * right_a;
    let second = left_b * right_b;
    for (product, a, b) in [(first, left_a, right_a), (second, left_b, right_b)] {
        if product_needs_exact_underflow_recovery(product, a, b) {
            return None;
        }
    }
    if second.is_finite() {
        // Compensate the rounded product too. FMA(a,b,-c*d) alone
        // leaves a nonzero product-rounding residual even when a*b == c*d.
        let error = left_b.mul_add(right_b, -second);
        let value = left_a.mul_add(right_a, -second) - error;
        if value.is_finite() {
            return Some(value);
        }
    }
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
    fn translated_dot_retains_small_terms_before_origin_cancellation() {
        for large in [2_f64.powi(54), 2_f64.powi(500), 2_f64.powi(1023)] {
            for small in [1., -1., f64::MIN_POSITIVE, f64::from_bits(1)] {
                let left = Vector3::try_new(large, small, 0.).unwrap();
                let right = Vector3::try_new(1., 1., 0.).unwrap();
                assert_eq!(left.dot_with_offset(right, -large).unwrap(), small);
            }
        }
    }

    #[test]
    fn dot_product_keeps_low_bits_of_normal_products_near_underflow() {
        let e = Real::EPSILON;
        let scale = 2.0_f64.powi(-486);
        let left = [
            (1.0 + e) * scale,
            (1.0 + e) * scale,
            -(2.0 + 6.0 * e) * scale,
        ];
        let right = [(1.0 + 2.0 * e) * scale, (1.0 + 2.0 * e) * scale, scale];
        // Exact sum: 2*(1+e)*(1+2e) - (2+6e) = 4e^2.
        // After scaling this is 2^-1074. Each positive product has a
        // half-subnormal residual which rounds to zero if recovered alone.
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let a = Vector3::try_from(order.map(|i| left[i])).unwrap();
            let b = Vector3::try_from(order.map(|i| right[i])).unwrap();
            assert!(
                left.into_iter()
                    .zip(right)
                    .all(|(x, y)| (x * y).is_normal())
            );
            assert_eq!(a.dot(b).unwrap(), Real::from_bits(1));
            assert_eq!(b.dot(a).unwrap(), Real::from_bits(1));
        }
    }

    #[test]
    fn cross_product_combines_subnormal_products_before_rounding() {
        let tiny = Real::from_bits(1);
        for axis in 0..3 {
            let mut left = [tiny, tiny, 0.0];
            let mut right = [0.5, -0.5, 0.0];
            let mut expected = [0.0, 0.0, -tiny];
            left.rotate_left(axis);
            right.rotate_left(axis);
            expected.rotate_left(axis);
            let a = Vector3::try_from(left).unwrap();
            let b = Vector3::try_from(right).unwrap();
            assert_eq!(a.cross(b).unwrap().to_array(), expected);
            assert_eq!(b.cross(a).unwrap().to_array(), expected.map(|v| -v));
        }
    }

    #[test]
    fn cross_product_keeps_subnormal_difference_of_normal_products() {
        let scale = 2.0_f64.powi(-537);
        for exponent in [27, 40, 52] {
            let n = (1_u64 << exponent) as f64;
            let a = Vector3::try_new(n * scale, (n - 1.0) * scale, 0.0).unwrap();
            let b = Vector3::try_new((n + 1.0) * scale, n * scale, 0.0).unwrap();
            assert!((a.x() * b.y()).is_normal());
            assert!((a.y() * b.x()).is_normal());
            // n*n - (n-1)*(n+1) = 1, scaled by 2^-1074.
            assert_eq!(a.cross(b).unwrap().z(), Real::from_bits(1));
            assert_eq!(b.cross(a).unwrap().z(), -Real::from_bits(1));
        }
    }

    #[test]
    fn dot_product_preserves_small_remainder_after_overflowing_cancellation() {
        for huge in [1e200, f64::MAX] {
            for small in [1.0, 1e-100, f64::from_bits(1)] {
                for axis in 0..3 {
                    let mut left = [huge, huge, small];
                    let mut right = [huge, -huge, 1.0];
                    left.rotate_left(axis);
                    right.rotate_left(axis);
                    let a = Vector3::try_from(left).unwrap();
                    let b = Vector3::try_from(right).unwrap();
                    assert_eq!(a.dot(b).unwrap(), small);
                    assert_eq!(b.dot(a).unwrap(), small);
                }
            }
        }
    }

    #[test]
    fn dot_product_combines_subnormal_products_before_rounding() {
        let tiny = Real::from_bits(1);
        let a = Vector3::try_new(tiny, tiny, tiny).unwrap();
        let b = Vector3::try_new(0.5, 0.5, 0.5).unwrap();
        assert_eq!(a.dot(b).unwrap(), Real::from_bits(2));
        // Each product rounds upward individually; rounding their sum once
        // produces two units, not three.
        let b = Vector3::try_new(0.75, 0.75, 0.75).unwrap();
        assert_eq!(a.dot(b).unwrap(), Real::from_bits(2));
        let huge = Vector3::try_new(Real::MAX, Real::MAX, Real::MAX).unwrap();
        assert!(huge.dot(huge).is_err());
    }

    #[test]
    fn dot_product_compensates_product_and_sum_rounding() {
        for magnitude in [0.1, 0.7, 1e100] {
            let a = Vector3::try_new(magnitude, magnitude, 0.0).unwrap();
            let b = Vector3::try_new(magnitude, -magnitude, 0.0).unwrap();
            assert_eq!(a.dot(b).unwrap(), 0.0);
            assert_eq!(b.dot(a).unwrap(), 0.0);
        }
        // Both products round to the same value, but their exact difference is one.
        let n = (1_u64 << 27) as f64;
        let a = Vector3::try_new(n, n - 1.0, 0.0).unwrap();
        let b = Vector3::try_new(n, -(n + 1.0), 0.0).unwrap();
        assert_eq!(a.dot(b).unwrap(), 1.0);
        assert_eq!(b.dot(a).unwrap(), 1.0);

        for axis in 0..3 {
            let mut coordinates = [1e100, 1.0, -1e100];
            coordinates.rotate_left(axis);
            let a = Vector3::try_from(coordinates).unwrap();
            let b = Vector3::try_new(1.0, 1.0, 1.0).unwrap();
            assert_eq!(a.dot(b).unwrap(), 1.0);
            assert_eq!(b.dot(a).unwrap(), 1.0);
        }
    }

    #[test]
    fn dot_product_matches_integer_oracle_across_axis_orders() {
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for exponent in [26, 27, 40, 52] {
            let n = 1_i128 << exponent;
            for third in [-3, 0, 7] {
                let left = [n, n - 1, third];
                let right = [n, -(n + 1), 1];
                let expected = left
                    .into_iter()
                    .zip(right)
                    .map(|(a, b)| a * b)
                    .sum::<i128>();
                for order in permutations {
                    let a = Vector3::try_from(order.map(|axis| left[axis] as f64)).unwrap();
                    let b = Vector3::try_from(order.map(|axis| right[axis] as f64)).unwrap();
                    assert_eq!(a.dot(b).unwrap(), expected as f64);
                    assert_eq!(b.dot(a).unwrap(), expected as f64);
                }
            }
        }
    }

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
