//! Stable mean spatial radius for a distant algebraic circle center.
use super::*;

pub(super) fn radius(points: &[[Real; 3]], center: Vector2<Real>) -> Result<Real, GeometryError> {
    let length = center.norm();
    let inverse = 1. / length;
    let normal = center * inverse;
    let mut sum = FiniteSum::default();
    for p in points {
        let square = p.iter().map(|v| v * v).sum::<Real>();
        let q = 0.5 * inverse * square - normal.dot(&Vector2::new(p[0], p[1]));
        let root = (1. + 2. * inverse * q).sqrt();
        // d - |center| = 2*q/(sqrt(1 + 2*q/|center|) + 1).
        // Accumulate small offsets, then add the large center length once.
        let delta = 2. * q / (root + 1.);
        if !root.is_finite() || root <= 0. || !delta.is_finite() {
            return Err(GeometryError::CircleFitDidNotConverge);
        }
        sum.add(delta)?;
    }
    let radius = length + sum.mean()?;
    crate::require_finite([radius], "distant fitted circle radius")?;
    Ok(radius)
}
