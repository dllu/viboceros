//! Editable SubD walls, caps, and native component properties.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubDExtrudeSelection {
    AllFaces,
    Components {
        faces: Vec<usize>,
        edges: Vec<usize>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SubDExtrudeDirection {
    World(Vector3),
    VertexNormals,
    EdgeOutward,
}
impl SubD {
    /// Extrude faces or naked edges. Normal directions use float control-mesh
    /// normals, averaged across selected crease sectors without renormalizing.
    /// At branching junctions that prevent manifold walls, native command
    /// behavior leaves world-directed inputs unchanged and moves UVN controls
    /// without changing topology.
    pub fn try_extrude(
        &self,
        selection: &SubDExtrudeSelection,
        distance: f64,
        direction: SubDExtrudeDirection,
    ) -> Result<Self, GeometryError> {
        let (faces, edges, whole) = match selection {
            SubDExtrudeSelection::AllFaces => (&[][..], &[][..], true),
            SubDExtrudeSelection::Components { faces, edges } => {
                (faces.as_slice(), edges.as_slice(), false)
            }
        };
        use viboceros_opennurbs::SubDExtrudeDirection as Native;
        let direction = match direction {
            SubDExtrudeDirection::World(v) => Native::World(v.to_array()),
            SubDExtrudeDirection::VertexNormals => Native::VertexNormals,
            SubDExtrudeDirection::EdgeOutward => Native::EdgeOutward,
        };
        Self::from_native(viboceros_opennurbs::subd_extrude(
            self.archive(),
            faces,
            edges,
            whole,
            distance,
            direction,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subd_extrude_rejects_excessive_growth_and_invalid_parameters_without_changing_source() {
        let source = SubD::try_plane_grid(
            crate::Frame3::try_from_directions(
                Point3::try_new(0., 0., 0.).unwrap(),
                Vector3::try_new(1., 0., 0.).unwrap(),
                Vector3::try_new(0., 1., 0.).unwrap(),
                crate::Tolerance::DEFAULT,
            )
            .unwrap(),
            [4., 3.],
            [315, 314],
        )
        .unwrap();
        let archive = source.archive().to_vec();
        let direction = SubDExtrudeDirection::World(Vector3::try_new(0., 0., 1.).unwrap());
        assert!(
            source
                .try_extrude(&SubDExtrudeSelection::AllFaces, 1., direction)
                .is_err()
        );
        for distance in [0., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                source
                    .try_extrude(&SubDExtrudeSelection::AllFaces, distance, direction)
                    .is_err()
            );
        }
        assert!(
            source
                .try_extrude(
                    &SubDExtrudeSelection::Components {
                        faces: vec![usize::MAX],
                        edges: vec![]
                    },
                    1.,
                    direction
                )
                .is_err()
        );
        assert_eq!(source.archive(), archive);
    }
}
