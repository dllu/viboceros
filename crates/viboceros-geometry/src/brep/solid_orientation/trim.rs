//! Exact oriented polygon images of conservative classes of UV NURBS curves.
use super::*;
use crate::exact_scalar::rational;

/// Degree-one curves retain their entire control polygon. Higher-degree curves
/// need an exact segment certificate: clamped endpoints, C0 continuity, same-sign
/// nonzero weights, and every control in the closed endpoint segment. The convex
/// hull bounds the image; continuity and the attained endpoints fill the segment.
/// Parameter speed need not be constant or monotone: retraces on that segment
/// cancel in winding tests. No sampled position or tangent authorizes this proof.
pub(super) fn polygon<'a>(
    curve: &'a NurbsCurve2,
    remaining: &mut usize,
) -> Option<impl DoubleEndedIterator<Item = [Real; 2]> + ExactSizeIterator + 'a> {
    let degree = curve.degree();
    let controls = curve.control_points();
    spend(remaining, controls.len().saturating_mul(8))?;
    let knots = curve.knots();
    if knots[0] != knots[degree] || knots[controls.len()] != *knots.last()? {
        return None;
    }
    // The validated knot vector is sorted with multiplicities <= degree + 1.
    // Interior full-order breaks have no continuity certificate, even if their
    // control polygon happens to be collinear.
    let mut multiplicity = 1;
    for pair in knots[degree + 1..controls.len()].windows(2) {
        multiplicity = if pair[0] == pair[1] {
            multiplicity + 1
        } else {
            1
        };
        if multiplicity > degree {
            return None;
        }
    }
    let sign = controls[0].weight().is_sign_positive();
    if controls
        .iter()
        .any(|p| p.weight().is_sign_positive() != sign)
    {
        return None;
    }
    let stride = if degree == 1 {
        1
    } else {
        let a = controls[0].point().to_array();
        let b = controls.last()?.point().to_array();
        if a == b
            || !controls
                .iter()
                .all(|p| in_segment(p.point().to_array(), a, b))
        {
            return None;
        }
        controls.len() - 1
    };
    // Borrow the whole polygon or just its endpoints; no per-trim allocation.
    Some(
        controls
            .iter()
            .step_by(stride)
            .map(|p| p.point().to_array()),
    )
}

fn in_segment(p: [Real; 2], a: [Real; 2], b: [Real; 2]) -> bool {
    if (0..2).any(|i| p[i] < a[i].min(b[i]) || p[i] > a[i].max(b[i])) {
        return false;
    }
    // For axis-aligned sides the comparison-only bounds already prove it.
    if a[0] == b[0] || a[1] == b[1] || p == a || p == b {
        return true;
    }
    // Convert before subtraction: rounded differences/products could hide a
    // tiny bulge, overflow, or underflow. Binary64 coefficients are exact input.
    let [ax, ay] = a.map(rational);
    let [bx, by] = b.map(rational);
    let [px, py] = p.map(rational);
    (px - &ax) * (by - &ay) == (py - &ay) * (bx - &ax)
}
