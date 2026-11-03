//! Editable native face subsets, independent of display tessellation.
use super::*;
use crate::mesh::union_find::{index_root, union_faces};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct SubDFaceExtraction {
    pub extracted: Vec<SubD>,
    /// First component retains the source object's identity at command level.
    pub remainder: Vec<SubD>,
}
impl SubD {
    /// Copy selected faces and remove unused topology. Corner ordinals refer to
    /// the source control table; missing vertices are ignored after deletion.
    pub fn try_retain_faces(
        &self,
        faces: &[usize],
        corners: &[usize],
    ) -> Result<Self, GeometryError> {
        Self::from_native(viboceros_opennurbs::subd_retain_faces(
            self.archive(),
            faces,
            corners,
        )?)
    }
    /// Extract connected selected regions and remaining regions in native face
    /// order. Selecting every face succeeds unchanged with no output.
    pub fn try_extract_faces(
        &self,
        faces: &[usize],
        add_corners: bool,
    ) -> Result<SubDFaceExtraction, GeometryError> {
        if faces.is_empty()
            || faces.len() > 100_000
            || faces.iter().any(|&f| f >= self.faces().len())
        {
            return Err(viboceros_opennurbs::SubDError(
                "Invalid SubD extraction face subset".into(),
            )
            .into());
        }
        let mut selected = vec![false; self.faces().len()];
        for &f in faces {
            selected[f] = true;
        }
        if selected.iter().all(|&s| s) {
            return Ok(SubDFaceExtraction {
                extracted: vec![],
                remainder: vec![],
            });
        }
        if self.faces().iter().map(Vec::len).sum::<usize>() > 400_000 {
            return Err(viboceros_opennurbs::SubDError(
                "SubD extraction corner budget exceeded".into(),
            )
            .into());
        }
        let mut edges = std::collections::BTreeSet::new();
        if self.edges().iter().any(|e| {
            let mut e = *e;
            e.sort_unstable();
            !edges.insert(e)
        }) {
            return Err(viboceros_opennurbs::SubDError(
                "Ambiguous parallel SubD source edges".into(),
            )
            .into());
        }
        let groups = self.face_regions(&selected);
        let mut counts = vec![0usize; self.control_points().len()];
        let mut remaining_edges = BTreeMap::<[u32; 2], usize>::new();
        if add_corners {
            for (f, &s) in selected.iter().enumerate() {
                let face = &self.faces()[f];
                if s {
                    for &v in face {
                        counts[v as usize] += 1;
                    }
                } else {
                    for i in 0..face.len() {
                        let mut e = [face[i], face[(i + 1) % face.len()]];
                        e.sort_unstable();
                        *remaining_edges.entry(e).or_default() += 1;
                    }
                }
            }
        }
        let mut boundary = vec![0usize; counts.len()];
        for (edge, n) in remaining_edges {
            if n == 1 {
                for v in edge {
                    boundary[v as usize] += 1;
                }
            }
        }
        let remainder_corners = counts
            .iter()
            .enumerate()
            .filter(|&(i, &n)| n == 1 && boundary[i] == 2)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        // Native archives are copied once per output; bound that work and the
        // total archive storage before admitting a pathological fragmented net.
        let records = self.control_points().len() + self.edges().len() + self.faces().len();
        if groups.len().saturating_mul(records) > 16_000_000
            || groups.len().saturating_mul(self.archive().len()) > 64 * 1024 * 1024
        {
            return Err(viboceros_opennurbs::SubDError(
                "SubD extraction staging budget exceeded".into(),
            )
            .into());
        }
        let mut result = SubDFaceExtraction {
            extracted: vec![],
            remainder: vec![],
        };
        for group in groups {
            let is_selected = selected[group[0]];
            let corners = if is_selected && add_corners {
                let mut local = BTreeMap::<usize, usize>::new();
                for &f in &group {
                    for &v in &self.faces()[f] {
                        *local.entry(v as usize).or_default() += 1;
                    }
                }
                local
                    .into_iter()
                    .filter(|&(_, n)| n == 1)
                    .map(|(v, _)| v)
                    .collect::<Vec<_>>()
            } else {
                remainder_corners.clone()
            };
            let target = if is_selected {
                &mut result.extracted
            } else {
                &mut result.remainder
            };
            target.push(self.try_retain_faces(&group, &corners)?);
        }
        Ok(result)
    }
    fn face_regions(&self, selected: &[bool]) -> Vec<Vec<usize>> {
        let mut parents = (0..self.faces().len()).collect::<Vec<_>>();
        let mut ranks = vec![0; parents.len()];
        let mut incident = BTreeMap::<[u32; 2], [Option<usize>; 2]>::new();
        for (i, face) in self.faces().iter().enumerate() {
            for j in 0..face.len() {
                let mut edge = [face[j], face[(j + 1) % face.len()]];
                edge.sort_unstable();
                let first =
                    &mut incident.entry(edge).or_insert([None; 2])[usize::from(selected[i])];
                if let Some(other) = *first {
                    union_faces(&mut parents, &mut ranks, other, i);
                } else {
                    *first = Some(i);
                }
            }
        }
        let mut groups = BTreeMap::<usize, Vec<usize>>::new();
        for i in 0..parents.len() {
            groups
                .entry(index_root(&mut parents, i))
                .or_default()
                .push(i);
        }
        let mut groups = groups.into_values().collect::<Vec<_>>();
        groups.sort_unstable_by_key(|g| g[0]);
        groups
    }
    /// Closest evaluated display facet and its control-face ordinal for typed
    /// face-location input. This is a bounded picking approximation.
    pub fn closest_display_face(
        &self,
        point: Point3,
    ) -> Result<Option<(usize, Point3)>, GeometryError> {
        let display = self.display_mesh_with_normals()?;
        let mut best = None;
        for (i, &face) in display.face_sources.iter().enumerate() {
            let p = display.mesh.closest_point_on_face(i, point)?;
            let distance = p.distance_to(point)?;
            if best.is_none_or(|(_, _, d)| distance < d) {
                best = Some((face, p, distance));
            }
        }
        Ok(best.map(|(f, p, _)| (f, p)))
    }
}

#[cfg(test)]
mod tests;
