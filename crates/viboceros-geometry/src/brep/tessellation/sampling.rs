use super::*;

#[derive(Clone, Copy)]
pub(super) struct Sampling<'a> {
    pub edge: usize,
    grids: Option<&'a [[usize; 2]]>,
}
impl<'a> Sampling<'a> {
    pub fn uniform(samples: usize) -> Self {
        Self {
            edge: samples,
            grids: None,
        }
    }
    pub fn per_face(grids: &'a [[usize; 2]]) -> Self {
        Self {
            edge: grids.iter().flatten().copied().max().unwrap_or(1),
            grids: Some(grids),
        }
    }
    pub fn grid(self, face: usize) -> [usize; 2] {
        self.grids.map_or([self.edge; 2], |g| g[face])
    }
    pub fn vertex_limit(self) -> usize {
        if self.grids.is_some() {
            1_048_576
        } else {
            u32::MAX as usize
        }
    }
    pub fn validate(self, faces: &[BrepFace]) -> Result<(), GeometryError> {
        if self.edge == 0
            || self
                .grids
                .is_some_and(|g| g.len() != faces.len() || g.iter().any(|r| r.contains(&0)))
        {
            return Err(GeometryError::InvalidTessellationResolution);
        }
        if self.grids.is_none() {
            return Ok(());
        }
        let mut total = 0usize;
        for (index, face) in faces.iter().enumerate() {
            let [u, v] = self.grid(index);
            let count = u
                .checked_add(1)
                .and_then(|u| v.checked_add(1).and_then(|v| u.checked_mul(v)))
                .and_then(|n| n.checked_mul(face.surface.spans_u().count()))
                .and_then(|n| n.checked_mul(face.surface.spans_v().count()));
            total = total
                .checked_add(count.ok_or(GeometryError::MeshSubdivisionLimit)?)
                .ok_or(GeometryError::MeshSubdivisionLimit)?;
            if total > 1_048_576 {
                return Err(GeometryError::MeshSubdivisionLimit);
            }
        }
        Ok(())
    }
}
