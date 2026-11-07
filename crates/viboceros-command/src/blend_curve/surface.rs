//! Cross-edge constraints use the selected face rather than boundary curvature.
use super::*;
use crate::surface_edge_reference::Edge;

#[derive(Clone, Debug)]
pub(super) struct SurfaceGuide {
    reference: Edge,
    planar: bool,
    pub angle: Real,
}
impl SurfaceGuide {
    pub fn capture(doc: &Document, id: ObjectId, edge: usize) -> Result<Self, CommandError> {
        let reference = Edge::capture(doc, id, edge, None).map_err(|error| match error {
            CommandError::Usage("Specify Face=index to choose the surface on a shared edge") => {
                CommandError::Usage("BlendCrv face selection on shared edges is not supported yet")
            }
            error => error,
        })?;
        let planar = reference
            .target
            .surface
            .plane(Tolerance::NUMERICAL_VALIDATION)?
            .is_some();
        Ok(Self {
            reference,
            planar,
            angle: 0.,
        })
    }
    pub fn direction(&self, guide: &Guide, i: usize) -> Result<UnitVector3, CommandError> {
        let normal = self.reference.target.source_normal(guide.point()?, true)?;
        self.direction_with_normal(guide, i, normal)
    }
    fn direction_with_normal(
        &self,
        guide: &Guide,
        i: usize,
        normal: Vector3,
    ) -> Result<UnitVector3, CommandError> {
        let tangent = guide
            .curve_ref()
            .evaluate_with_tangent_on_side(guide.parameter, guide.side())?
            .tangent()
            .as_vector()
            .scaled(
                if self.reference.reversed_trim ^ self.reference.target.reversed {
                    -1.
                } else {
                    1.
                },
            )?;
        let outward = tangent.cross(normal)?.normalized_nonzero()?.as_vector();
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let a = outward.to_array();
        let b = tangent.to_array();
        let direction = Vector3::try_from(std::array::from_fn(|k| cos.mul_add(a[k], sin * b[k])))?
            .normalized_nonzero()?;
        Ok(if i == 0 {
            direction
        } else {
            direction.opposite()
        })
    }
    pub fn jet(&self, guide: &Guide, i: usize, order: u8) -> Result<CurveBlendJet, CommandError> {
        if order >= 3 && !self.planar {
            return Err(CommandError::Usage(
                "G3/G4 surface-edge blends currently require a planar face",
            ));
        }
        let location = self.reference.target.location(guide.point()?, true)?;
        let normal = location
            .surface
            .normal_at(location.uv[0], location.uv[1])?
            .as_vector()
            .scaled(if location.reversed { -1. } else { 1. })?;
        let direction = self.direction_with_normal(guide, i, normal)?;
        let zero = Vector3::try_new(0., 0., 0.)?;
        let curvature = if order >= 2 && !self.planar {
            let shape = location
                .surface
                .curvature_at(location.uv[0], location.uv[1])?;
            let a = direction.as_vector().dot(shape.directions[0].as_vector())?;
            let b = direction.as_vector().dot(shape.directions[1].as_vector())?;
            shape
                .normal
                .as_vector()
                .scaled(shape.principal[0] * a * a + shape.principal[1] * b * b)?
        } else {
            zero
        };
        Ok(
            CurveBlendJet::new(guide.point()?, Some(direction), curvature)
                .with_higher_derivatives(zero, zero),
        )
    }
}
