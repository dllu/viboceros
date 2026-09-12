use super::*;

impl Brep {
    /// Combines owned B-reps into one topology container without joining edges.
    ///
    /// Input order, surfaces, curves, UV trims and face sense are preserved.
    /// Coincident vertices remain distinct. This is not a Boolean union or a
    /// solid-containment classifier: callers must retain the intended orientation
    /// of outer and cavity shells. The complete result is validated at `tolerance`.
    pub fn try_combine(parts: Vec<Self>, tolerance: Tolerance) -> Result<Self, GeometryError> {
        let overflow = || GeometryError::InvalidBrepTopology {
            context: "combined B-rep topology exceeds allocation limits",
        };
        let mut counts = [0_usize; 3];
        for part in &parts {
            for (count, size) in
                counts
                    .iter_mut()
                    .zip([part.vertices.len(), part.edges.len(), part.faces.len()])
            {
                *count = count.checked_add(size).ok_or_else(overflow)?;
            }
        }
        let mut vertices = Vec::new();
        let mut edges = Vec::new();
        let mut faces = Vec::new();
        vertices
            .try_reserve_exact(counts[0])
            .map_err(|_| overflow())?;
        edges.try_reserve_exact(counts[1]).map_err(|_| overflow())?;
        faces.try_reserve_exact(counts[2]).map_err(|_| overflow())?;
        for mut part in parts {
            let vertex_offset = vertices.len();
            let edge_offset = edges.len();
            for edge in &mut part.edges {
                for vertex in &mut edge.vertices {
                    *vertex = vertex.checked_add(vertex_offset).ok_or_else(overflow)?;
                }
            }
            for face in &mut part.faces {
                for face_loop in &mut face.loops {
                    for trim in &mut face_loop.trims {
                        for vertex in &mut trim.vertices {
                            *vertex = vertex.checked_add(vertex_offset).ok_or_else(overflow)?;
                        }
                        if let Some(edge) = &mut trim.edge {
                            *edge = edge.checked_add(edge_offset).ok_or_else(overflow)?;
                        }
                    }
                }
            }
            vertices.append(&mut part.vertices);
            edges.append(&mut part.edges);
            faces.append(&mut part.faces);
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(radius: Real) -> Brep {
        Brep::try_box(
            Frame3::try_from_normal(
                Point3::try_new(0., 0., 0.).unwrap(),
                Vector3::try_new(0., 0., 1.).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
            [[-radius, radius]; 3],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn combined_cavity_retains_signed_volume_and_independent_topology() {
        let outer = cube(5.);
        let mut inner = cube(1.);
        for face in &mut inner.faces {
            face.reversed = !face.reversed;
        }
        let original = [outer.clone(), inner.clone()];
        let combined = Brep::try_combine(vec![outer, inner], Tolerance::DEFAULT).unwrap();
        assert_eq!(
            (
                combined.vertices.len(),
                combined.edges.len(),
                combined.faces.len()
            ),
            (16, 24, 12)
        );
        assert!((combined.signed_volume(Tolerance::DEFAULT).unwrap() - 992.).abs() < 1e-10);
        assert!((combined.area(Tolerance::DEFAULT).unwrap() - 624.).abs() < 1e-10);
        for (part_index, part) in original.iter().enumerate() {
            let vo = part_index * 8;
            let eo = part_index * 12;
            assert_eq!(&combined.vertices[vo..vo + 8], part.vertices());
            for (actual, old) in combined.edges[eo..eo + 12].iter().zip(part.edges()) {
                assert_eq!(actual.vertices, old.vertices.map(|v| v + vo));
                assert_eq!(actual.curve, old.curve);
                assert_eq!(actual.tolerance, old.tolerance);
            }
            for (actual, old) in combined.faces[part_index * 6..part_index * 6 + 6]
                .iter()
                .zip(part.faces())
            {
                assert_eq!(actual.surface, old.surface);
                assert_eq!(actual.reversed, old.reversed);
                for (actual_loop, old_loop) in actual.loops.iter().zip(&old.loops) {
                    assert_eq!(actual_loop.loop_type, old_loop.loop_type);
                    for (actual_trim, old_trim) in actual_loop.trims.iter().zip(&old_loop.trims) {
                        let mut expected = old_trim.clone();
                        expected.vertices = expected.vertices.map(|v| v + vo);
                        expected.edge = expected.edge.map(|e| e + eo);
                        assert_eq!(*actual_trim, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn combination_does_not_weld_coincident_parts_and_rejects_empty_input() {
        let original = cube(1.);
        assert_eq!(
            Brep::try_combine(vec![original.clone()], Tolerance::DEFAULT).unwrap(),
            original
        );
        let combined =
            Brep::try_combine(vec![original.clone(), original], Tolerance::DEFAULT).unwrap();
        assert_eq!(combined.vertices.len(), 16);
        assert_eq!(combined.edges.len(), 24);
        assert!((combined.signed_volume(Tolerance::DEFAULT).unwrap() - 16.).abs() < 1e-12);
        assert!(Brep::try_combine(vec![], Tolerance::DEFAULT).is_err());
    }
}
