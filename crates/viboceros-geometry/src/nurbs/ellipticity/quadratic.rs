//! Exact center of a nondegenerate rational quadratic conic.
use super::super::exact::{Rational, rational, scalar};
use super::*;
use num_traits::Zero;

fn ratio(span: &NurbsCurve) -> Option<Rational> {
    let p = span.control_points();
    let sign = p[0].weight().is_sign_positive();
    if p.iter().any(|p| p.weight().is_sign_positive() != sign) {
        return None;
    }
    let a = rational(p[0].weight()) * rational(p[2].weight());
    let b = rational(p[1].weight()).pow(2);
    (a > b && b > Rational::zero()).then(|| b / a)
}

pub(super) fn proposal(
    spans: &[NurbsCurve],
    tolerance: Tolerance,
) -> Result<Option<EllipticLocus>, GeometryError> {
    // Choose the widest elliptic span, so refinement at a nearly stationary end
    // does not dominate the candidate. Only one whole-curve certification is
    // needed, avoiding quadratic work on an almost-elliptic many-span curve.
    let mut best: Option<(&NurbsCurve, Rational)> = None;
    for span in spans {
        if let Some(q) = ratio(span)
            && best.as_ref().is_none_or(|(_, old)| q < *old)
        {
            best = Some((span, q));
        }
    }
    let Some((span, q)) = best else {
        return Ok(None);
    };
    let p = span.control_points();
    let one_minus = rational(1.) - &q;
    let mut center = [0.; 3];
    let mut tangent = [0.; 3];
    let mut chord = [0.; 3];
    for i in 0..3 {
        let a = rational(p[0].point().to_array()[i]);
        let b = rational(p[1].point().to_array()[i]);
        let c = rational(p[2].point().to_array()[i]);
        let middle = (&a + &c) / rational(2.);
        center[i] = scalar(&((&middle - &q * &b) / &one_minus))?;
        tangent[i] = scalar(&(b - &middle))?;
        chord[i] = scalar(&((c - a) / rational(2.)))?;
    }
    let complement = scalar(&one_minus)?;
    if complement <= 0. {
        return Ok(None);
    }
    let u_scale = scalar(&q)?.sqrt() / complement;
    let v_scale = 1. / complement.sqrt();
    // P0=C+u cos(alpha)-v sin(alpha), P2=C+u cos(alpha)+v sin(alpha),
    // P1=C+u/cos(alpha), with cos²(alpha)=q. The columns may be oblique.
    let matrix = Mat::from_fn(3, 2, |i, j| {
        if j == 0 {
            tangent[i] * u_scale
        } else {
            chord[i] * v_scale
        }
    });
    if (0..3).any(|i| (0..2).any(|j| !matrix[(i, j)].is_finite())) {
        return Ok(None);
    }
    let Ok(svd) = matrix.thin_svd() else {
        return Ok(None);
    };
    let radii = [svd.S().column_vector()[0], svd.S().column_vector()[1]];
    if radii
        .iter()
        .any(|r| !r.is_finite() || *r <= tolerance.absolute())
    {
        return Ok(None);
    }
    let axis = |j| {
        Vector3::try_from(std::array::from_fn(|i| svd.U()[(i, j)]))?
            .normalized_nonzero()
            .map(|u| u.as_vector())
    };
    let axes = [axis(0)?, axis(1)?];
    Ok(Some(EllipticLocus {
        center: Point3::try_from(center)?,
        axes,
        normal: axes[0].cross(axes[1])?.normalized_nonzero()?.as_vector(),
        radii,
    }))
}
