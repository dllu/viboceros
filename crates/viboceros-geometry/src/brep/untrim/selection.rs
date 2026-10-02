//! Exact selection of connected trimmed boundary runs, in original topology.
use super::*;

impl Brep {
    /// Identifies the original face-local trims selected by an Untrim pick.
    ///
    /// Inner picks select their entire hole; `all_similar` includes every inner
    /// loop of that face. Outer picks select a circularly connected run of
    /// trimmed edges, stopping at natural surface boundaries. `all_similar`
    /// selects the complete outer boundary, without selecting holes. A natural
    /// outer pick also selects the complete outer boundary.
    ///
    /// References are `(loop, trim)` pairs in original table order. This query
    /// never changes geometry. It classifies the complete rational UV control
    /// net against the native surface domains, independent of cached iso tags
    /// and the trimming curve's own scalar parameter interval.
    pub fn untrim_boundary_trims(
        &self,
        face: usize,
        boundary: usize,
        trim: usize,
        all_similar: bool,
    ) -> Result<Vec<(usize, usize)>, GeometryError> {
        let face = self
            .faces
            .get(face)
            .ok_or(GeometryError::BrepFaceIndexOutOfRange {
                face,
                face_count: self.faces.len(),
            })?;
        let boundary_record =
            face.loops
                .get(boundary)
                .ok_or(GeometryError::InvalidBrepTopology {
                    context: "Untrim boundary index outside face",
                })?;
        if trim >= boundary_record.trims.len() {
            return Err(GeometryError::InvalidBrepTopology {
                context: "Untrim trim index outside boundary",
            });
        }
        if boundary_record.loop_type == BrepLoopType::Inner {
            return Ok(face
                .loops
                .iter()
                .enumerate()
                .filter(|(index, ring)| {
                    ring.loop_type == BrepLoopType::Inner && (all_similar || *index == boundary)
                })
                .flat_map(|(index, ring)| (0..ring.trims.len()).map(move |trim| (index, trim)))
                .collect());
        }
        let trimmed = boundary_record
            .trims
            .iter()
            .map(|trim| {
                !matches!(
                    trim_iso::classify(&trim.curve, &face.surface),
                    SurfaceIso::South | SurfaceIso::East | SurfaceIso::North | SurfaceIso::West
                )
            })
            .collect::<Vec<_>>();
        if all_similar || !trimmed[trim] {
            return Ok((0..trimmed.len()).map(|trim| (boundary, trim)).collect());
        }
        let mut selected = vec![false; trimmed.len()];
        selected[trim] = true;
        let mut forward = (trim + 1) % trimmed.len();
        while trimmed[forward] && !selected[forward] {
            selected[forward] = true;
            forward = (forward + 1) % trimmed.len();
        }
        let mut backward = (trim + trimmed.len() - 1) % trimmed.len();
        while trimmed[backward] && !selected[backward] {
            selected[backward] = true;
            backward = (backward + trimmed.len() - 1) % trimmed.len();
        }
        Ok(selected
            .into_iter()
            .enumerate()
            .filter_map(|(trim, selected)| selected.then_some((boundary, trim)))
            .collect())
    }
}
