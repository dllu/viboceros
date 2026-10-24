//! Reconstruct stored faces and logical boundaries from original face ownership.
use super::*;
use viboceros_geometry::{MeshChannelSource, MeshFace};

pub(crate) struct Appearance<'a> {
    source: &'a TriangleMesh,
    channels: MeshChannelSource<'a>,
    original_ngons: Vec<Option<usize>>,
}
impl<'a> Appearance<'a> {
    pub(crate) fn new(source: &'a TriangleMesh) -> Result<Self, CommandError> {
        let mut original_ngons = vec![None; source.faces().len()];
        for (i, ngon) in source.ngons().iter().enumerate() {
            for &face in ngon.faces() {
                original_ngons[face as usize] = Some(i);
            }
        }
        Ok(Self {
            source,
            channels: MeshChannelSource::new(source)?,
            original_ngons,
        })
    }
    pub(crate) fn restore(
        &self,
        part: TriangleMesh,
        parents: Vec<usize>,
        create_ngons: bool,
    ) -> Result<TriangleMesh, CommandError> {
        let source = self.source;
        if &part == source {
            return Ok(if create_ngons {
                part
            } else {
                part.try_with_ngons(vec![])?
            });
        }
        let mut groups = BTreeMap::<usize, Vec<usize>>::new();
        for (i, &parent) in parents.iter().enumerate() {
            groups.entry(parent).or_default().push(i);
        }
        let mut replacement = BTreeMap::new();
        let mut omitted = BTreeSet::new();
        for (&parent, faces) in &groups {
            let original = source.faces()[parent];
            if original.is_quad() && faces.len() == 2 {
                let vertices = faces
                    .iter()
                    .flat_map(|&i| part.faces()[i].indices().iter().copied())
                    .collect::<BTreeSet<_>>();
                if vertices.len() != 4 {
                    continue;
                }
                let mapped = original
                    .indices()
                    .iter()
                    .map(|&v| {
                        vertices
                            .iter()
                            .copied()
                            .find(|&w| source.vertices()[v as usize] == part.vertices()[w as usize])
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(mapped) = mapped {
                    replacement.insert(faces[0], MeshFace::Quad(mapped.try_into().unwrap()));
                    omitted.insert(faces[1]);
                }
            }
        }
        let mut faces = Vec::new();
        let mut output_parents = Vec::new();
        for (i, &face) in part.faces().iter().enumerate() {
            if omitted.contains(&i) {
                continue;
            }
            faces.push(replacement.get(&i).copied().unwrap_or(face));
            output_parents.push(parents[i]);
        }
        let result = TriangleMesh::try_from_face_records(part.vertices().to_vec(), faces)?;
        let result = self.channels.transfer(result, &output_parents)?;
        let original_ngons = &self.original_ngons;
        let mut logical = BTreeMap::<(bool, usize), Vec<u32>>::new();
        for (i, &parent) in output_parents.iter().enumerate() {
            let key = original_ngons[parent].map_or((false, parent), |n| (true, n));
            logical.entry(key).or_default().push(i as u32);
        }
        let ngons = logical
            .into_values()
            .filter_map(|faces| {
                (create_ngons && faces.len() > 1)
                    .then(|| result.ngon_from_faces(faces))
                    .flatten()
            })
            .collect();
        Ok(result.try_with_ngons(ngons)?)
    }
}
