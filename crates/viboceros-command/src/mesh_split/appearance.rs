//! Reconstruct stored faces and logical boundaries from original face ownership.
use super::*;
use viboceros_geometry::{MeshFace, MeshProjectionIndex};

pub(super) struct Appearance<'a> {
    source: &'a TriangleMesh,
    index: MeshProjectionIndex<'a>,
    original_ngons: Vec<Option<usize>>,
}
impl<'a> Appearance<'a> {
    pub(super) fn new(source: &'a TriangleMesh) -> Result<Self, CommandError> {
        let mut original_ngons = vec![None; source.faces().len()];
        for (i, ngon) in source.ngons().iter().enumerate() {
            for &face in ngon.faces() {
                original_ngons[face as usize] = Some(i);
            }
        }
        Ok(Self {
            source,
            index: MeshProjectionIndex::new(source)?,
            original_ngons,
        })
    }
    pub(super) fn restore(
        &self,
        part: TriangleMesh,
        create_ngons: bool,
    ) -> Result<TriangleMesh, CommandError> {
        let source = self.source;
        let index = &self.index;
        if &part == source {
            return Ok(part);
        }
        let mut groups = BTreeMap::<usize, Vec<usize>>::new();
        let mut parents = Vec::with_capacity(part.faces().len());
        for (i, face) in part.faces().iter().enumerate() {
            let p = face.indices();
            let a = part.vertices()[p[0] as usize];
            let b = part.vertices()[p[1] as usize];
            let c = part.vertices()[p[2] as usize];
            let center = Point3::try_from(std::array::from_fn(|axis| {
                a.to_array()[axis] / 3. + b.to_array()[axis] / 3. + c.to_array()[axis] / 3.
            }))?;
            let location = index
                .closest_location(center)?
                .ok_or(CommandError::Usage("mesh split source face"))?;
            let parent = index
                .triangle_face(location.triangle_index)
                .ok_or(CommandError::Usage("mesh split source face"))?;
            parents.push(parent);
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
        let original_ngons = &self.original_ngons;
        let mut logical = BTreeMap::<(bool, usize), Vec<u32>>::new();
        for (i, &parent) in output_parents.iter().enumerate() {
            let key = original_ngons[parent].map_or((false, parent), |n| (true, n));
            logical.entry(key).or_default().push(i as u32);
        }
        let ngons = logical
            .into_iter()
            .filter_map(|((existing, _), faces)| {
                ((existing || create_ngons) && faces.len() > 1)
                    .then(|| result.ngon_from_faces(faces))
                    .flatten()
            })
            .collect();
        Ok(result.try_with_ngons(ngons)?)
    }
}
