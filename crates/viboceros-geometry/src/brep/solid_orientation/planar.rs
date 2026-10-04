//! Exact planar-polygon extraction and outside-to-inside ray witnesses.
use super::*;
use crate::exact_scalar::{Rational, rational};
use num_traits::{Signed, Zero};

mod face;
mod ray;
#[cfg(test)]
mod tests;

pub(in crate::brep) type ExactPoint = [Rational; 3];

pub(in crate::brep) struct PolygonFace {
    pub(in crate::brep) loops: Vec<Vec<ExactPoint>>,
    pub(in crate::brep) normal: ExactPoint,
    pub(in crate::brep) reversed: bool,
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

pub(in crate::brep) fn extract_face(face: &BrepFace, remaining: &mut usize) -> Option<PolygonFace> {
    face::extract(face, remaining)
}

pub(in crate::brep) fn point(p: Point3) -> ExactPoint {
    p.to_array().map(rational)
}
pub(in crate::brep) fn sub(a: &ExactPoint, b: &ExactPoint) -> ExactPoint {
    std::array::from_fn(|i| &a[i] - &b[i])
}
pub(in crate::brep) fn cross(a: &ExactPoint, b: &ExactPoint) -> ExactPoint {
    std::array::from_fn(|i| &a[(i + 1) % 3] * &b[(i + 2) % 3] - &a[(i + 2) % 3] * &b[(i + 1) % 3])
}
pub(in crate::brep) fn dot(a: &ExactPoint, b: &ExactPoint) -> Rational {
    (0..3).map(|i| &a[i] * &b[i]).sum()
}
pub(in crate::brep) fn zero(p: &ExactPoint) -> bool {
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
