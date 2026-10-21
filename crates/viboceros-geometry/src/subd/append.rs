//! Exact-location components and archival SubD face attachment.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
type Key = [u64; 3];
fn key(point: Point3) -> Key {
    point
        .to_array()
        .map(|x| if x == 0. { 0 } else { x.to_bits() })
}
fn error(message: &str) -> GeometryError {
    viboceros_opennurbs::SubDError(message.into()).into()
}

#[derive(Clone, Debug)]
pub struct SubDAppendResult {
    pub source: SubD,
    pub detached: Vec<SubD>,
    pub attached_faces: usize,
}
impl SubD {
    /// Adds all supplied faces to this record. The caller chooses whether
    /// disconnected components should remain in the same object; face commands
    /// normally use `try_append_face_components` instead.
    pub fn try_append_faces(&self, faces: &[Vec<Point3>]) -> Result<Self, GeometryError> {
        if faces.is_empty() {
            return Ok(self.clone());
        }
        check_faces(faces)?;
        let mut points = self.control_points().to_vec();
        let mut locations = BTreeMap::new();
        let mut ambiguous = BTreeSet::new();
        for (i, &point) in points.iter().enumerate() {
            if locations.insert(key(point), i as u32).is_some() {
                ambiguous.insert(key(point));
            }
        }
        let mut polygons = Vec::with_capacity(faces.len());
        for face in faces {
            let mut polygon = Vec::with_capacity(face.len());
            for &point in face {
                let k = key(point);
                if ambiguous.contains(&k) {
                    return Err(error("Ambiguous coincident source SubD controls"));
                }
                let index = if let Some(&i) = locations.get(&k) {
                    i
                } else {
                    if points.len() >= 100_000 {
                        return Err(error("SubD append control limit exceeded"));
                    }
                    let i = points.len() as u32;
                    points.push(point);
                    locations.insert(k, i);
                    i
                };
                polygon.push(index);
            }
            polygons.push(polygon);
        }
        let points = points.iter().map(|p| p.to_array()).collect::<Vec<_>>();
        Self::from_native(viboceros_opennurbs::subd_append(
            self.archive(),
            &points,
            &polygons,
        )?)
    }
    /// Retains this object's original native data. Components connected to one
    /// of its exact vertices are appended; detached components become
    /// separate SubDs. Exact shared vertices also join, including nonmanifold fans.
    pub fn try_append_face_components(
        &self,
        faces: &[Vec<Point3>],
    ) -> Result<SubDAppendResult, GeometryError> {
        let (attached, detached) = components(Some(self), faces)?;
        let source = self.try_append_faces(&attached)?;
        Ok(SubDAppendResult {
            source,
            attached_faces: attached.len(),
            detached: detached
                .into_iter()
                .map(build)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
    /// New objects follow exact shared vertices rather than coordinate proximity.
    pub fn try_from_face_components(faces: &[Vec<Point3>]) -> Result<Vec<Self>, GeometryError> {
        let (_, components) = components(None, faces)?;
        components.into_iter().map(build).collect()
    }
}
fn check_faces(faces: &[Vec<Point3>]) -> Result<(), GeometryError> {
    if faces.len() > 100_000
        || faces.iter().any(|f| !(3..=0xfff0).contains(&f.len()))
        || faces.iter().map(Vec::len).sum::<usize>() > 400_000
    {
        return Err(error("Invalid or excessive SubD face draft"));
    }
    Ok(())
}
type FaceSet = Vec<Vec<Point3>>;
fn components(
    source: Option<&SubD>,
    faces: &[Vec<Point3>],
) -> Result<(FaceSet, Vec<FaceSet>), GeometryError> {
    check_faces(faces)?;
    let source_points = source
        .map(|s| {
            s.control_points()
                .iter()
                .copied()
                .map(key)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let mut uses = BTreeMap::<Key, Vec<usize>>::new();
    for (i, face) in faces.iter().enumerate() {
        for &point in face {
            uses.entry(key(point)).or_default().push(i);
        }
    }
    let mut neighbors = vec![Vec::new(); faces.len()];
    let mut touches = vec![false; faces.len()];
    for (k, indices) in uses {
        // A star connects the incidence list in linear space; a clique would
        // grow quadratically when many faces meet at one vertex.
        let first = indices[0];
        for &next in &indices[1..] {
            if next != first {
                neighbors[first].push(next);
                neighbors[next].push(first);
            }
        }
        if source_points.contains(&k) {
            for i in indices {
                touches[i] = true;
            }
        }
    }
    let mut visited = vec![false; faces.len()];
    let mut attached = Vec::new();
    let mut detached = Vec::new();
    for start in 0..faces.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut queue = vec![start];
        let mut i = 0;
        let mut connects = false;
        while i < queue.len() {
            let here = queue[i];
            connects |= touches[here];
            for &next in &neighbors[here] {
                if !visited[next] {
                    visited[next] = true;
                    queue.push(next);
                }
            }
            i += 1;
        }
        queue.sort_unstable();
        if connects {
            attached.extend(queue)
        } else {
            detached.push(queue.into_iter().map(|i| faces[i].clone()).collect())
        }
    }
    attached.sort_unstable();
    Ok((
        attached.into_iter().map(|i| faces[i].clone()).collect(),
        detached,
    ))
}
fn build(faces: FaceSet) -> Result<SubD, GeometryError> {
    let first = &faces[0];
    let vertices = (0..first.len() as u32).collect();
    let subd = SubD::try_from_polygons(first.clone(), vec![vertices])?;
    subd.try_append_faces(&faces[1..])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn face(x: f64) -> Vec<Point3> {
        [[x, 0., 0.], [x + 4., 0., 0.], [x + 4., 3., 0.], [x, 3., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect()
    }
    #[test]
    fn exact_edge_attachment_preserves_archive_source_and_splits_gaps() {
        let source = SubD::try_from_polygons(face(0.), vec![vec![0, 1, 2, 3]]).unwrap();
        let before = source.archive().to_vec();
        let result = source
            .try_append_face_components(&[face(4.), face(12.)])
            .unwrap();
        assert_eq!(result.attached_faces, 1);
        assert_eq!(result.source.faces().len(), 2);
        assert_eq!(result.detached.len(), 1);
        assert_eq!(source.archive(), before);
        assert!(result.source.edge_tags().contains(&4));
        assert_eq!(
            source
                .try_append_face_components(&[face(4. + 2e-7)])
                .unwrap()
                .detached
                .len(),
            1
        );
        let mut reversed = face(4.);
        reversed.reverse();
        assert!(source.try_append_faces(&[reversed]).is_ok());
    }
    #[test]
    fn source_corner_retags_only_at_a_join_and_orientation_can_cross_a_later_bridge() {
        let source = SubD::try_from_polygons(face(0.), vec![vec![0, 1, 2, 3]])
            .unwrap()
            .try_with_vertex_tags(&[(0, 3), (1, 3)])
            .unwrap();
        let result = source.try_append_faces(&[face(4.)]).unwrap();
        assert_eq!(result.vertex_tags()[0], 3);
        assert_eq!(result.vertex_tags()[1], 2);
        let mut far = face(8.);
        far.reverse();
        let result = source.try_append_faces(&[far, face(4.)]).unwrap();
        assert_eq!(result.faces().len(), 3);
        assert_eq!(source.vertex_tags()[1], 3);
    }
}
