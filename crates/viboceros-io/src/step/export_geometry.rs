//! Mesh export records with explicit STEP real-number syntax.
use super::TruckPoint3;
use monstertruck::core::cgmath64::Vector3;
use monstertruck::step::save::{StepCurve, StepDisplay, StepFormat, StepLength};
use std::fmt;
use viboceros_geometry::{GeometryError, Point3};

pub(super) struct StepReal(pub f64);
impl fmt::Display for StepReal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = format!("{:e}", self.0);
        let (mantissa, exponent) = text.split_once('e').ok_or(fmt::Error)?;
        write!(
            f,
            "{mantissa}{}E{exponent}",
            if mantissa.contains('.') { "" } else { ".0" }
        )
    }
}
#[derive(Clone, Copy)]
pub(super) struct ExportPoint(pub TruckPoint3);
impl StepLength for ExportPoint {
    fn step_length(&self) -> usize {
        1
    }
}
impl StepFormat for ExportPoint {
    fn fmt(&self, index: usize, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "#{index} = CARTESIAN_POINT('', ({}, {}, {}));",
            StepReal(self.0.x),
            StepReal(self.0.y),
            StepReal(self.0.z)
        )
    }
}
pub(super) struct ExportDirection(pub Vector3);
impl StepFormat for ExportDirection {
    fn fmt(&self, index: usize, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "#{index} = DIRECTION('', ({}, {}, {}));",
            StepReal(self.0.x),
            StepReal(self.0.y),
            StepReal(self.0.z)
        )
    }
}
pub(super) struct ExportLine {
    origin: TruckPoint3,
    direction: Vector3,
    length: f64,
}
impl ExportLine {
    pub(super) fn try_new(start: Point3, end: Point3) -> Result<Self, GeometryError> {
        let vector = start.vector_to(end)?;
        let length = vector.length()?;
        let direction = vector.normalized_nonzero()?.as_vector();
        Ok(Self {
            origin: TruckPoint3::new(start.x(), start.y(), start.z()),
            direction: Vector3::new(direction.x(), direction.y(), direction.z()),
            length,
        })
    }
}
impl StepLength for ExportLine {
    fn step_length(&self) -> usize {
        4
    }
}
impl StepCurve for ExportLine {}
impl StepFormat for ExportLine {
    fn fmt(&self, index: usize, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "#{index} = LINE('', #{}, #{});", index + 1, index + 2)?;
        write!(
            f,
            "{}",
            StepDisplay::new(ExportPoint(self.origin), index + 1)
        )?;
        writeln!(
            f,
            "#{} = VECTOR('', #{}, {});",
            index + 2,
            index + 3,
            StepReal(self.length)
        )?;
        write!(
            f,
            "{}",
            StepDisplay::new(ExportDirection(self.direction), index + 3)
        )
    }
}
