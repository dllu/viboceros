//! Cross-edge constraints use the selected face rather than boundary curvature.
use super::*;
use crate::surface_edge_reference::Edge;

#[derive(Clone, Debug)]
pub(super) struct SurfaceGuide {
    reference: Edge,
    uv: [Real; 2],
    pub angle: Real,
}
impl SurfaceGuide {
    pub fn capture(
        doc: &Document,
        id: ObjectId,
        edge: usize,
        point: Point3,
    ) -> Result<Self, CommandError> {
        let reference = Edge::capture(doc, id, edge, None).map_err(|error| match error {
            CommandError::Usage("Specify Face=index to choose the surface on a shared edge") => {
                CommandError::Usage("BlendCrv face selection on shared edges is not supported yet")
            }
            error => error,
        })?;
        let uv = reference.target.location(point, true)?.uv;
        Ok(Self {
            reference,
            uv,
            angle: 0.,
        })
    }
    pub fn move_contact(&mut self, point: Point3) -> Result<(), CommandError> {
        self.uv = self.reference.target.location(point, true)?.uv;
        Ok(())
    }
    fn normal(&self) -> Result<Vector3, CommandError> {
        Ok(self
            .reference
            .target
            .surface
            .normal_at(self.uv[0], self.uv[1])?
            .as_vector()
            .scaled(if self.reference.target.reversed {
                -1.
            } else {
                1.
            })?)
    }
    pub fn direction(&self, guide: &Guide, i: usize) -> Result<UnitVector3, CommandError> {
        let normal = self.normal()?;
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
        let normal = self.normal()?;
        let direction = self.direction_with_normal(guide, i, normal)?;
        let zero = Vector3::try_new(0., 0., 0.)?;
        if order >= 3 {
            let jet = self
                .reference
                .target
                .surface
                .evaluate_with_third_derivatives(self.uv[0], self.uv[1])?
                .normal_parameter_line_jet(direction)?;
            // Native uses the normalized straight-UV-line third target projected
            // onto the face normal, and retains its zero-fourth default for G4.
            return Ok(
                CurveBlendJet::new(guide.point()?, Some(direction), jet.curvature)
                    .with_higher_derivatives(jet.third_derivative, zero),
            );
        }
        let curvature = if order >= 2 {
            let shape = self
                .reference
                .target
                .surface
                .curvature_at(self.uv[0], self.uv[1])?;
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
