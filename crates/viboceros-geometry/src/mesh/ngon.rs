//! Validated mesh n-gon overlays over the triangle/quad face table.
use super::*;

impl TriangleMesh {
    pub fn ngons(&self) -> &[MeshNgon] {
        &self.ngons
    }

    /// Attaches logical polygons without changing the underlying mesh faces.
    /// An n-gon must cover an edge-connected face group with one oriented
    /// boundary cycle; no mesh face may belong to more than one n-gon.
    pub fn try_with_ngons(mut self, ngons: Vec<MeshNgon>) -> Result<Self, GeometryError> {
        let mut occupied_faces = BTreeSet::new();
        for (index, ngon) in ngons.iter().enumerate() {
            let invalid = || GeometryError::InvalidMeshNgon { ngon: index };
            if ngon.vertices.len() < 3
                || ngon
                    .vertices
                    .iter()
                    .any(|&vertex| vertex as usize >= self.vertices.len())
                || ngon.vertices.iter().copied().collect::<BTreeSet<_>>().len()
                    != ngon.vertices.len()
                || ngon.faces.is_empty()
            {
                return Err(invalid());
            }
            for &face in &ngon.faces {
                if face as usize >= self.faces.len() || !occupied_faces.insert(face) {
                    return Err(invalid());
                }
            }
            let Some(boundary) = self.ngon_boundary(&ngon.faces) else {
                return Err(invalid());
            };
            let Some(start) = boundary
                .iter()
                .position(|&vertex| vertex == ngon.vertices[0])
            else {
                return Err(invalid());
            };
            if boundary.len() != ngon.vertices.len()
                || !ngon
                    .vertices
                    .iter()
                    .copied()
                    .zip(
                        boundary
                            .iter()
                            .cycle()
                            .skip(start)
                            .take(boundary.len())
                            .copied(),
                    )
                    .all(|(expected, actual)| expected == actual)
            {
                return Err(invalid());
            }
        }
        self.ngons = ngons;
        Ok(self)
    }

    pub(crate) fn ngon_from_faces(&self, faces: Vec<u32>) -> Option<MeshNgon> {
        let vertices = self.ngon_boundary(&faces)?;
        Some(MeshNgon { vertices, faces })
    }

    pub(super) fn rebuild_ngons_for_same_faces(
        self,
        source: &[MeshNgon],
    ) -> Result<Self, GeometryError> {
        if source.is_empty() {
            return Ok(self);
        }
        let ngons = source
            .iter()
            .enumerate()
            .map(|(index, original)| {
                self.ngon_from_faces(original.faces.clone())
                    .ok_or(GeometryError::InvalidMeshNgon { ngon: index })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.try_with_ngons(ngons)
    }

    fn ngon_boundary(&self, faces: &[u32]) -> Option<Vec<u32>> {
        if faces.is_empty() || faces.iter().copied().collect::<BTreeSet<_>>().len() != faces.len() {
            return None;
        }
        let mut edge_uses = BTreeMap::<(u32, u32), Vec<(u32, u32, u32)>>::new();
        for &face_index in faces {
            let face = self.faces.get(face_index as usize)?;
            let indices = face.indices();
            for side in 0..indices.len() {
                let (from, to) = (indices[side], indices[(side + 1) % indices.len()]);
                edge_uses
                    .entry((from.min(to), from.max(to)))
                    .or_default()
                    .push((from, to, face_index));
            }
        }
        let mut next = BTreeMap::<u32, u32>::new();
        let mut incoming = BTreeSet::<u32>::new();
        let mut neighbors = BTreeMap::<u32, Vec<u32>>::new();
        for uses in edge_uses.values() {
            match uses.as_slice() {
                [(from, to, _)] => {
                    if next.insert(*from, *to).is_some() || !incoming.insert(*to) {
                        return None;
                    }
                }
                [
                    (first_from, first_to, first_face),
                    (second_from, second_to, second_face),
                ] if first_from == second_to && first_to == second_from => {
                    neighbors.entry(*first_face).or_default().push(*second_face);
                    neighbors.entry(*second_face).or_default().push(*first_face);
                }
                _ => return None,
            }
        }
        if next.len() < 3 || next.len() != incoming.len() {
            return None;
        }
        let start = *next.keys().next()?;
        let mut boundary = Vec::with_capacity(next.len());
        let mut current = start;
        for _ in 0..next.len() {
            boundary.push(current);
            current = *next.get(&current)?;
        }
        if current != start
            || boundary.iter().copied().collect::<BTreeSet<_>>().len() != boundary.len()
        {
            return None;
        }
        let mut visited = BTreeSet::from([faces[0]]);
        let mut pending = VecDeque::from([faces[0]]);
        while let Some(face) = pending.pop_front() {
            for &neighbor in neighbors.get(&face).into_iter().flatten() {
                if visited.insert(neighbor) {
                    pending.push_back(neighbor);
                }
            }
        }
        (visited.len() == faces.len()).then_some(boundary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_connected_face_region_and_oriented_boundary() {
        let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let mesh = TriangleMesh::try_new(
            vec![point(0., 0.), point(2., 0.), point(2., 2.), point(0., 2.)],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let ngon = MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0, 1]);
        let decorated = mesh.clone().try_with_ngons(vec![ngon.clone()]).unwrap();
        assert_eq!(decorated.ngons(), std::slice::from_ref(&ngon));
        assert_eq!(decorated.reversed().reversed(), decorated);
        assert!(
            decorated
                .reversed()
                .clone()
                .try_with_ngons(decorated.reversed().ngons().to_vec())
                .is_ok()
        );
        let translated = decorated
            .transformed(
                AffineTransform3::from_translation(crate::Vector3::try_new(5., 3., 1.).unwrap()),
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(translated.ngons(), decorated.ngons());
        let appended = TriangleMesh::try_append(&[&decorated, &decorated]).unwrap();
        assert_eq!(appended.ngons().len(), 2);
        assert_eq!(appended.ngons()[1].vertices(), &[4, 5, 6, 7]);
        assert_eq!(appended.ngons()[1].faces(), &[2, 3]);
        assert!(
            appended
                .clone()
                .try_with_ngons(appended.ngons().to_vec())
                .is_ok()
        );
        assert_eq!(mesh.wireframe_lines(Tolerance::DEFAULT).unwrap().len(), 5);
        assert_eq!(
            decorated.wireframe_lines(Tolerance::DEFAULT).unwrap().len(),
            5
        );
        assert_eq!(
            decorated
                .visible_wireframe_lines(Tolerance::DEFAULT)
                .unwrap()
                .len(),
            4
        );
        assert_eq!(decorated.topology_edge_points().len(), 5);
        assert!(
            mesh.clone()
                .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 3, 2, 1], vec![0, 1])])
                .is_err()
        );
        assert!(
            mesh.clone()
                .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2], vec![0, 1])])
                .is_err()
        );
        assert!(
            mesh.clone()
                .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0, 0])])
                .is_err()
        );
        assert!(mesh.try_with_ngons(vec![ngon.clone(), ngon]).is_err());
    }

    #[test]
    fn face_conversion_and_orientation_repair_retain_ngon_membership() {
        let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
        let quad = TriangleMesh::try_new_faces(
            vec![
                point(0., 0., 0.),
                point(2., 0., 0.),
                point(2., 2., 0.),
                point(0., 2., 0.),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0])])
        .unwrap();
        let (triangulated, count) = quad.triangulate_quads(Tolerance::DEFAULT).unwrap();
        assert_eq!(count, 1);
        assert_eq!(triangulated.ngons()[0].faces(), &[0, 1]);
        assert_eq!(
            triangulated
                .visible_wireframe_lines(Tolerance::DEFAULT)
                .unwrap()
                .len(),
            4
        );
        assert!(
            triangulated
                .clone()
                .try_with_ngons(triangulated.ngons().to_vec())
                .is_ok()
        );

        let mesh = TriangleMesh::try_new(
            vec![
                point(0., 0., 0.),
                point(2., 0., 0.),
                point(2., 2., 0.),
                point(0., 2., 0.),
                point(3., 1., 1.),
            ],
            vec![[0, 1, 2], [0, 2, 3], [1, 2, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0, 1])])
        .unwrap();
        let (unified, count) = mesh.unified_face_orientations().unwrap();
        assert_eq!(count, 1);
        assert_eq!(unified.ngons(), mesh.ngons());
        assert!(
            unified
                .clone()
                .try_with_ngons(unified.ngons().to_vec())
                .is_ok()
        );

        let with_unused = TriangleMesh::try_new(
            vec![
                point(9., 9., 9.),
                point(0., 0., 0.),
                point(2., 0., 0.),
                point(2., 2., 0.),
                point(0., 2., 0.),
            ],
            vec![[1, 2, 3], [1, 3, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_ngons(vec![MeshNgon::from_parts(vec![1, 2, 3, 4], vec![0, 1])])
        .unwrap();
        let (culled, removed) = with_unused.culled_unused_vertices();
        assert_eq!(removed, 1);
        assert_eq!(culled.ngons()[0].vertices(), &[0, 1, 2, 3]);
        assert!(
            culled
                .clone()
                .try_with_ngons(culled.ngons().to_vec())
                .is_ok()
        );
    }
}
