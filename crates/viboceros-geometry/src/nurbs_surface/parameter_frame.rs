//! Lossless local parameter origins shared by surfaces and trimmed faces.
use super::*;
use crate::parameter::exact_difference;
use std::borrow::Cow;

pub(crate) struct SurfaceParameterFrame<'a> {
    pub(crate) surface: Cow<'a, NurbsSurface>,
    pub(crate) origin: [Real; 2],
}

impl NurbsSurface {
    /// Translates all knots and caller-supplied UV coordinates exactly, or
    /// declines that axis. Callers still validate their own parameter ranges.
    pub(crate) fn local_parameter_frame(
        &self,
        coordinates: impl Iterator<Item = [Real; 2]> + Clone,
    ) -> Result<SurfaceParameterFrame<'_>, GeometryError> {
        let knots = [self.knots_u(), self.knots_v()];
        let domains = [self.domain_u(), self.domain_v()];
        let origin = std::array::from_fn(|axis| {
            let domain = &domains[axis];
            let candidate = if *domain.start() > 0. {
                *domain.start()
            } else if *domain.end() < 0. {
                *domain.end()
            } else {
                return 0.;
            };
            if knots[axis]
                .iter()
                .copied()
                .chain(coordinates.clone().map(|p| p[axis]))
                .all(|value| exact_difference(value, candidate).is_some())
            {
                candidate
            } else {
                0.
            }
        });
        let surface = if origin == [0.; 2] {
            Cow::Borrowed(self)
        } else {
            Cow::Owned(Self::try_new_rational(
                self.degree_u,
                self.degree_v,
                self.control_point_count_u,
                self.control_point_count_v,
                self.control_points.clone(),
                knots[0].iter().map(|k| k - origin[0]).collect(),
                knots[1].iter().map(|k| k - origin[1]).collect(),
            )?)
        };
        Ok(SurfaceParameterFrame { surface, origin })
    }
}
