//! Face, channel and raw-sharing equivalence under vertex renumbering.
use viboceros_geometry::TriangleMesh;

fn same_vertex(a: &TriangleMesh, i: usize, b: &TriangleMesh, j: usize) -> bool {
    let near = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(a, b)| (a - b).abs() <= 1e-6);
    near(&a.vertices()[i].to_array(), &b.vertices()[j].to_array())
        && match (a.texture_coordinates(), b.texture_coordinates()) {
            (Some(a), Some(b)) => near(&a[i], &b[j]),
            (None, None) => true,
            _ => false,
        }
        && match (a.vertex_normals(), b.vertex_normals()) {
            (Some(a), Some(b)) => near(&a[i].to_array(), &b[j].to_array()),
            (None, None) => true,
            _ => false,
        }
        && match (a.vertex_colors(), b.vertex_colors()) {
            (Some(a), Some(b)) => a[i] == b[j],
            (None, None) => true,
            _ => false,
        }
}

pub fn equivalent(a: &TriangleMesh, b: &TriangleMesh) -> bool {
    if a.vertices().len() != b.vertices().len()
        || a.face_count() != b.face_count()
        || !a.ngons().is_empty()
        || !b.ngons().is_empty()
    {
        return false;
    }
    let mut forward = vec![None; a.vertices().len()];
    let mut reverse = vec![None; b.vertices().len()];
    for (left, right) in a.faces().iter().zip(b.faces()) {
        let left = left.indices();
        let right = right.indices();
        if left.len() != right.len() {
            return false;
        }
        let shift = (0..right.len()).find(|&shift| {
            left.iter().enumerate().all(|(i, &raw)| {
                let target = right[(i + shift) % right.len()] as usize;
                same_vertex(a, raw as usize, b, target)
                    && forward[raw as usize].is_none_or(|mapped| mapped == target)
                    && reverse[target].is_none_or(|mapped| mapped == raw as usize)
            })
        });
        let Some(shift) = shift else {
            return false;
        };
        for (i, &raw) in left.iter().enumerate() {
            let target = right[(i + shift) % right.len()] as usize;
            forward[raw as usize] = Some(target);
            reverse[target] = Some(raw as usize);
        }
    }
    for (i, mapped) in forward.iter().enumerate() {
        if mapped.is_some() {
            continue;
        }
        let Some(j) =
            (0..b.vertices().len()).find(|&j| reverse[j].is_none() && same_vertex(a, i, b, j))
        else {
            return false;
        };
        reverse[j] = Some(i);
    }
    true
}
