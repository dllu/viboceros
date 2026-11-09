//! Compact explicitly declared vertex identities without discovering contacts.
use super::*;

impl Brep {
    pub(crate) fn try_identify_vertices(
        mut self,
        pairs: &[(usize, usize)],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let invalid = || GeometryError::InvalidBrepTopology {
            context: "declared vertex identities disagree in model space",
        };
        let mut parents = (0..self.vertices.len()).collect::<Vec<_>>();
        for &(a, b) in pairs {
            if a >= parents.len() || b >= parents.len() {
                return Err(invalid());
            }
            let a = root(&mut parents, a);
            let b = root(&mut parents, b);
            parents[a.max(b)] = a.min(b);
        }
        let mut mapping = vec![0; parents.len()];
        let mut representatives = vec![usize::MAX; parents.len()];
        let mut vertices: Vec<BrepVertex> = Vec::new();
        for i in 0..parents.len() {
            let r = root(&mut parents, i);
            let point = self.vertices[i].point;
            let target = self.vertices[r].point;
            let distance = point.distance_to(target)?;
            // Check every member against the stored representative, rather
            // than accepting an arbitrarily long chain of nearby pairs.
            if !crate::curve_points_coincident(point, target) || distance > tolerance.absolute() {
                return Err(invalid());
            }
            let index = if representatives[r] == usize::MAX {
                representatives[r] = vertices.len();
                vertices.push(self.vertices[r]);
                vertices.len() - 1
            } else {
                representatives[r]
            };
            let uncertainty = self.vertices[i].tolerance + distance;
            require_nonnegative_finite(uncertainty, "identified vertex tolerance")?;
            vertices[index].tolerance = vertices[index].tolerance.max(uncertainty);
            mapping[i] = index;
        }
        if vertices.len() == self.vertices.len() {
            return Ok(self);
        }
        for e in &mut self.edges {
            e.vertices = e.vertices.map(|v| mapping[v]);
        }
        for f in &mut self.faces {
            for t in f.loops.iter_mut().flat_map(|l| &mut l.trims) {
                t.vertices = t.vertices.map(|v| mapping[v]);
            }
        }
        Self::try_new(vertices, self.edges, self.faces, tolerance)
    }
}

fn root(parents: &mut [usize], mut i: usize) -> usize {
    while parents[i] != i {
        parents[i] = parents[parents[i]];
        i = parents[i];
    }
    i
}

#[cfg(test)]
mod tests;
