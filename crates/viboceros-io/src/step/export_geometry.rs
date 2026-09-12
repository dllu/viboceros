//! Mesh export records with explicit STEP real-number syntax.
use super::TruckPoint3;
use monstertruck::core::cgmath64::Vector3;
use monstertruck::step::save::{StepCurve, StepDisplay, StepFormat, StepLength};
use std::fmt;
use std::fmt::Write as _;
use viboceros_geometry::{GeometryError, Point3};

pub(super) struct StepReal(pub f64);

// Shortest binary64 scientific notation needs at most 24 ASCII bytes:
// sign, one digit, decimal point, 16 digits, 'e', exponent sign, three digits.
// Leave spare room and return a formatting error instead of truncating.
struct NumberBuffer {
    bytes: [u8; 32],
    length: usize,
}

impl fmt::Write for NumberBuffer {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.length.checked_add(value.len()).ok_or(fmt::Error)?;
        let destination = self.bytes.get_mut(self.length..end).ok_or(fmt::Error)?;
        destination.copy_from_slice(value.as_bytes());
        self.length = end;
        Ok(())
    }
}

impl fmt::Display for StepReal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = NumberBuffer {
            bytes: [0; 32],
            length: 0,
        };
        write!(&mut buffer, "{:e}", self.0)?;
        let text = std::str::from_utf8(&buffer.bytes[..buffer.length]).map_err(|_| fmt::Error)?;
        let (mantissa, exponent) = text.split_once('e').ok_or(fmt::Error)?;
        write!(
            f,
            "{mantissa}{}E{exponent}",
            if mantissa.contains('.') { "" } else { ".0" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_buffer_rejects_overflow_without_truncation_or_partial_append() {
        let mut buffer = NumberBuffer {
            bytes: [0; 32],
            length: 0,
        };
        buffer.write_str("1234").unwrap();
        let before = buffer.bytes;
        assert!(buffer.write_str("12345678901234567890123456789").is_err());
        assert_eq!(buffer.length, 4);
        assert_eq!(buffer.bytes, before);
        buffer.write_str("1234567890123456789012345678").unwrap();
        assert_eq!(buffer.length, 32);
        assert!(buffer.write_str("x").is_err());
        buffer.write_str("").unwrap();
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
