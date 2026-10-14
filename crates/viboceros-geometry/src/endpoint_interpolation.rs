//! Exact minimum-degree Bezier interpolation of independent endpoint jets.
use crate::{GeometryError, Real};

const MAX_COLUMNS: usize = 4096;

/// Position and optional first/second derivatives on a normalized [0,1] chart.
/// Rows at the first endpoint have ascending derivative order; rows at the
/// second have descending order. Each row contains `column_count` quadruples
/// of independent channels, which can represent homogeneous spline controls.
pub struct EndpointInterpolation<'a> {
    pub orders: [u8; 2],
    pub column_count: usize,
    pub targets: &'a [[Real; 4]],
}
impl EndpointInterpolation<'_> {
    pub fn degree(&self) -> usize {
        usize::from(self.orders[0]) + usize::from(self.orders[1]) + 1
    }
    pub fn validate(&self) -> Result<(), GeometryError> {
        if self.orders.iter().any(|order| *order > 2)
            || self.column_count == 0
            || self.column_count > MAX_COLUMNS
            || self.targets.len() != self.column_count * (self.degree() + 1)
        {
            return Err(GeometryError::InvalidEndpointInterpolation {
                context: "invalid orders or dimensions",
            });
        }
        crate::require_finite(
            self.targets.iter().flat_map(|h| *h),
            "endpoint interpolation targets",
        )
    }
}

/// Portable endpoint interpolation, shared by curve and surface blends.
pub fn try_interpolate_endpoints(
    input: &EndpointInterpolation<'_>,
) -> Result<Vec<[Real; 4]>, GeometryError> {
    input.validate()?;
    let degree = input.degree();
    let count = input.column_count;
    let mut net = vec![[0.; 4]; input.targets.len()];
    for end in 0..2 {
        let row = if end == 0 { 0 } else { degree };
        let direction = if end == 0 { 1. } else { -1. };
        for u in 0..count {
            let h = input.targets[row * count + u];
            for order in 0..=usize::from(input.orders[end]) {
                let row = if end == 0 { order } else { degree - order };
                for k in 0..4 {
                    let d = if order > 0 {
                        input.targets[if end == 0 {
                            count + u
                        } else {
                            (degree - 1) * count + u
                        }][k]
                            * direction
                    } else {
                        0.
                    };
                    net[row * count + u][k] = match order {
                        0 => h[k],
                        1 => h[k] + d / degree as Real,
                        2 => {
                            h[k] + 2. * d / degree as Real
                                + input.targets[row * count + u][k]
                                    / (degree * (degree - 1)) as Real
                        }
                        _ => unreachable!(),
                    };
                }
            }
        }
    }
    Ok(net)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_endpoint_orders_and_dimensions_are_rejected_without_overflow() {
        for (orders, column_count) in [
            ([255, 255], usize::MAX),
            ([3, 0], 1),
            ([0, 0], 0),
            ([2, 2], MAX_COLUMNS + 1),
            ([2, 2], usize::MAX),
            ([1, 1], 1),
        ] {
            assert!(
                try_interpolate_endpoints(&EndpointInterpolation {
                    orders,
                    column_count,
                    targets: &[],
                })
                .is_err()
            );
        }
    }
}
