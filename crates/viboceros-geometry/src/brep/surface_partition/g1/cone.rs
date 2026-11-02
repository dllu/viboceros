//! Positive Bernstein combinations retain a common acute angular cone.
use super::*;
use crate::bezier::polynomial::subdivide;

const MAX_DEPTH: u8 = 64;
const ROUND_GUARD: Real = 128. * Real::EPSILON;

pub(super) fn has_break(
    initial: Seam,
    cosine: Real,
    budget: &mut usize,
) -> Result<bool, GeometryError> {
    let mut stack = vec![initial];
    while let Some(seam) = stack.pop() {
        charge(budget, 1)?;
        let [a, b] = &seam.directions;
        let positive_weight =
            seam.weights.iter().all(|w| w[0] > 0.) || seam.weights.iter().all(|w| w[0] < 0.);
        if positive_weight && cosine == 1. {
            charge(budget, (a.len() + b.len()).saturating_mul(64))?;
            if super::exact_ray::same(a, b) {
                continue;
            }
        }
        let constant_equal = a == b && a[0] != [0.; 3] && a.iter().all(|v| *v == a[0]);
        if positive_weight && (constant_equal || within_cone(a, b, cosine, budget)?) {
            continue;
        }
        if seam.depth >= MAX_DEPTH {
            return unresolved();
        }
        let [al, ar] = subdivide(a, &mut |n| charge(budget, n))?;
        let [bl, br] = subdivide(b, &mut |n| charge(budget, n))?;
        let [wl, wr] = subdivide(&seam.weights, &mut |n| charge(budget, n))?;
        if wl.last().unwrap()[0] != 0.
            && super::angle::compare(*al.last().unwrap(), *bl.last().unwrap(), cosine, budget)?
                .is_some_and(|order| order.is_lt())
        {
            return Ok(true);
        }
        let depth = seam.depth + 1;
        stack.push(Seam {
            directions: [ar, br],
            weights: wr,
            depth,
        });
        stack.push(Seam {
            directions: [al, bl],
            weights: wl,
            depth,
        });
    }
    Ok(false)
}

fn within_cone(
    a: &[[Real; 3]],
    b: &[[Real; 3]],
    cosine: Real,
    budget: &mut usize,
) -> Result<bool, GeometryError> {
    charge(budget, a.len().saturating_mul(b.len()).saturating_mul(12))?;
    let threshold = cosine.max(0.);
    let normalize = |values: &[[Real; 3]]| -> Result<Vec<([Real; 3], Vector3)>, GeometryError> {
        let mut directions = Vec::with_capacity(values.len());
        for &value in values {
            let vector = Vector3::try_from(value)?;
            if vector.length()? > 0. {
                directions.push((value, vector.normalized_nonzero()?.as_vector()));
            }
        }
        Ok(directions)
    };
    let (a, b) = (normalize(a)?, normalize(b)?);
    let mut acute = !a.is_empty() && !b.is_empty();
    'pairs: for &(raw_a, a) in &a {
        for &(raw_b, b) in &b {
            let order = super::angle::compare_with_dot(raw_a, raw_b, threshold, a.dot(b)?, budget)?;
            if !(if threshold > 0. {
                order.is_ge()
            } else {
                order.is_gt()
            }) {
                acute = false;
                break 'pairs;
            }
        }
    }
    if acute {
        return Ok(true);
    }
    if cosine >= 0. || a.is_empty() || b.is_empty() {
        return Ok(false);
    }
    // Each individual cone remains acute; their centers may have an obtuse
    // separation. The spherical triangle inequality bounds their sum.
    let radius = |values: &[([Real; 3], Vector3)]| -> Result<Option<Real>, GeometryError> {
        let center = values[0].1;
        let mut minimum = 1_f64;
        for &(_, v) in values {
            minimum = minimum.min(center.dot(v)?);
        }
        if minimum <= ROUND_GUARD {
            return Ok(None);
        }
        Ok(Some((minimum - ROUND_GUARD).clamp(-1., 1.).acos()))
    };
    let (Some(ra), Some(rb)) = (radius(&a)?, radius(&b)?) else {
        return Ok(false);
    };
    let separation = (a[0].1.dot(b[0].1)? - ROUND_GUARD).clamp(-1., 1.).acos();
    Ok(separation + ra + rb < cosine.acos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_angle_inside_the_roundoff_guard_resolves_on_either_side() {
        for (cosine, expected) in [(0.6_f64.next_down(), false), (0.6_f64.next_up(), true)] {
            let seam = Seam {
                directions: [
                    vec![[1., 0., 0.], [2., 0., 0.]],
                    vec![[3., 4., 0.], [6., 8., 0.]],
                ],
                weights: vec![[1.], [1.]],
                depth: 0,
            };
            assert_eq!(has_break(seam, cosine, &mut 16_384).unwrap(), expected);
        }
    }
}
