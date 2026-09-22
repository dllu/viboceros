//! Exact planar-polygon extraction and outside-to-inside ray witnesses.
use super::*;
use crate::exact_scalar::{Rational, rational};
use num_traits::{Signed, Zero};

mod face;
mod ray;
#[cfg(test)]
mod tests;

type ExactPoint = [Rational; 3];

struct PolygonFace {
    loops: Vec<Vec<ExactPoint>>,
    normal: ExactPoint,
    reversed: bool,
    bounds: [[Rational; 2]; 3],
}

pub(super) fn classify(brep: &Brep, remaining: &mut usize) -> Option<BrepSolidOrientation> {
    use BrepSolidOrientation::*;
    let faces = brep
        .faces
        .iter()
        .map(|f| face::extract(f, remaining))
        .collect::<Option<Vec<_>>>()?;
    let minimum = faces.iter().map(|f| &f.bounds[0][0]).min()?;
    let mut sense = None;
    for component in brep.edge_connected_face_components() {
        if !component.iter().any(|&i| &faces[i].bounds[0][0] == minimum) {
            continue;
        }
        let component = component.iter().map(|&i| &faces[i]).collect::<Vec<_>>();
        let Some(inward) = ray::component_sense(&component, remaining) else {
            return Some(Unknown);
        };
        if sense.is_some_and(|previous| previous != inward) {
            return Some(Unknown);
        }
        sense = Some(inward);
    }
    Some(match sense {
        Some(true) => Inward,
        Some(false) => Outward,
        None => Unknown,
    })
}

fn spend(remaining: &mut usize, cost: usize) -> Option<()> {
    let next = remaining.checked_sub(cost);
    *remaining = next.unwrap_or(0);
    next.map(|_| ())
}

fn point(p: Point3) -> ExactPoint {
    p.to_array().map(rational)
}
fn sub(a: &ExactPoint, b: &ExactPoint) -> ExactPoint {
    std::array::from_fn(|i| &a[i] - &b[i])
}
fn cross(a: &ExactPoint, b: &ExactPoint) -> ExactPoint {
    std::array::from_fn(|i| &a[(i + 1) % 3] * &b[(i + 2) % 3] - &a[(i + 2) % 3] * &b[(i + 1) % 3])
}
fn dot(a: &ExactPoint, b: &ExactPoint) -> Rational {
    (0..3).map(|i| &a[i] * &b[i]).sum()
}
fn zero(p: &ExactPoint) -> bool {
    p.iter().all(Zero::is_zero)
}

impl PolygonFace {
    fn new(loops: Vec<Vec<ExactPoint>>, normal: ExactPoint, reversed: bool) -> Option<Self> {
        if zero(&normal) || loops.is_empty() || loops.iter().any(|p| p.len() < 3) {
            return None;
        }
        let bounds = std::array::from_fn(|axis| {
            let points = || loops.iter().flatten().map(|p| &p[axis]);
            [
                points().min().unwrap().clone(),
                points().max().unwrap().clone(),
            ]
        });
        Some(Self {
            loops,
            normal,
            reversed,
            bounds,
        })
    }
}
