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
    removed_quality: Real,
}

impl TriangleMesh {
    /// Convert triangle pairs with public OpenNURBS selection and angle policy.
    /// Rectangularity is the maximum ratio between a quad's diagonal lengths.
    /// Native conversion determines face slots and n-gon changes; existing
    /// vertices and their stored channels remain unchanged.
    pub fn quadrangulate_triangles(
        &self,
        max_planarity_degrees: Real,
        max_diagonal_ratio: Real,
        _tolerance: Tolerance,
    ) -> Result<(Self, usize), GeometryError> {
        if !max_planarity_degrees.is_finite()
            || !(0.0..=180.0).contains(&max_planarity_degrees)
            || !max_diagonal_ratio.is_finite()
            || max_diagonal_ratio < 1.0
        {
            return Err(GeometryError::InvalidMeshQuadrangulationOptions);
        }
        self.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
        let (mut data, changed) = viboceros_opennurbs::quadrangulate_mesh(
            &self.open_nurbs_data(),
            max_planarity_degrees.to_radians(),
            1. / max_diagonal_ratio,
        )?;
        if changed == 0 {
            return Ok((self.clone(), 0));
        }
        if data.vertices.len() != self.vertices.len()
            || data
                .vertices
                .iter()
                .zip(&self.vertices)
                .any(|(native, point)| *native != point.to_array())
        {
            return Err(viboceros_opennurbs::MeshError(
                "Quad conversion changed vertex storage".into(),
            )
            .into());
        }
        let ngons = std::mem::take(&mut data.ngons)
            .into_iter()
            .map(|g| MeshNgon::from_parts(g.vertices, g.faces))
            .collect();
        let mut result = Self::from_open_nurbs_data(data)?.retain_boundary_matched_ngons(ngons);
        result.vertex_colors = self.vertex_colors.clone();
        result.vertex_channels = self.vertex_channels.clone();
        result.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
        Ok((result, changed))
    }

    /// Prefer more coplanar pairs, then retain better standalone triangles.
    /// New quads must remain valid when a source boundary changes their origin.
    pub(crate) fn quadrangulate_inserted_region(
        &self,
        tolerance: Tolerance,
    ) -> Result<(Self, usize), GeometryError> {
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
        let triangle_quality = (0..self.face_count())
            .map(|i| self.face_aspect_ratio(i).map(|ratio| 1. / ratio))
            .collect::<Result<Vec<_>, _>>()?;
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
            let (Ok(shared), Ok(other)) = (
                self.vertices[u as usize].distance_to(self.vertices[v as usize]),
                self.vertices[w as usize].distance_to(self.vertices[x as usize]),
            ) else {
                continue;
            };
            let short = shared.min(other);
            let long = shared.max(other);
            let diagonal_ratio = long / short;
            if !diagonal_ratio.is_finite() {
                continue;
            }
            let quad = [u, x, v, w];
            {
                let Some(projected) =
                    project_mesh_hole_boundary(&quad.map(|i| self.vertices[i as usize]))?
                else {
                    continue;
                };
                if (0..4).any(|i| {
                    mesh_hole_cross(projected[i], projected[(i + 1) % 4], projected[(i + 2) % 4])
                        <= 64. * Real::EPSILON
                }) {
                    continue;
                }
            }
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
                removed_quality: triangle_quality[first.face] + triangle_quality[second.face],
            });
        }
        let planarity = |c: &Candidate| {
            if c.normal_angle > 64. * Real::EPSILON {
                c.normal_angle
            } else {
                0.
            }
        };
        candidates.sort_by(|a, b| {
            planarity(a)
                .total_cmp(&planarity(b))
                .then_with(|| a.removed_quality.total_cmp(&b.removed_quality))
                .then_with(|| a.diagonal_ratio.total_cmp(&b.diagonal_ratio))
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
        result.vertex_channels = self.vertex_channels.clone();
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
    fn insertion_pairing_keeps_collinear_corners_as_separate_triangles() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0., 0.818517629189515, 0.10913568389193534),
                point(0., 0., 0.),
                point(4., 0., 0.),
                point(0., 2.2945783401646214, 0.30594377868861616),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::MESH_VALIDATION,
        )
        .unwrap();
        let (result, merged) = mesh
            .quadrangulate_inserted_region(Tolerance::MESH_VALIDATION)
            .unwrap();
        assert_eq!(merged, 0);
        assert_eq!(result, mesh);
    }

    #[test]
    fn merges_a_welded_square_and_retains_colors_with_native_ngon_cleanup() {
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
        assert!(converted.ngons().is_empty());
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
