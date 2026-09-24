//! Typed point syntax, separate from document edits and viewport interaction.

mod calculator;

use thiserror::Error;
use viboceros_geometry::{Frame3, GeometryError, LengthUnitSystem, Point3, Real};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointInput {
    coordinates: [Real; 3],
    world: bool,
    relative: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointConstraintInput {
    Distance(Real),
    Angle(Real),
}

impl PointConstraintInput {
    pub fn parse_with_units(
        text: &str,
        units: &LengthUnitSystem,
    ) -> Option<Result<Self, PointInputError>> {
        let text = text.trim();
        if let Some(angle) = text.strip_prefix('<') {
            return Some(
                if angle.is_empty()
                    || angle.contains([',', '<'])
                    || text.len() > 512
                    || angle.chars().any(char::is_whitespace)
                {
                    Err(PointInputError::Syntax)
                } else {
                    angle_number(angle, units).map(Self::Angle)
                },
            );
        }
        if matches!(
            PointInput::parse_with_units(text, units),
            Some(Err(PointInputError::DistanceConstraint))
        ) {
            return Some(number(text, units).map(Self::Distance));
        }
        None
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum PointInputError {
    #[error(
        "enter x,y[,z], x,y<elevation, distance<angle[,z], distance<angle<elevation, or distance<N/Sangle E/W"
    )]
    Syntax,
    #[error("point coordinates must be finite numbers")]
    InvalidNumber,
    #[error("relative coordinates require a previous point")]
    MissingPreviousPoint,
    #[error("a nonzero number alone is a distance constraint at a point prompt")]
    DistanceConstraint,
    #[error("spherical elevation must be between -90 and 90 degrees after full-turn reduction")]
    ElevationRange,
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

impl PointInput {
    /// Parses a point-like token, returning `None` for ordinary command text.
    /// Coordinates have no internal whitespace. R/@ and W prefixes may be
    /// combined in either order; unsuffixed angles are decimal degrees.
    pub fn parse(text: &str) -> Option<Result<Self, PointInputError>> {
        Self::parse_with_units(text, &LengthUnitSystem::Millimeters)
    }

    /// Interpret explicit length suffixes in the caller's current model units.
    pub fn parse_with_units(
        text: &str,
        units: &LengthUnitSystem,
    ) -> Option<Result<Self, PointInputError>> {
        let text = text.trim();
        let mut body = text;
        let (mut world, mut relative, mut duplicate) = (false, false, false);
        while let Some(prefix) = body.chars().next() {
            match prefix {
                'w' | 'W' => {
                    duplicate |= world;
                    world = true;
                }
                'r' | 'R' | '@' => {
                    duplicate |= relative;
                    relative = true;
                }
                _ => break,
            }
            body = &body[prefix.len_utf8()..];
        }
        let head = body.split([',', '<']).next().unwrap_or("").trim_start();
        let numeric_head = head.trim_start_matches(['+', '-']).trim_start();
        let numeric = numeric_head
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || c == '.')
            || body.starts_with([',', '<'])
            || text
                .split_whitespace()
                .next()
                .is_some_and(|token| token.contains([',', '<']))
            || ((world || relative) && body.is_empty())
            || numeric_head.eq_ignore_ascii_case("nan")
            || numeric_head.eq_ignore_ascii_case("inf")
            || numeric_head.eq_ignore_ascii_case("infinity")
            || numeric_head.eq_ignore_ascii_case("pi")
            || [
                "sin(", "cos(", "tan(", "asin(", "acos(", "atan(", "atan2(", "ln(", "log10(",
                "exp(", "sinh(", "cosh(", "tanh(", "pow(", "sqrt(",
            ]
            .iter()
            .any(|prefix| {
                numeric_head
                    .get(..prefix.len())
                    .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
            });
        // Do not mistake Rotate, Rebuild, Weld, or a full command containing
        // coordinate arguments for point continuation.
        if !numeric {
            return None;
        }
        Some((|| {
            if duplicate || text.len() > 512 || body.chars().any(char::is_whitespace) {
                return Err(PointInputError::Syntax);
            }
            Ok(Self {
                coordinates: coordinates(body, units)?,
                world,
                relative,
            })
        })())
    }

    /// Resolves construction-plane/world coordinates or a displacement from
    /// the caller's last accepted point. Snapping never modifies typed values.
    pub fn resolve(
        self,
        plane: Frame3,
        previous: Option<Point3>,
    ) -> Result<Point3, PointInputError> {
        let origin = if self.relative {
            previous.ok_or(PointInputError::MissingPreviousPoint)?
        } else if self.world {
            Point3::try_new(0.0, 0.0, 0.0)?
        } else {
            plane.origin()
        };
        let offset = if self.world {
            self.coordinates
        } else {
            let axes = plane.axes().map(|axis| axis.as_vector().to_array());
            std::array::from_fn(|axis| {
                self.coordinates[0].mul_add(
                    axes[0][axis],
                    self.coordinates[1].mul_add(axes[1][axis], self.coordinates[2] * axes[2][axis]),
                )
            })
        };
        Ok(Point3::try_from(std::array::from_fn(|axis| {
            origin.to_array()[axis] + offset[axis]
        }))?)
    }
}

fn number(text: &str, units: &LengthUnitSystem) -> Result<Real, PointInputError> {
    calculator::evaluate_in_units(text, units).ok_or(PointInputError::InvalidNumber)
}

fn angle_number(text: &str, units: &LengthUnitSystem) -> Result<Real, PointInputError> {
    // Rhino's point prompt accepts calculator functions in coordinates but
    // rejects function calls after the polar/spherical angle separator.
    if text
        .as_bytes()
        .windows(2)
        .any(|pair| pair[1] == b'(' && pair[0].is_ascii_alphanumeric())
    {
        return Err(PointInputError::Syntax);
    }
    if text.contains(['\'', '"']) {
        return dms_angle(text);
    }
    let lower = text.to_ascii_lowercase();
    for (suffix, multiplier) in [
        ("gradians", 0.9),
        ("degrees", 1.0),
        ("radians", 180.0 / std::f64::consts::PI),
        ("d", 1.0),
    ] {
        if lower.ends_with(suffix) {
            let value = number(&text[..text.len() - suffix.len()], units)? * multiplier;
            return value
                .is_finite()
                .then_some(value)
                .ok_or(PointInputError::InvalidNumber);
        }
    }
    number(text, units)
}

fn dms_angle(text: &str) -> Result<Real, PointInputError> {
    let (sign, unsigned) = match text.as_bytes().first() {
        Some(b'-') => (-1.0, &text[1..]),
        Some(b'+') => (1.0, &text[1..]),
        _ => (1.0, text),
    };
    let (degrees, minutes_seconds) = match unsigned.find(['d', 'D']) {
        Some(at) => (unsigned[..at].parse::<Real>().ok(), &unsigned[at + 1..]),
        None => (Some(0.0), unsigned),
    };
    let (minutes, seconds) = minutes_seconds
        .split_once('\'')
        .ok_or(PointInputError::Syntax)?;
    let minutes = minutes
        .parse::<Real>()
        .map_err(|_| PointInputError::Syntax)?;
    let seconds = if seconds.is_empty() {
        0.0
    } else {
        seconds
            .strip_suffix('"')
            .ok_or(PointInputError::Syntax)?
            .parse::<Real>()
            .map_err(|_| PointInputError::Syntax)?
    };
    let degrees = degrees.ok_or(PointInputError::Syntax)?;
    let value = sign * (degrees + minutes / 60.0 + seconds / 3600.0);
    if !value.is_finite() || !(0.0..60.0).contains(&minutes) || !(0.0..60.0).contains(&seconds) {
        return Err(PointInputError::InvalidNumber);
    }
    Ok(value)
}

fn coordinates(text: &str, units: &LengthUnitSystem) -> Result<[Real; 3], PointInputError> {
    let angles: Vec<_> = text.split('<').collect();
    match angles.as_slice() {
        [cartesian] => {
            let components = split_components(cartesian);
            match components.as_slice() {
                [single] => {
                    if number(single, units)? == 0.0 {
                        Ok([0.0; 3])
                    } else {
                        Err(PointInputError::DistanceConstraint)
                    }
                }
                [x, y] => Ok([number(x, units)?, number(y, units)?, 0.0]),
                [x, y, z] => Ok([number(x, units)?, number(y, units)?, number(z, units)?]),
                _ => Err(PointInputError::Syntax),
            }
        }
        [xy, elevation] if split_components(xy).len() > 1 => {
            let components = split_components(xy);
            let [x, y] = components.as_slice() else {
                return Err(PointInputError::Syntax);
            };
            let [x, y] = [number(x, units)?, number(y, units)?];
            let elevation = reduced_degrees(angle_number(elevation, units)?);
            if !(-90.0..=90.0).contains(&elevation) {
                return Err(PointInputError::ElevationRange);
            }
            if (x == 0.0 && y == 0.0) || elevation == 0.0 {
                return Ok([x, y, 0.0]);
            }
            let (sin, cos) = sin_cos_degrees(elevation);
            if cos == 0.0 {
                return Err(PointInputError::InvalidNumber);
            }
            let scale = x.abs().max(y.abs());
            let horizontal = (x / scale).hypot(y / scale);
            let slope = sin / cos;
            let height = if slope.abs() <= 1.0 {
                scale * (horizontal * slope)
            } else {
                (scale * slope) * horizontal
            };
            if !height.is_finite() {
                return Err(PointInputError::InvalidNumber);
            }
            Ok([x, y, height])
        }
        [radius, azimuth] => {
            let radius = number(radius, units)?;
            let components = split_components(azimuth);
            if let [angle] = components.as_slice()
                && angle.len() >= 3
                && angle.starts_with(['N', 'n', 'S', 's'])
            {
                let north = angle.starts_with(['N', 'n']);
                let east = angle.ends_with(['E', 'e']);
                if !east && !angle.ends_with(['W', 'w']) {
                    return Err(PointInputError::Syntax);
                }
                let bearing = angle_number(&angle[1..angle.len() - 1], units)?;
                let (sin, cos) = sin_cos_degrees(bearing);
                return Ok([
                    radius * if east { sin } else { -sin },
                    radius * if north { cos } else { -cos },
                    0.0,
                ]);
            }
            let (angle, z) = match components.as_slice() {
                [angle] => (angle_number(angle, units)?, 0.0),
                [angle, z] => (angle_number(angle, units)?, number(z, units)?),
                _ => return Err(PointInputError::Syntax),
            };
            let (sin, cos) = sin_cos_degrees(angle);
            Ok([radius * cos, radius * sin, z])
        }
        [radius, azimuth, elevation] => {
            let radius = number(radius, units)?;
            let (sin_a, cos_a) = sin_cos_degrees(angle_number(azimuth, units)?);
            let elevation = reduced_degrees(angle_number(elevation, units)?);
            if !(-90.0..=90.0).contains(&elevation) {
                return Err(PointInputError::ElevationRange);
            }
            let (sin_e, cos_e) = sin_cos_degrees(elevation);
            let horizontal = radius * cos_e;
            // Rhino's signed distance reverses the horizontal bearing;
            // elevation retains its own sign above/below the plane.
            Ok([horizontal * cos_a, horizontal * sin_a, radius.abs() * sin_e])
        }
        _ => Err(PointInputError::Syntax),
    }
}

fn split_components(text: &str) -> Vec<&str> {
    let mut components = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0;
    for (index, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                components.push(&text[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        if depth < 0 {
            return Vec::new();
        }
    }
    if depth != 0 {
        return Vec::new();
    }
    components.push(&text[start..]);
    components
}

fn reduced_degrees(degrees: Real) -> Real {
    // Reduce before converting, so even finite huge angles do not overflow.
    // Exact quadrants avoid large spurious offsets at huge model scales.
    // Signed remainder retains tiny negative angles. Adding 360 first can
    // round them to a full turn and introduce a large spurious displacement.
    let mut angle = degrees % 360.0;
    if angle > 180.0 {
        angle -= 360.0;
    }
    if angle < -180.0 {
        angle += 360.0;
    }
    angle
}

fn sin_cos_degrees(degrees: Real) -> (Real, Real) {
    match reduced_degrees(degrees) {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        180.0 | -180.0 => (0.0, -1.0),
        -90.0 => (-1.0, 0.0),
        angle => angle.to_radians().sin_cos(),
    }
}

#[cfg(test)]
mod tests;
