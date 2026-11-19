//! Edit a face chart against the existing model-space boundary.
use super::*;

impl Brep {
    /// Replace a face's support and selected directed p-curves while retaining
    /// the UV domains, orientation, vertices, and spatial edges. Trim edits
    /// name a zero-based loop and trim in that loop. A chart whose boundary
    /// fails ordinary B-rep validation leaves the original face intact.
    pub fn try_edit_face_chart(
        &mut self,
        index: usize,
        surface: NurbsSurface,
        trims: Vec<(usize, usize, NurbsCurve2)>,
        tolerance: Tolerance,
    ) -> Result<(), GeometryError> {
        let Some(source) = self.faces.get(index) else {
            return invalid("face chart edit index is out of range");
        };
        if surface.domain_u() != source.surface.domain_u()
            || surface.domain_v() != source.surface.domain_v()
        {
            return invalid("a face chart edit must retain its UV domains");
        }
        let mut loops = source.loops.clone();
        let mut seen = BTreeSet::new();
        for (loop_index, trim_index, curve) in trims {
            if !seen.insert((loop_index, trim_index)) {
                return invalid("face chart edit trim indices must be unique");
            }
            let Some(trim) = loops
                .get_mut(loop_index)
                .and_then(|boundary| boundary.trims.get_mut(trim_index))
            else {
                return invalid("face chart edit trim index is out of range");
            };
            if trim.curve != curve {
                trim.iso = trim_iso::classify(&curve, &surface);
                trim.curve = curve;
            }
        }
        let replacement = BrepFace::try_new(surface, source.reversed, loops)?;
        if replacement == *source {
            return Ok(());
        }
        let frame = replacement.local_parameter_frame()?;
        for boundary in &frame.face.loops {
            self.validate_loop(&frame.face, boundary, tolerance, true)?;
        }
        self.faces[index] = replacement;
        self.extrusion = None;
        Ok(())
    }
}
