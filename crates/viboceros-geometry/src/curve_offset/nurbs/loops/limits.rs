//! Necessary support-strip clearance before searching collapsed offset loci.
use super::*;

pub(super) fn collapsed(
    source: &NurbsCurve,
    normal: UnitVector3,
    inward: Real,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let c = source.control_points();
    if inward <= 0.
        || c.iter()
            .any(|p| p.weight().is_sign_positive() != c[0].weight().is_sign_positive())
    {
        return Ok(false);
    }
    let n = normal.as_vector();
    for i in 0..3 {
        let axis = Vector3::try_from(std::array::from_fn(|j| if i == j { 1. } else { 0. }))?;
        let side = axis.cross(n)?;
        if side.length()? == 0. {
            continue;
        }
        let exact = n.to_array()[i] == 0.;
        let direction = if exact {
            axis
        } else {
            n.cross(side)?.normalized_nonzero()?.as_vector()
        };
        let mut lo = 0.0_f64;
        let mut hi = 0.0_f64;
        for p in c {
            let x = c[0].point().vector_to(p.point())?.dot(direction)?;
            lo = lo.min(x);
            hi = hi.max(x);
        }
        // Same-sign rational curves lie inside their control hull. Any disk
        // of inward clearance must fit in this enclosing support strip.
        let half_width = hi * 0.5 - lo * 0.5;
        let margin = tolerance.absolute().max(tolerance.relative() * half_width);
        if inward > half_width + margin || exact && inward >= half_width {
            return Ok(true);
        }
    }
    Ok(false)
}
