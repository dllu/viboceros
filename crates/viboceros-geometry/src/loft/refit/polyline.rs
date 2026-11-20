//! Refit certified straight regions, retaining genuine corners and reversals.
use super::*;
use crate::curve_fit::cubic_polygon;

pub(super) fn refit(
    source: &NurbsCurve,
    tolerance: Real,
    angle: Option<Real>,
) -> Result<Section, GeometryError> {
    let unsupported = || GeometryError::UnsupportedCurveApproximation {
        context: "Loft Refit currently requires straight regions with resolved corners; smooth fitting is pending",
    };
    let vertices =
        crate::surface_pullback::exact_linear_curve_vertices(source)?.ok_or_else(unsupported)?;
    let count = 3 * (vertices.len() - 1) + 1;
    if count > MAX_LOFT_SECTION_CONTROLS {
        return Err(GeometryError::LoftResourceLimit {
            context: "Refit polynomial section controls",
            maximum: MAX_LOFT_SECTION_CONTROLS,
        });
    }
    let cosine = corner_angle(angle).cos();
    for triple in vertices.windows(3) {
        let a = std::array::from_fn::<_, 3, _>(|i| &triple[1][i] - &triple[0][i]);
        let b = std::array::from_fn::<_, 3, _>(|i| &triple[2][i] - &triple[1][i]);
        let dot = (0..3).map(|i| &a[i] * &b[i]).sum::<Rational>();
        let aa = a.iter().map(|x| x * x).sum::<Rational>();
        let bb = b.iter().map(|x| x * x).sum::<Rational>();
        // Exact orthogonality retains the public right-angle boundary. Other
        // floating angular boundaries keep a conservative roundoff margin.
        let c = rational(
            cosine
                - if dot.is_zero() {
                    0.
                } else {
                    16. * Real::EPSILON
                },
        );
        // Compare signed normalized dot products without square roots.
        let retained = if dot.is_negative() && !c.is_negative() {
            true
        } else if !dot.is_negative() && c.is_negative() {
            false
        } else if dot.is_negative() {
            &dot * &dot > &aa * &bb * &c * &c
        } else {
            &dot * &dot < &aa * &bb * &c * &c
        };
        if !retained {
            return Err(unsupported());
        }
    }
    Ok(Section {
        curve: cubic_polygon(&vertices, tolerance)?,
        vertices,
    })
}
