//! Exact axis-box witnesses before giving disconnected shells STEP void semantics.
use std::collections::BTreeSet;
use viboceros_geometry::Brep;

struct AxisBox {
    min: [f64; 3],
    max: [f64; 3],
    outward: bool,
}

/// Returns outer-first component order only when one closed axis box strictly
/// contains disjoint inward-oriented box cavities. Coincident, touching,
/// overlapping, nested-void and non-box shells remain separate surface models.
pub(super) fn solid_shell_order(brep: &Brep) -> Option<Vec<usize>> {
    if !brep.is_solid() {
        return None;
    }
    let components = brep.edge_connected_face_components();
    let boxes = components
        .iter()
        .map(|faces| classify_box(brep, faces))
        .collect::<Option<Vec<_>>>()?;
    let candidates = (0..boxes.len())
        .filter(|&outer| {
            boxes[outer].outward
                && (0..boxes.len()).all(|inner| {
                    inner == outer
                        || (!boxes[inner].outward && contains(&boxes[outer], &boxes[inner]))
                })
        })
        .collect::<Vec<_>>();
    let [outer] = candidates.as_slice() else {
        return None;
    };
    let cavities = (0..boxes.len())
        .filter(|index| index != outer)
        .collect::<Vec<_>>();
    for (index, &left) in cavities.iter().enumerate() {
        for &right in &cavities[index + 1..] {
            if !disjoint(&boxes[left], &boxes[right]) {
                return None;
            }
        }
    }
    Some(std::iter::once(*outer).chain(cavities).collect())
}

fn contains(outer: &AxisBox, inner: &AxisBox) -> bool {
    (0..3).all(|axis| outer.min[axis] < inner.min[axis] && inner.max[axis] < outer.max[axis])
}

fn disjoint(left: &AxisBox, right: &AxisBox) -> bool {
    (0..3).any(|axis| left.max[axis] < right.min[axis] || right.max[axis] < left.min[axis])
}

fn classify_box(brep: &Brep, faces: &[usize]) -> Option<AxisBox> {
    if faces.len() != 6 {
        return None;
    }
    let mut vertices = BTreeSet::new();
    let mut edges = BTreeSet::new();
    for &face_index in faces {
        let face = &brep.faces()[face_index];
        let [face_loop] = face.loops() else {
            return None;
        };
        if face_loop.trims().len() != 4 {
            return None;
        }
        for trim in face_loop.trims() {
            vertices.extend(trim.vertices());
            edges.insert(trim.edge()?);
        }
    }
    if vertices.len() != 8 || edges.len() != 12 {
        return None;
    }
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for &index in &vertices {
        let point = brep.vertices()[index].point().to_array();
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    if (0..3).any(|axis| min[axis] >= max[axis]) {
        return None;
    }
    // Every combinatorial corner must occur exactly once. This also rejects
    // extra interior vertices disguised as an eight-vertex six-face shell.
    for bits in 0..8 {
        let expected = std::array::from_fn(|axis| {
            if bits & (1 << axis) == 0 {
                min[axis]
            } else {
                max[axis]
            }
        });
        if vertices
            .iter()
            .filter(|&&index| brep.vertices()[index].point().to_array() == expected)
            .count()
            != 1
        {
            return None;
        }
    }
    for &index in &edges {
        let [first, second] = brep.edges()[index].vertices();
        let first = brep.vertices()[first].point().to_array();
        let second = brep.vertices()[second].point().to_array();
        if (0..3).filter(|&axis| first[axis] != second[axis]).count() != 1 {
            return None;
        }
    }
    let mut seen_side = [false; 6];
    let mut outward = None;
    for &face_index in faces {
        let face = &brep.faces()[face_index];
        let controls = face.surface().control_points();
        let sides = (0..6)
            .filter(|&side| {
                let axis = side / 2;
                let value = if side % 2 == 0 { min[axis] } else { max[axis] };
                controls
                    .iter()
                    .all(|control| control.point().to_array()[axis] == value)
            })
            .collect::<Vec<_>>();
        let [side] = sides.as_slice() else {
            return None;
        };
        if std::mem::replace(&mut seen_side[*side], true) {
            return None;
        }
        let corners = face.loops()[0]
            .trims()
            .iter()
            .map(|trim| trim.vertices()[0])
            .collect::<BTreeSet<_>>();
        if corners.len() != 4
            || corners.iter().any(|&index| {
                let axis = side / 2;
                let value = if side % 2 == 0 { min[axis] } else { max[axis] };
                brep.vertices()[index].point().to_array()[axis] != value
            })
        {
            return None;
        }
        if *side == 0 {
            let u = face.surface().domain_u();
            let v = face.surface().domain_v();
            let normal = face
                .surface()
                .normal_at(
                    u.start() * 0.5 + u.end() * 0.5,
                    v.start() * 0.5 + v.end() * 0.5,
                )
                .ok()?
                .as_vector()
                .to_array();
            if normal[0].abs() < 0.999_999_999 || normal[1].abs() > 1e-9 || normal[2].abs() > 1e-9 {
                return None;
            }
            outward = Some(if face.is_reversed() {
                normal[0] > 0.0
            } else {
                normal[0] < 0.0
            });
        }
    }
    seen_side.iter().all(|seen| *seen).then_some(AxisBox {
        min,
        max,
        outward: outward?,
    })
}
