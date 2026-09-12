//! STEP plane placement with directions computed by the native mesh kernel.
use std::fmt;

use monstertruck::core::cgmath64::Vector3;
use monstertruck::step::save::{StepDisplay, StepFormat, StepLength, StepSurface};
use viboceros_geometry::TriangleMesh;

use super::export_geometry::{ExportDirection, ExportPoint};
use super::{StepError, TruckPoint3};

pub(super) struct ExportPlane {
    origin: TruckPoint3,
    normal: Vector3,
    reference: Vector3,
}

impl ExportPlane {
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
