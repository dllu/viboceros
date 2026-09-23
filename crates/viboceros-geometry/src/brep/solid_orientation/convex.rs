//! Exact, deliberately narrow certificates for convex polyhedral shell nesting.
use super::planar::{ExactPoint, cross, dot, extract_face, point, sub, zero};
use super::*;
use num_traits::Signed;

struct ConvexShell {
    vertices: Vec<ExactPoint>,
    edges: Vec<ExactPoint>,
    planes: Vec<(ExactPoint, ExactPoint)>, // outward normal, point on plane
    outward: bool,
}

pub(super) fn shell_order(brep: &Brep) -> Option<Vec<usize>> {
    if !brep.is_solid() {
        return None;
    }
    let mut remaining = EXACT_WORK_LIMIT;
    let components = brep.edge_connected_face_components();
    let shells = components
        .iter()
        .map(|faces| extract_shell(brep, faces, &mut remaining))
        .collect::<Option<Vec<_>>>()?;
    let candidates = (0..shells.len())
        .filter(|&outer| {
            shells[outer].outward
                && (0..shells.len()).all(|inner| {
                    inner == outer
                        || (!shells[inner].outward
                            && contains(&shells[outer], &shells[inner], &mut remaining)
                                == Some(true))
                })
        })
        .collect::<Vec<_>>();
    let [outer] = candidates.as_slice() else {
        return None;
    };
    let cavities = (0..shells.len())
        .filter(|&index| index != *outer)
        .collect::<Vec<_>>();
    for (i, &left) in cavities.iter().enumerate() {
        for &right in &cavities[i + 1..] {
            if separated(&shells[left], &shells[right], &mut remaining) != Some(true) {
                return None;
            }
        }
    }
    Some(std::iter::once(*outer).chain(cavities).collect())
}

fn extract_shell(brep: &Brep, faces: &[usize], remaining: &mut usize) -> Option<ConvexShell> {
    let polygons = faces
        .iter()
        .map(|&index| extract_face(&brep.faces[index], remaining))
        .collect::<Option<Vec<_>>>()?;
    let mut vertices = Vec::new();
    let mut edges = Vec::new();
    for (index, polygon) in faces.iter().zip(&polygons) {
        let [boundary] = brep.faces[*index].loops.as_slice() else {
            return None;
        };
        let [ring] = polygon.loops.as_slice() else {
            return None;
        };
        if ring.len() != boundary.trims.len() {
            return None;
        }
        for (p, trim) in ring.iter().zip(&boundary.trims) {
            spend(remaining, 1)?;
            if *p != point(brep.vertices[trim.vertices[0]].point) {
                return None;
            }
            let edge = &brep.edges[trim.edge?];
            let controls = edge.curve.control_points();
            if edge.curve.degree() != 1
                || controls.len() != 2
                || controls[0].weight().is_sign_positive()
                    != controls[1].weight().is_sign_positive()
                || controls[0].point() != brep.vertices[edge.vertices[0]].point
                || controls[1].point() != brep.vertices[edge.vertices[1]].point
            {
                return None;
            }
        }
        for i in 0..ring.len() {
            spend(remaining, 1)?;
            let a = &ring[i];
            let b = &ring[(i + 1) % ring.len()];
            let c = &ring[(i + 2) % ring.len()];
            let ab = sub(b, a);
            let bc = sub(c, b);
            if !dot(&polygon.normal, &cross(&ab, &bc)).is_positive() {
                return None;
            }
            edges.push(ab);
            vertices.push(a.clone());
        }
    }
    let mut planes = Vec::with_capacity(polygons.len());
    let mut outward = None;
    for polygon in &polygons {
        let origin = &polygon.loops[0][0];
        let mut positive = false;
        let mut negative = false;
        for vertex in &vertices {
            spend(remaining, 1)?;
            let side = dot(&polygon.normal, &sub(vertex, origin));
            positive |= side.is_positive();
            negative |= side.is_negative();
            if positive && negative {
                return None;
            }
        }
        if !positive && !negative {
            return None;
        }
        let face_outward = negative ^ polygon.reversed;
        if outward.is_some_and(|sense| sense != face_outward) {
            return None;
        }
        outward = Some(face_outward);
        let normal = if polygon.reversed {
            polygon.normal.clone().map(|v| -v)
        } else {
            polygon.normal.clone()
        };
        planes.push((normal, origin.clone()));
    }
    Some(ConvexShell {
        vertices,
        edges,
        planes,
        outward: outward?,
    })
}

fn contains(outer: &ConvexShell, inner: &ConvexShell, remaining: &mut usize) -> Option<bool> {
    for (normal, origin) in &outer.planes {
        for vertex in &inner.vertices {
            spend(remaining, 1)?;
            if !dot(normal, &sub(vertex, origin)).is_negative() {
                return Some(false);
            }
        }
    }
    Some(true)
}

// Exact separating-axis theorem: face normals and every edge-edge cross axis.
fn separated(left: &ConvexShell, right: &ConvexShell, remaining: &mut usize) -> Option<bool> {
    for axis in left
        .planes
        .iter()
        .chain(right.planes.iter())
        .map(|(normal, _)| normal.clone())
        .chain(
            left.edges
                .iter()
                .flat_map(|a| right.edges.iter().map(move |b| cross(a, b))),
        )
    {
        if zero(&axis) {
            continue;
        }
        spend(remaining, left.vertices.len() + right.vertices.len())?;
        let mut l = left.vertices.iter().map(|p| dot(&axis, p));
        let mut r = right.vertices.iter().map(|p| dot(&axis, p));
        let first_l = l.next()?;
        let first_r = r.next()?;
        let (mut l_min, mut l_max) = (first_l.clone(), first_l);
        let (mut r_min, mut r_max) = (first_r.clone(), first_r);
        for value in l {
            l_min = l_min.min(value.clone());
            l_max = l_max.max(value);
        }
        for value in r {
            r_min = r_min.min(value.clone());
            r_max = r_max.max(value);
        }
        if l_max < r_min || r_max < l_min {
            return Some(true);
        }
    }
    Some(false)
}
