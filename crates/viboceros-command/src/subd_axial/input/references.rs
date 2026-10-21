//! Source-only circle handoff and rank-deficient fitted bases.
use super::*;
impl Input {
    /// Accept an independently solved reference without adding a temporary circle.
    pub fn reference_circle(&mut self, circle: Circle3) -> Result<(), CommandError> {
        if !self.at_start() || self.pending.is_some() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        self.circle = Some(CircleDefinition::from_circle(circle)?);
        self.preferences.radius = circle.radius();
        self.cursor = None;
        Ok(())
    }
    pub fn tangent_curve_circle(&mut self, circle: Circle3) -> Result<(), CommandError> {
        self.reference_circle(circle)?;
        self.allow_angles = false;
        Ok(())
    }
    /// Native round SubD primitives request height for a zero-radius point fit.
    /// Keep that base staged; a truncated cone can accept a nonzero end radius.
    /// Other primitives reject it at geometry admission without a model edit.
    pub fn fit_points(&mut self, document: &Document) -> Result<(), CommandError> {
        if !self.at_start() || self.pending.is_some() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if let Some(circle) = crate::circle_fit_points::selected_circle(document)? {
            return self.reference_circle(circle);
        }
        let mut sums: [viboceros_geometry::FiniteSum; 3] =
            std::array::from_fn(|_| Default::default());
        for point in crate::circle_fit_points::selected_points(document) {
            for (sum, value) in sums.iter_mut().zip(point.to_array()) {
                sum.add(value)?;
            }
        }
        let center = Point3::try_new(sums[0].mean()?, sums[1].mean()?, sums[2].mean()?)?;
        let frame = if self.kind == Kind::TruncatedCone {
            let normal = if let Some(point) =
                crate::circle_fit_points::selected_points(document).find(|&p| p != center)
            {
                Frame3::try_from_unit_normal(
                    center,
                    center.vector_to(point)?.normalized_nonzero()?,
                )?
                .y_axis()
            } else {
                viboceros_geometry::Vector3::try_new(1., 0., 0.)?.normalized_nonzero()?
            };
            Frame3::try_from_unit_normal(center, normal)?
        } else {
            self.getter.construction_plane().with_origin(center)
        };
        self.circle = Some(CircleDefinition { frame, radius: 0. });
        Ok(())
    }
}
