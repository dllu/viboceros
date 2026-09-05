//! Typed point syntax, separate from document edits and viewport interaction.

use thiserror::Error;
use viboceros_geometry::{Frame3, GeometryError, Point3, Real};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointInput {
    coordinates: [Real; 3],
    world: bool,
    relative: bool,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum PointInputError {
    #[error("enter x,y[,z], distance<angle[,z], or distance<angle<elevation")]
    Syntax,
    #[error("point coordinates must be finite numbers")]
    InvalidNumber,
    #[error("relative coordinates require a previous point")]
    MissingPreviousPoint,
    #[error("a nonzero number alone is a distance constraint; enter point coordinates instead")]
    DistanceConstraint,
    #[error("spherical elevation must be between -90 and 90 degrees after full-turn reduction")]
    ElevationRange,
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

impl PointInput {
    /// Parses a point-like token, returning `None` for ordinary command text.
    /// Coordinates have no internal whitespace. R/@ and W prefixes may be
    /// combined in either order; angles are decimal degrees, not radians.
    pub fn parse(text: &str) -> Option<Result<Self, PointInputError>> {
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
            || numeric_head.eq_ignore_ascii_case("infinity");
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
                coordinates: coordinates(body)?,
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

fn number(text: &str) -> Result<Real, PointInputError> {
    text.parse::<Real>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or(PointInputError::InvalidNumber)
}

fn coordinates(text: &str) -> Result<[Real; 3], PointInputError> {
    let angles: Vec<_> = text.split('<').collect();
    match angles.as_slice() {
        [cartesian] => {
            let components: Vec<_> = cartesian.split(',').collect();
            match components.as_slice() {
                [single] => {
                    if number(single)? == 0.0 {
                        Ok([0.0; 3])
                    } else {
                        Err(PointInputError::DistanceConstraint)
                    }
                }
                [x, y] => Ok([number(x)?, number(y)?, 0.0]),
                [x, y, z] => Ok([number(x)?, number(y)?, number(z)?]),
                _ => Err(PointInputError::Syntax),
            }
        }
        [radius, azimuth] if !radius.contains(',') => {
            let radius = number(radius)?;
            let components: Vec<_> = azimuth.split(',').collect();
            let (angle, z) = match components.as_slice() {
                [angle] => (number(angle)?, 0.0),
                [angle, z] => (number(angle)?, number(z)?),
                _ => return Err(PointInputError::Syntax),
            };
            let (sin, cos) = sin_cos_degrees(angle);
            Ok([radius * cos, radius * sin, z])
        }
        [radius, azimuth, elevation] => {
            let radius = number(radius)?;
            let (sin_a, cos_a) = sin_cos_degrees(number(azimuth)?);
            let elevation = reduced_degrees(number(elevation)?);
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
