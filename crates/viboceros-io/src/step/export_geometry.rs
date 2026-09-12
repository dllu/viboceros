//! Mesh export records with explicit STEP real-number syntax.
use super::TruckPoint3;
use monstertruck::core::cgmath64::{InnerSpace, Vector3};
use monstertruck::step::save::{StepCurve, StepDisplay, StepFormat, StepLength};
use std::fmt;

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
pub(super) struct ExportLine(pub TruckPoint3, pub TruckPoint3);
impl StepLength for ExportLine {
    fn step_length(&self) -> usize {
        4
    }
}
impl StepCurve for ExportLine {}
impl StepFormat for ExportLine {
    fn fmt(&self, index: usize, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vector = self.1 - self.0;
        let length = vector.magnitude();
        writeln!(f, "#{index} = LINE('', #{}, #{});", index + 1, index + 2)?;
        write!(f, "{}", StepDisplay::new(ExportPoint(self.0), index + 1))?;
        writeln!(
            f,
            "#{} = VECTOR('', #{}, {});",
            index + 2,
            index + 3,
            StepReal(length)
        )?;
        write!(
            f,
            "{}",
            StepDisplay::new(ExportDirection(vector / length), index + 3)
        )
    }
}
