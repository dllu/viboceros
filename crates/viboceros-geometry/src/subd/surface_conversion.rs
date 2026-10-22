//! Surface control nets reuse the existing cleaned control-polygon mesh.
use super::*;
use crate::{NurbsSurface, Tolerance};
mod axis;
mod features;
mod fitting;
mod poles;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SubDFromSurfaceMethod {
    ControlPoints,
    #[default]
    Location,
}

impl SubD {
    /// Converts a surface control net or fits its Greville samples to natural
    /// or periodic cubic control rows before constructing the subdivision net.
    pub fn try_from_surface(
        surface: &NurbsSurface,
        method: SubDFromSurfaceMethod,
        corners: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        match method {
            SubDFromSurfaceMethod::ControlPoints => {
                Self::try_from_surface_control_net(surface, corners, tolerance)
            }
            SubDFromSurfaceMethod::Location => {
                let fitted = fitting::fit(surface, tolerance)?;
                let converted =
                    Self::try_from_surface_control_net(&fitted.surface, corners, tolerance)?;
                features::apply_creases(converted, &fitted.creases)
            }
        }
    }

    /// Converts Euclidean surface controls, retaining closed seams and singular
    /// sides through the shared control-polygon cleanup. Rational weights do
    /// not change this control-net interpretation. The input stays unchanged.
    pub fn try_from_surface_control_net(
        surface: &NurbsSurface,
        corners: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mesh = surface
            .control_polygon_mesh(tolerance)?
            .ok_or(GeometryError::EmptyMesh)?;
        // A cleaned mesh must still contain each source pole. Otherwise a
        // collapsed first band would silently discard part of the surface.
        let singular = singular_sides(surface);
        let u = surface.control_point_count_u();
        let v = surface.control_point_count_v();
        for (side, index) in [0, u - 1, (v - 1) * u, 0].into_iter().enumerate() {
            if singular[side]
                && !mesh
                    .vertices()
                    .contains(&surface.control_points()[index].point())
            {
                return Err(GeometryError::EmptyMesh);
            }
        }
        let mut converted = Self::try_from_mesh(&mesh, SubDFromMeshOptions::default(), tolerance)?;
        if corners && !surface.is_closed_u()? && !surface.is_closed_v()? {
            let u = surface.control_point_count_u();
            let v = surface.control_point_count_v();
            let wanted =
                [0, u - 1, (v - 1) * u, u * v - 1].map(|i| surface.control_points()[i].point());
            let tags = converted
                .control_points()
                .iter()
                .enumerate()
                .filter_map(|(i, p)| wanted.contains(p).then_some((i, 3)))
                .collect::<Vec<_>>();
            converted = converted.try_with_vertex_tags(&tags)?;
        }
        Ok(converted)
    }
}

fn singular_sides(surface: &NurbsSurface) -> [bool; 4] {
    use crate::nurbs_surface::{
        control_net_singular_sides, knots_are_clamped_at_end, knots_are_clamped_at_start,
    };
    control_net_singular_sides(
        &surface
            .control_points()
            .iter()
            .map(|p| p.point())
            .collect::<Vec<_>>(),
        surface.control_point_count_u(),
        surface.control_point_count_v(),
        [
            knots_are_clamped_at_start(surface.degree_v(), surface.knots_v()),
            knots_are_clamped_at_end(surface.degree_u(), surface.knots_u()),
            knots_are_clamped_at_end(surface.degree_v(), surface.knots_v()),
            knots_are_clamped_at_start(surface.degree_u(), surface.knots_u()),
        ],
    )
}
