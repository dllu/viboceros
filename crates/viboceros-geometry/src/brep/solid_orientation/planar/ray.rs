//! Exact +X first crossings; boundary, coplanar, and tied hits are unresolved.
use super::*;

pub(super) fn component_sense(faces: &[&PolygonFace], remaining: &mut usize) -> Option<bool> {
    for face in faces {
        if face.normal[0].is_zero() {
            continue;
        }
        let polygon = &face.loops[0];
        for i in 1..polygon.len() - 1 {
            for k in 1..=7 {
                spend(remaining, 1)?;
                // Exact barycentric proposals, not tolerance-dependent ray jitter.
                // Containment and crossing tests alone authorize a result.
                let a = Rational::from_integer(k.into());
                let b = &a * &a;
                let denominator = Rational::from_integer(1.into()) + &a + &b;
                let target: ExactPoint = std::array::from_fn(|axis| {
                    (&polygon[0][axis] + &a * &polygon[i][axis] + &b * &polygon[i + 1][axis])
                        / &denominator
                });
                if let Some(sense) = first_hit(faces, &target[1], &target[2], remaining) {
                    return Some(sense);
                }
            }
        }
    }
    None
}

pub(super) fn first_hit(
    faces: &[&PolygonFace],
    y: &Rational,
    z: &Rational,
    remaining: &mut usize,
) -> Option<bool> {
    let mut first: Option<(Rational, bool)> = None;
    let mut tied = false;
    for face in faces {
        spend(remaining, 1)?;
        if y < &face.bounds[1][0]
            || y > &face.bounds[1][1]
            || z < &face.bounds[2][0]
            || z > &face.bounds[2][1]
        {
            continue;
        }
        let origin = &face.loops[0][0];
        let offset = &face.normal[1] * (y - &origin[1]) + &face.normal[2] * (z - &origin[2]);
        if face.normal[0].is_zero() {
            if offset.is_zero() {
                return None;
            }
            continue;
        }
        let x = &origin[0] - offset / &face.normal[0];
        let p = [x.clone(), y.clone(), z.clone()];
        if !contains(face, &p, remaining)? {
            continue;
        }
        if first.as_ref().is_none_or(|(previous, _)| x < *previous) {
            first = Some((x, face.normal[0].is_positive() ^ face.reversed));
            tied = false;
        } else if first.as_ref().is_some_and(|(previous, _)| x == *previous) {
            tied = true;
        }
    }
    if tied {
        None
    } else {
        first.map(|(_, sense)| sense)
    }
}

pub(super) fn contains(face: &PolygonFace, p: &ExactPoint, remaining: &mut usize) -> Option<bool> {
    let omitted = face.normal.iter().position(|v| !v.is_zero())?;
    let (x, y) = ((omitted + 1) % 3, (omitted + 2) % 3);
    let mut inside = true;
    for (index, polygon) in face.loops.iter().enumerate() {
        let mut winding = 0;
        for i in 0..polygon.len() {
            spend(remaining, 1)?;
            let a = &polygon[i];
            let b = &polygon[(i + 1) % polygon.len()];
            let determinant = (&b[x] - &a[x]) * (&p[y] - &a[y]) - (&b[y] - &a[y]) * (&p[x] - &a[x]);
            if determinant.is_zero()
                && p[x] >= a[x].clone().min(b[x].clone())
                && p[x] <= a[x].clone().max(b[x].clone())
                && p[y] >= a[y].clone().min(b[y].clone())
                && p[y] <= a[y].clone().max(b[y].clone())
            {
                return None;
            }
            if a[y] <= p[y] && b[y] > p[y] && determinant.is_positive() {
                winding += 1;
            }
            if a[y] > p[y] && b[y] <= p[y] && determinant.is_negative() {
                winding -= 1;
            }
        }
        inside &= if index == 0 {
            winding != 0
        } else {
            winding == 0
        };
    }
    Some(inside)
}
