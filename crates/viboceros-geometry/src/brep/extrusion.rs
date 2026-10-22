//! A typed native extrusion plus its validated canonical B-rep view.
use super::*;
use std::sync::Arc;

/// Immutable extrusion data retained alongside its exact B-rep view. Only
/// constructors that decode both views together can attach this representation.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtrusionRepresentation {
    archive: Arc<[u8]>,
}
impl ExtrusionRepresentation {
    pub fn archive(&self) -> &[u8] {
        &self.archive
    }
    /// Complete profile NURBS form, including the native profile joining policy.
    pub fn nurbs_form(&self) -> Result<ExtrusionNurbsForm, GeometryError> {
        let data = viboceros_opennurbs::extrusion_surface(self.archive())?;
        let surface =
            crate::interop::decode_surface(&data.surface).map_err(|error| match error {
                crate::interop::GeometryCodecError::Geometry(e) => e,
                _ => viboceros_opennurbs::ExtrusionError("Invalid NURBS surface transfer".into())
                    .into(),
            })?;
        Ok(ExtrusionNurbsForm {
            surface,
            caps: data.caps,
            path_axis: data.path_axis,
        })
    }
}
pub struct ExtrusionNurbsForm {
    pub surface: NurbsSurface,
    pub caps: [bool; 2],
    pub path_axis: usize,
}
impl Brep {
    pub fn extrusion(&self) -> Option<&ExtrusionRepresentation> {
        self.extrusion.as_ref()
    }
    pub fn is_extrusion(&self) -> bool {
        self.extrusion.is_some()
    }
    /// Returns an independent explicit B-rep, removing its extrusion representation.
    pub fn explicit_brep(&self) -> Self {
        let mut result = self.clone();
        result.extrusion = None;
        result
    }
    pub fn try_from_extrusion_archive(
        bytes: &[u8],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::from_extrusion_data(viboceros_opennurbs::extrusion_read(bytes)?, tolerance)
    }
    /// Create the public extrusion form of a planar profile, projected onto
    /// the given frame, with optional caps on a closed profile.
    pub fn try_planar_profile_extrusion(
        profile: &NurbsCurve,
        frame: Frame3,
        height: Real,
        cap: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::profile_extrusion(profile, frame, height, cap, false, tolerance)
    }
    fn profile_extrusion(
        profile: &NurbsCurve,
        frame: Frame3,
        height: Real,
        cap: bool,
        command: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let controls = profile
            .control_points()
            .iter()
            .map(|p| {
                let [x, y, z] = p.point().to_array();
                [x, y, z, p.weight()]
            })
            .collect::<Vec<_>>();
        let mut plane = [0.; 9];
        plane[..3].copy_from_slice(&frame.origin().to_array());
        plane[3..6].copy_from_slice(&frame.x_axis().as_vector().to_array());
        plane[6..].copy_from_slice(&frame.y_axis().as_vector().to_array());
        Self::from_extrusion_data(
            (if command {
                viboceros_opennurbs::extrusion_create_command
            } else {
                viboceros_opennurbs::extrusion_create
            })(
                viboceros_opennurbs::ExtrusionProfile {
                    degree: profile.degree() as u32,
                    controls: &controls,
                    knots: profile.knots(),
                },
                plane,
                height,
                cap,
            )?,
            tolerance,
        )
    }
    /// Command form for a perpendicular planar profile. Oblique or nonplanar
    /// inputs return None so the modeling kernel can construct an explicit B-rep.
    pub fn try_command_curve_extrusion(
        curve: &NurbsCurve,
        start: Vector3,
        end: Vector3,
        cap: bool,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let displacement = Vector3::try_from(std::array::from_fn(|i| {
            end.to_array()[i] - start.to_array()[i]
        }))?;
        let normal = displacement.normalized(tolerance)?;
        let points = curve
            .control_points()
            .iter()
            .map(|p| p.point())
            .collect::<Vec<_>>();
        if let Some(plane) = crate::try_control_polygon_plane(&points, tolerance)? {
            if plane
                .normal()
                .as_vector()
                .cross(normal.as_vector())?
                .length()?
                > tolerance.angular()
            {
                return Ok(None);
            }
        } else {
            if !curve.is_linear(tolerance)? {
                return Ok(None);
            }
            let tangent = curve
                .evaluate(*curve.domain().start())?
                .vector_to(curve.evaluate(*curve.domain().end())?)?
                .normalized_nonzero()?;
            if tangent.as_vector().dot(normal.as_vector())?.abs() > tolerance.angular() {
                return Ok(None);
            }
        }
        let profile = curve.transformed(AffineTransform3::from_translation(start))?;
        let origin = profile.evaluate(*profile.domain().start())?;
        let frame = Frame3::try_from_unit_normal(origin, normal)?;
        Ok(Some(Self::profile_extrusion(
            &profile,
            frame,
            displacement.length()?,
            cap,
            true,
            tolerance,
        )?))
    }
    /// A closed box represented by a planar rectangular profile and a signed
    /// path. Intervals are measured in the supplied frame's XY plane.
    pub fn try_box_extrusion(
        frame: Frame3,
        xy: [[Real; 2]; 2],
        height: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        crate::require_finite(
            xy.into_iter().flatten().chain([height]),
            "extrusion box dimensions",
        )?;
        if xy.iter().any(|i| i[0] >= i[1]) || height == 0. {
            return Err(GeometryError::Degenerate {
                context: "extrusion box",
            });
        }
        let [[x0, x1], [y0, y1]] = xy;
        let corners = if height > 0. {
            [
                [x0, y1, 0.],
                [x0, y0, 0.],
                [x1, y0, 0.],
                [x1, y1, 0.],
                [x0, y1, 0.],
            ]
        } else {
            [
                [x0, y0, 0.],
                [x0, y1, 0.],
                [x1, y1, 0.],
                [x1, y0, 0.],
                [x0, y0, 0.],
            ]
        };
        let points = corners
            .into_iter()
            .map(|p| frame.point_at(p))
            .collect::<Result<Vec<_>, _>>()?;
        let x = points[0].vector_to(points[1])?.normalized_nonzero()?;
        let normal = if height > 0. {
            frame.z_axis()
        } else {
            frame.z_axis().opposite()
        };
        let profile_frame = Frame3::try_from_unit_x_and_normal(points[0], x, normal, tolerance)?;
        let profile =
            crate::Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION)?.to_nurbs()?;
        Self::profile_extrusion(
            &profile,
            profile_frame,
            height.abs(),
            true,
            false,
            tolerance,
        )
    }
    /// A cylinder whose path follows the signed start/end heights. Its profile
    /// interval is the circumference and its path interval starts at zero.
    pub fn try_cylinder_extrusion(
        frame: Frame3,
        radius: Real,
        heights: [Real; 2],
        cap: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        crate::require_finite(heights, "extrusion cylinder heights")?;
        let height = heights[1] - heights[0];
        let origin = frame.point_at([0., 0., heights[0]])?;
        let normal = if height < 0. {
            frame.z_axis().opposite()
        } else {
            frame.z_axis()
        };
        let profile_frame =
            Frame3::try_from_unit_x_and_normal(origin, frame.x_axis(), normal, tolerance)?;
        let profile = crate::Circle3::try_from_frame(
            origin,
            radius,
            profile_frame.x_axis(),
            normal,
            tolerance,
        )?
        .to_nurbs()?;
        Self::profile_extrusion(&profile, profile_frame, height.abs(), cap, false, tolerance)
    }
    pub(super) fn from_extrusion_data(
        data: viboceros_opennurbs::ExtrusionData,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut brep =
            crate::interop::decode_brep(&data.brep, tolerance).map_err(|error| match error {
                crate::interop::GeometryCodecError::Geometry(e) => e,
                _ => viboceros_opennurbs::ExtrusionError("Invalid canonical B-rep transfer".into())
                    .into(),
            })?;
        brep.extrusion = Some(ExtrusionRepresentation {
            archive: data.archive.into(),
        });
        Ok(brep)
    }
}
