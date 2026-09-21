//! Exact edge-table permutation without changing the spatial or trim geometry.
use super::*;

impl Brep {
    /// Returns the same B-rep with edge-table entries in the requested old-index
    /// order, remapping every trim reference. Requires a complete permutation.
    pub fn reordered_edges(
        &self,
        order: &[usize],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let invalid = || GeometryError::InvalidBrepTopology {
            context: "B-rep edge order must be a permutation",
        };
        if order.len() != self.edges.len() {
            return Err(invalid());
        }
        let mut inverse = vec![usize::MAX; order.len()];
        for (new, &old) in order.iter().enumerate() {
            let slot = inverse.get_mut(old).ok_or_else(invalid)?;
            if *slot != usize::MAX {
                return Err(invalid());
            }
            *slot = new;
        }
        let mut faces = self.faces.clone();
        for trim in faces
            .iter_mut()
            .flat_map(|f| &mut f.loops)
            .flat_map(|l| &mut l.trims)
        {
            if let Some(edge) = &mut trim.edge {
                *edge = inverse[*edge];
            }
        }
        Self::try_new(
            self.vertices.clone(),
            order.iter().map(|&i| self.edges[i].clone()).collect(),
            faces,
            tolerance,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edge_permutations_are_lossless_invertible_and_validated_before_indexing() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let brep = Brep::try_tube(frame, [3., 1.], 5., Tolerance::DEFAULT).unwrap();
        let order = (0..brep.edges.len()).rev().collect::<Vec<_>>();
        let reordered = brep.reordered_edges(&order, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            reordered
                .reordered_edges(&order, Tolerance::DEFAULT)
                .unwrap(),
            brep
        );
        assert_eq!(reordered.vertices, brep.vertices);
        assert_eq!(
            reordered.edges,
            brep.edges.iter().rev().cloned().collect::<Vec<_>>()
        );
        for invalid in [
            vec![],
            vec![0; brep.edges.len()],
            vec![usize::MAX; brep.edges.len()],
        ] {
            assert!(brep.reordered_edges(&invalid, Tolerance::DEFAULT).is_err());
        }
        assert!(reordered.is_solid());
    }
}
