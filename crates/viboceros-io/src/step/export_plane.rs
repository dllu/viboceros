//! STEP plane placement with directions computed by the native mesh kernel.
use std::fmt;

use monstertruck::core::cgmath64::Vector3;
use monstertruck::step::save::{StepDisplay, StepFormat, StepLength, StepSurface};
use viboceros_geometry::{NurbsSurface, Point3, Tolerance, TriangleMesh};

use super::export_geometry::{ExportDirection, ExportPoint};
use super::{StepError, TruckPoint3};

#[derive(Clone)]
pub(super) struct ExportPlane {
    origin: TruckPoint3,
    normal: Vector3,
    reference: Vector3,
}

impl ExportPlane {
    pub(super) fn scaled(mut self, scale: f64) -> Result<Self, StepError> {
        let origin = Point3::try_new(
            self.origin.x * scale,
            self.origin.y * scale,
            self.origin.z * scale,
        )?;
        self.origin = TruckPoint3::new(origin.x(), origin.y(), origin.z());
        Ok(self)
    }

    pub(super) fn from_surface(
        surface: &NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, StepError> {
        if surface.plane(tolerance)?.is_none() {
            return Ok(None);
        }
        for (u0, u1) in surface.spans_u() {
            for (v0, v1) in surface.spans_v() {
                if let Ok(frame) =
                    surface.frame_at(u0 * 0.5 + u1 * 0.5, v0 * 0.5 + v1 * 0.5, tolerance)
                {
                    let origin = frame.origin();
                    let normal = frame.z_axis().as_vector();
                    let reference = frame.x_axis().as_vector();
                    return Ok(Some(Self {
                        origin: TruckPoint3::new(origin.x(), origin.y(), origin.z()),
                        normal: Vector3::new(normal.x(), normal.y(), normal.z()),
                        reference: Vector3::new(reference.x(), reference.y(), reference.z()),
                    }));
                }
            }
        }
        Ok(None)
    }

    pub(super) fn from_triangle(mesh: &TriangleMesh, face: usize) -> Result<Self, StepError> {
        let triangle = mesh.triangles()[face];
        let origin = mesh.vertices()[triangle[0] as usize];
        let normal = mesh.face_normal(face)?.as_vector();
        let reference = origin
            .vector_to(mesh.vertices()[triangle[1] as usize])?
            .normalized_nonzero()?
            .as_vector();
        Ok(Self {
            origin: TruckPoint3::new(origin.x(), origin.y(), origin.z()),
            normal: Vector3::new(normal.x(), normal.y(), normal.z()),
            reference: Vector3::new(reference.x(), reference.y(), reference.z()),
        })
    }
}

impl StepLength for ExportPlane {
    fn step_length(&self) -> usize {
        5
    }
}

impl StepSurface for ExportPlane {}

impl StepFormat for ExportPlane {
    fn fmt(&self, index: usize, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "#{index} = PLANE('', #{});", index + 1)?;
        writeln!(
            formatter,
            "#{} = AXIS2_PLACEMENT_3D('', #{}, #{}, #{});",
            index + 1,
            index + 2,
            index + 3,
            index + 4
        )?;
        write!(
            formatter,
            "{}{}{}",
            StepDisplay::new(ExportPoint(self.origin), index + 2),
            StepDisplay::new(ExportDirection(self.normal), index + 3),
            StepDisplay::new(ExportDirection(self.reference), index + 4)
        )
    }
}
