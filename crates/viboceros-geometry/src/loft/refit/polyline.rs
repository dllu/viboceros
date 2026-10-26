//! Refit certified straight regions, retaining genuine corners and reversals.
use super::*;

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
    // The legacy public Refit uses a one-degree corner threshold. Shallower
    // bends enter the smooth fitter, whose policy is not implemented yet.
    // Retain a small conservative margin at the floating angular boundary.
    let cosine = rational((std::f64::consts::PI / 180.).cos() - 16. * Real::EPSILON);
    for triple in vertices.windows(3) {
        let a = std::array::from_fn::<_, 3, _>(|i| &triple[1][i] - &triple[0][i]);
        let b = std::array::from_fn::<_, 3, _>(|i| &triple[2][i] - &triple[1][i]);
        let dot = (0..3).map(|i| &a[i] * &b[i]).sum::<Rational>();
        let aa = a.iter().map(|x| x * x).sum::<Rational>();
        let bb = b.iter().map(|x| x * x).sum::<Rational>();
        if dot.is_positive() && &dot * &dot >= &aa * &bb * &cosine * &cosine {
            return Err(unsupported());
        }
        // Compare the signed normalized dot product without square roots.
        // Squaring reverses the ordering when both sides are negative.
        if let Some(angle) = angle {
            // A zero dot product is exact at a right angle. Other floating
            // angular boundaries retain the legacy conservative margin.
            let margin = if dot.is_zero() {
                0.
            } else {
                16. * Real::EPSILON
            };
            let c = rational(angle.cos() - margin);
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
    }
    let mut controls = Vec::with_capacity(count);
    let mut knots = vec![0.; 4];
    let mut length = 0.;
    for pair in vertices.windows(2) {
        let segment = cubic_controls(&pair[0], &pair[1], tolerance)?;
        let distance = segment[0].point().distance_to(segment[3].point())?;
        let next = length + distance;
        if !next.is_finite() || next <= length {
            return Err(unsupported());
        }
        length = next;
        controls.extend(segment.into_iter().skip(usize::from(!controls.is_empty())));
        knots.extend([length; 3]);
    }
    knots.push(length);
    Ok(Section {
        curve: NurbsCurve::try_new_rational(3, controls, knots)?,
        vertices,
    })
}
