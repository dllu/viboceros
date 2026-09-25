//! Find single-boundary planar n-gons without changing mesh faces or vertices.
//! Raw-edge adjacency and first-face plane checks follow the public
//! ON_MeshNgon::FindPlanarNgons algorithm in third_party/opennurbs.

use super::*;

impl TriangleMesh {
    /// Adds overlays for connected, compatibly oriented face groups sharing
    /// raw welded edges. Each group must fit its first face's plane within the
    /// supplied 3D distance tolerance and have one simple outer boundary.
    /// Existing n-gons and their faces are left intact.
    pub fn add_planar_ngons(&self, planar_tolerance: Real) -> Result<(Self, usize), GeometryError> {
        if !planar_tolerance.is_finite() || planar_tolerance < 0.0 {
            return Err(GeometryError::InvalidMeshNgonPlanarityTolerance);
        }
        let mut occupied = vec![false; self.faces.len()];
        for ngon in &self.ngons {
            for &face in ngon.faces() {
                occupied[face as usize] = true;
            }
        }
        let normals = self.polygon_face_normals()?;
        let mut edges = BTreeMap::<(u32, u32), Vec<(usize, bool)>>::new();
        for (face_index, face) in self.faces.iter().enumerate() {
            if occupied[face_index] {
                continue;
            }
            let indices = face.indices();
            for (&from, &to) in indices
                .iter()
                .zip(indices.iter().cycle().skip(1))
                .take(indices.len())
            {
                edges
                    .entry((from.min(to), from.max(to)))
                    .or_default()
                    .push((face_index, from < to));
            }
        }
        let mut neighbors = vec![Vec::<usize>::new(); self.faces.len()];
        for uses in edges.values() {
            if let [(first, first_forward), (second, second_forward)] = uses.as_slice()
                && first_forward != second_forward
                && first != second
            {
                neighbors[*first].push(*second);
                neighbors[*second].push(*first);
            }
        }
        let mut visited = occupied;
        let mut additions = Vec::new();
        for seed in 0..self.faces.len() {
            if visited[seed] {
                continue;
            }
            let normal = normals[seed].as_vector();
            let origin = self.vertices[self.faces[seed].indices()[0] as usize];
            if !self.face_fits_ngon_plane(seed, normal, origin, planar_tolerance) {
                continue;
            }
            let mut queue = VecDeque::from([seed]);
            let mut faces = Vec::new();
            visited[seed] = true;
            while let Some(face) = queue.pop_front() {
                faces.push(face as u32);
                for &other in &neighbors[face] {
                    if visited[other]
                        || normals[other].as_vector().dot(normal)? <= 0.0
                        || !self.face_fits_ngon_plane(other, normal, origin, planar_tolerance)
                    {
                        continue;
                    }
                    visited[other] = true;
                    queue.push_back(other);
                }
            }
            if faces.len() >= 2 {
                faces.sort_unstable();
                if let Some(ngon) = self.ngon_from_faces(faces) {
                    additions.push(ngon);
                }
            }
        }
        if additions.is_empty() {
            return Ok((self.clone(), 0));
        }
        let count = additions.len();
        let mut ngons = self.ngons.clone();
        ngons.extend(additions);
        self.clone().try_with_ngons(ngons).map(|mesh| (mesh, count))
    }

    fn face_fits_ngon_plane(
        &self,
        face: usize,
        normal: crate::Vector3,
        origin: Point3,
        tolerance: Real,
    ) -> bool {
        self.faces[face].indices().iter().all(|&index| {
            let distance = normal.dot_point_difference(self.vertices[index as usize], origin);
            distance.is_finite() && distance.abs() <= tolerance
        })
    }

    /// Removes all logical n-gon overlays, retaining the stored triangles,
    /// quadrilaterals, vertex order, and optional vertex colors.
    pub fn without_ngons(&self) -> Self {
        let mut result = self.clone();
        result.ngons.clear();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn square(z: Real) -> TriangleMesh {
        TriangleMesh::try_new(
            vec![
                point(0., 0., 0.),
                point(2., 0., 0.),
                point(2., 2., z),
                point(0., 2., 0.),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn adds_and_removes_planar_overlay_without_changing_stored_faces() {
        let source = square(0.)
            .try_with_vertex_colors(Some(vec![[8, 16, 32, 0]; 4]))
            .unwrap();
        let (decorated, count) = source.add_planar_ngons(0.).unwrap();
        assert_eq!(count, 1);
        assert_eq!(decorated.faces(), source.faces());
        assert_eq!(decorated.vertices(), source.vertices());
        assert_eq!(decorated.vertex_colors(), source.vertex_colors());
        assert_eq!(decorated.ngons()[0].faces(), &[0, 1]);
        assert_eq!(
            decorated
                .visible_wireframe_lines(Tolerance::DEFAULT)
                .unwrap()
                .len(),
            4
        );
        let (unchanged, count) = decorated.add_planar_ngons(0.).unwrap();
        assert_eq!(count, 0);
        assert_eq!(unchanged, decorated);
        let removed = decorated.without_ngons();
        assert_eq!(removed, source);
        assert_eq!(
            removed
                .visible_wireframe_lines(Tolerance::DEFAULT)
                .unwrap()
                .len(),
            5
        );
    }

    #[test]
    fn raw_unwelded_seam_and_nonplanar_faces_do_not_form_ngons() {
        let unwelded = TriangleMesh::try_new(
            vec![
                point(0., 0., 0.),
                point(1., 0., 0.),
                point(0., 1., 0.),
                point(1., 0., 0.),
                point(1., 1., 0.),
                point(0., 1., 0.),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(unwelded.add_planar_ngons(0.).unwrap().1, 0);
        assert_eq!(square(0.2).add_planar_ngons(0.01).unwrap().1, 0);
        assert_eq!(square(0.2).add_planar_ngons(0.2).unwrap().1, 1);
    }

    #[test]
    fn invalid_tolerance_is_rejected() {
        let mesh = square(0.);
        for tolerance in [Real::NAN, Real::INFINITY, -1.] {
            assert_eq!(
                mesh.add_planar_ngons(tolerance),
                Err(GeometryError::InvalidMeshNgonPlanarityTolerance)
            );
        }
    }

    #[test]
    fn existing_ngons_are_preserved_and_single_boundary_groups_are_added() {
        let first = square(0.).add_planar_ngons(0.).unwrap().0;
        let joined = TriangleMesh::try_append(&[&first, &square(0.)]).unwrap();
        let (result, added) = joined.add_planar_ngons(0.).unwrap();
        assert_eq!(added, 1);
        assert_eq!(result.ngons().len(), 2);
        assert_eq!(result.ngons()[0], joined.ngons()[0]);
        assert_eq!(result.ngons()[1].faces(), &[2, 3]);
    }

    #[test]
    fn planar_ring_with_inner_boundary_is_skipped() {
        let vertices = (0..4)
            .flat_map(|y| (0..4).map(move |x| point(x as Real, y as Real, 0.)))
            .collect();
        let mut faces = Vec::new();
        for y in 0..3 {
            for x in 0..3 {
                if x == 1 && y == 1 {
                    continue;
                }
                let a = (y * 4 + x) as u32;
                faces.push(MeshFace::Quad([a, a + 1, a + 5, a + 4]));
            }
        }
        let ring = TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap();
        let (result, added) = ring.add_planar_ngons(0.).unwrap();
        assert_eq!(added, 0);
        assert!(result.ngons().is_empty());
    }
}
