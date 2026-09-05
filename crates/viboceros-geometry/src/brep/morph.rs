use super::*;
use crate::PointMorph;

#[cfg(test)]
mod tests;

impl Brep {
    /// Fits a point-map image without changing shared topology or UV trims.
    ///
    /// Each component receives a quarter of the document's absolute fitting
    /// tolerance. The assembled B-rep must still pass boundary correspondence
    /// checks at the document tolerance; component tolerances are never enlarged
    /// past that limit to make an inconsistent result pass.
    /// Face reversals are retained: a black-box point map does not establish
    /// a globally orientation-preserving or injective transformation.
    pub fn morphed(
        &self,
        morph: &(impl PointMorph + ?Sized),
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let fitting = Tolerance::try_new(
            tolerance.absolute() * 0.25,
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let vertices = self
            .vertices
            .iter()
            .map(|vertex| {
                BrepVertex::try_new(morph.morph_point(vertex.point)?, tolerance.absolute())
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        // A shared edge is fitted once, not once for each incident face.
        let edges = self
            .edges
            .iter()
            .map(|edge| {
                BrepEdge::try_new(
                    edge.vertices,
                    morph.morph_nurbs_curve(&edge.curve, fitting)?,
                    tolerance.absolute(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let faces = self
            .faces
            .iter()
            .map(|face| {
                BrepFace::try_new(
                    morph.morph_nurbs_surface(&face.surface, fitting)?,
                    face.reversed,
                    face.loops.clone(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Self::try_new(vertices, edges, faces, tolerance)
    }
}
