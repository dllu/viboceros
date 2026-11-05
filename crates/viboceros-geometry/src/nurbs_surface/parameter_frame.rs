//! Lossless local parameter origins shared by surfaces and trimmed faces.
use super::*;
use crate::parameter::{exact_difference, lossless_parameter_origin};
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
        self.parameter_frame(coordinates, false)
    }

    /// Finite mesh grids start at each chart's lower endpoint. Prefer that
    /// endpoint when knots and trim controls all translate without rounding,
    /// including negative domains ending at zero.
    pub(crate) fn sampling_parameter_frame(
        &self,
        coordinates: impl Iterator<Item = [Real; 2]> + Clone,
    ) -> Result<SurfaceParameterFrame<'_>, GeometryError> {
        self.parameter_frame(coordinates, true)
    }

    fn parameter_frame(
        &self,
        coordinates: impl Iterator<Item = [Real; 2]> + Clone,
        sampling: bool,
    ) -> Result<SurfaceParameterFrame<'_>, GeometryError> {
        let knots = [self.knots_u(), self.knots_v()];
        let domains = [self.domain_u(), self.domain_v()];
        let origin = std::array::from_fn(|axis| {
            let values = || {
                knots[axis]
                    .iter()
                    .copied()
                    .chain(coordinates.clone().map(move |p| p[axis]))
            };
            let start = *domains[axis].start();
            if sampling && (start == 0. || values().all(|v| exact_difference(v, start).is_some())) {
                return start;
            }
            lossless_parameter_origin(domains[axis].clone(), values())
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
