//! Merge welded, consistently oriented triangle pairs into validated quads.

use super::*;

#[derive(Clone, Copy)]
struct TriangleEdgeUse {
    face: usize,
    start: u32,
    end: u32,
    opposite: u32,
}

#[derive(Clone, Copy)]
struct Candidate {
    first: usize,
    second: usize,
    quad: [u32; 4],
    diagonal_ratio: Real,
    normal_angle: Real,
}

impl TriangleMesh {
    /// Greedily merges pairs of raw-vertex-welded triangles. The face-normal
    /// angle and the ratio of the prospective quad's diagonal lengths must
    /// meet the supplied limits. Existing n-gon boundaries are preserved.
    /// Returns the unchanged mesh when no pair qualifies.
    pub fn quadrangulate_triangles(
        &self,
        max_planarity_degrees: Real,
        max_diagonal_ratio: Real,
        tolerance: Tolerance,
    ) -> Result<(Self, usize), GeometryError> {
        if !max_planarity_degrees.is_finite()
            || !(0.0..=180.0).contains(&max_planarity_degrees)
            || !max_diagonal_ratio.is_finite()
            || max_diagonal_ratio < 1.0
        {
            return Err(GeometryError::InvalidMeshQuadrangulationOptions);
        }
        if self.faces.iter().filter(|face| face.is_triangle()).count() < 2 {
            return Ok((self.clone(), 0));
        }

        let mut ngon_owner = vec![None; self.faces.len()];
        for (ngon_index, ngon) in self.ngons.iter().enumerate() {
            for &face in &ngon.faces {
                ngon_owner[face as usize] = Some(ngon_index);
            }
        }
        let mut edges = BTreeMap::<[u32; 2], Vec<TriangleEdgeUse>>::new();
        for (face, &polygon) in self.faces.iter().enumerate() {
            let MeshFace::Triangle([a, b, c]) = polygon else {
                continue;
            };
            for (start, end, opposite) in [(a, b, c), (b, c, a), (c, a, b)] {
                let key = [start.min(end), start.max(end)];
                edges.entry(key).or_default().push(TriangleEdgeUse {
                    face,
                    start,
                    end,
                    opposite,
                });
            }
        }
        let normals = self.polygon_face_normals()?;
        let max_angle = max_planarity_degrees.to_radians();
        let mut candidates = Vec::new();
        for uses in edges.values() {
            let [first, second] = uses.as_slice() else {
                continue;
            };
            if first.start != second.end
                || first.end != second.start
                || ngon_owner[first.face] != ngon_owner[second.face]
            {
                continue;
            }
            let [u, v, w, x] = [first.start, first.end, first.opposite, second.opposite];
            if [u, v, w, x].into_iter().collect::<BTreeSet<_>>().len() != 4 {
                continue;
            }
            let normal_angle = normals[first.face]
                .as_vector()
                .angle_to(normals[second.face].as_vector())?;
            if normal_angle > max_angle {
                continue;
            }
            let (Ok(shared), Ok(other)) = (
                self.vertices[u as usize].distance_to(self.vertices[v as usize]),
                self.vertices[w as usize].distance_to(self.vertices[x as usize]),
            ) else {
                continue;
            };
            let short = shared.min(other);
            let long = shared.max(other);
            let diagonal_ratio = long / short;
            if !diagonal_ratio.is_finite() || diagonal_ratio > max_diagonal_ratio {
                continue;
            }
            let quad = [u, x, v, w];
            // A valid diagonal split does not imply a convex boundary. Both
            // triangles across the other diagonal must retain orientation.
            let (Ok(left), Ok(right)) = (
                self.normal_for_face(MeshFace::Triangle([x, v, w])),
                self.normal_for_face(MeshFace::Triangle([x, w, u])),
            ) else {
                continue;
            };
            if left.as_vector().dot(normals[first.face].as_vector())? <= 0.0
                || right.as_vector().dot(normals[first.face].as_vector())? <= 0.0
            {
                continue;
            }
            candidates.push(Candidate {
                first: first.face.min(second.face),
                second: first.face.max(second.face),
                quad,
                diagonal_ratio,
                normal_angle,
            });
        }
        candidates.sort_by(|a, b| {
            a.diagonal_ratio
                .total_cmp(&b.diagonal_ratio)
                .then_with(|| a.normal_angle.total_cmp(&b.normal_angle))
                .then_with(|| (a.first, a.second).cmp(&(b.first, b.second)))
        });
        let mut replacement = vec![None; self.faces.len()];
        let mut removed = vec![false; self.faces.len()];
        let mut merged_pairs = Vec::new();
        let mut merged = 0;
        for candidate in candidates {
            if replacement[candidate.first].is_some()
                || replacement[candidate.second].is_some()
                || removed[candidate.first]
                || removed[candidate.second]
            {
                continue;
            }
            replacement[candidate.first] = Some(MeshFace::Quad(candidate.quad));
            removed[candidate.second] = true;
            merged_pairs.push((candidate.first, candidate.second));
            merged += 1;
        }
        if merged == 0 {
            return Ok((self.clone(), 0));
        }
        let mut faces = Vec::with_capacity(self.faces.len() - merged);
        let mut old_to_new = vec![0u32; self.faces.len()];
        for (index, &face) in self.faces.iter().enumerate() {
            if removed[index] {
                continue;
            }
            old_to_new[index] =
                u32::try_from(faces.len()).map_err(|_| GeometryError::TooManyMeshFaces)?;
            faces.push(replacement[index].unwrap_or(face));
        }
        for (first, second) in merged_pairs {
            old_to_new[second] = old_to_new[first];
        }
        let mut result = Self::try_new_faces(self.vertices.clone(), faces, tolerance)?
            .try_with_vertex_colors(self.vertex_colors.clone())?;
        if !self.ngons.is_empty() {
            let ngons = self
                .ngons
                .iter()
                .enumerate()
                .map(|(index, ngon)| {
                    let mut faces = ngon
                        .faces
                        .iter()
                        .map(|&face| old_to_new[face as usize])
                        .collect::<Vec<_>>();
                    let mut seen = BTreeSet::new();
                    faces.retain(|face| seen.insert(*face));
                    result
                        .ngon_from_faces(faces)
                        .ok_or(GeometryError::InvalidMeshNgon { ngon: index })
                })
                .collect::<Result<Vec<_>, _>>()?;
            result = result.try_with_ngons(ngons)?;
        }
        Ok((result, merged))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn merges_a_welded_square_and_retains_colors_and_ngon_boundary() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_vertex_colors(Some(vec![[1, 2, 3, 4]; 4]))
        .unwrap()
        .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0, 1])])
        .unwrap();
        let (converted, count) = mesh
            .quadrangulate_triangles(1.0, 2.0, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(converted.faces().len(), 1);
        assert!(matches!(converted.faces()[0], MeshFace::Quad(_)));
        assert_eq!(converted.vertex_colors(), mesh.vertex_colors());
        assert_eq!(converted.ngons().len(), 1);
        assert_eq!(converted.ngons()[0].faces(), &[0]);
        assert_eq!(converted.ngons()[0].vertices(), &[0, 1, 2, 3]);
        assert_eq!(
            converted
                .triangulate_quads(Tolerance::DEFAULT)
                .unwrap()
                .0
                .triangles()
                .len(),
            2
        );
    }

    #[test]
    fn does_not_merge_unwelded_or_oppositely_oriented_pairs() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(0.0, 1.0, 0.0),
        ];
        let unwelded = TriangleMesh::try_new(
            vertices.clone(),
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (unchanged, count) = unwelded
            .quadrangulate_triangles(180.0, 100.0, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(unchanged, unwelded);
        let conflict = TriangleMesh::try_new(
            vec![vertices[0], vertices[1], vertices[2], vertices[5]],
            vec![[0, 1, 2], [0, 3, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            conflict
                .quadrangulate_triangles(180.0, 100.0, Tolerance::DEFAULT)
                .unwrap()
                .1,
            0
        );
    }

    #[test]
    fn obeys_planarity_and_diagonal_ratio_limits() {
        let folded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 1.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            folded
                .quadrangulate_triangles(1.0, 10.0, Tolerance::DEFAULT)
                .unwrap()
                .1,
            0
        );
        let skewed = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(10.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            skewed
                .quadrangulate_triangles(1.0, 2.0, Tolerance::DEFAULT)
                .unwrap()
                .1,
            0
        );
        assert_eq!(
            skewed
                .quadrangulate_triangles(1.0, 8.0, Tolerance::DEFAULT)
                .unwrap()
                .1,
            1
        );
    }

    #[test]
    fn invalid_options_leave_the_source_unchanged() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        for (angle, ratio) in [(f64::NAN, 2.0), (-1.0, 2.0), (181.0, 2.0), (1.0, 0.5)] {
            assert_eq!(
                mesh.quadrangulate_triangles(angle, ratio, Tolerance::DEFAULT),
                Err(GeometryError::InvalidMeshQuadrangulationOptions)
            );
        }
    }
}
