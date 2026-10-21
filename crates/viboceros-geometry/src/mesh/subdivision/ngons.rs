//! Preserve each connected remainder of existing n-gons after selected faces split.
use super::*;
use std::collections::HashMap;
#[derive(Default)]
pub(super) struct Remainders {
    pub covered: Vec<bool>,
    pub groups: Vec<Vec<usize>>,
}
impl Remainders {
    pub fn new(mesh: &TriangleMesh, selected: &[bool]) -> Self {
        if mesh.ngons.is_empty() {
            return Self::default();
        }
        let mut parents = (0..mesh.faces.len()).collect::<Vec<_>>();
        let mut ranks = vec![0_u8; parents.len()];
        let mut covered = vec![false; mesh.faces.len()];
        let mut shared = HashMap::new();
        for (group, ngon) in mesh.ngons.iter().enumerate() {
            for &face in ngon.faces().iter().filter(|&&f| !selected[f as usize]) {
                let face = face as usize;
                covered[face] = true;
                let v = mesh.faces[face].indices();
                for i in 0..v.len() {
                    let a = v[i];
                    let b = v[(i + 1) % v.len()];
                    if let Some(&other) = shared.get(&(group, a.min(b), a.max(b))) {
                        union_faces(&mut parents, &mut ranks, face, other);
                    } else {
                        shared.insert((group, a.min(b), a.max(b)), face);
                    }
                }
            }
        }
        let mut groups = Vec::new();
        for ngon in &mesh.ngons {
            let mut components = BTreeMap::<usize, Vec<usize>>::new();
            for &face in ngon.faces().iter().filter(|&&f| !selected[f as usize]) {
                let face = face as usize;
                let root = index_root(&mut parents, face);
                components.entry(root).or_default().push(face);
            }
            groups.extend(components.into_values());
        }
        Self { covered, groups }
    }
}
