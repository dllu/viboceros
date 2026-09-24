//! Coordinate filters for interactive 3D point prompts.

use thiserror::Error;
use viboceros_geometry::{Frame3, GeometryError, Point3, Real};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointFilter {
    axes: [bool; 3],
    world: bool,
}

impl PointFilter {
    pub fn parse(text: &str) -> Option<Self> {
        let body = text.strip_prefix('.')?.to_ascii_lowercase();
        let (world, axes) = if let Some(axes) = body.strip_prefix('w') {
            (true, axes)
        } else {
            (false, body.as_str())
        };
        let axes = match axes {
            "x" => [true, false, false],
            "y" => [false, true, false],
            "z" => [false, false, true],
            "xy" | "yx" => [true, true, false],
            "xz" | "zx" => [true, false, true],
            "yz" | "zy" => [false, true, true],
            _ => return None,
        };
        Some(Self { axes, world })
    }

    pub const fn world(self) -> bool {
        self.world
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum PointFilterError {
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointFilterSession {
    plane: Frame3,
    world: bool,
    fixed: [Option<Real>; 3],
    pending: Option<PointFilter>,
}

impl PointFilterSession {
    pub fn new(filter: PointFilter, plane: Frame3) -> Self {
        Self {
            plane,
            world: filter.world,
            fixed: [None; 3],
            pending: Some(filter),
        }
    }

    pub fn awaiting_source(self) -> bool {
        self.pending.is_some()
    }

    /// A source pick records filtered coordinates. A later pick fills all
    /// unfiltered coordinates and returns the point to submit to the command.
    pub fn offer_point(&mut self, point: Point3) -> Result<Option<Point3>, PointFilterError> {
        let coordinates = if self.world {
            point.to_array()
        } else {
            self.plane.coordinates_of(point)?
        };
        if let Some(filter) = self.pending {
            for (axis, selected) in filter.axes.into_iter().enumerate() {
                if selected {
                    self.fixed[axis] = Some(coordinates[axis]);
                }
            }
            self.pending = None;
            return Ok(None);
        }
        Ok(Some(self.resolved_point(coordinates)?))
    }

    pub fn preview_point(self, point: Point3) -> Result<Point3, PointFilterError> {
        if self.pending.is_some() {
            return Ok(point);
        }
        let coordinates = if self.world {
            point.to_array()
        } else {
            self.plane.coordinates_of(point)?
        };
        self.resolved_point(coordinates)
    }

    fn resolved_point(self, coordinates: [Real; 3]) -> Result<Point3, PointFilterError> {
        let combined = std::array::from_fn(|axis| self.fixed[axis].unwrap_or(coordinates[axis]));
        Ok(if self.world {
            Point3::try_from(combined)?
        } else {
            self.plane.point_at(combined)?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{Tolerance, Vector3};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn accepts_documented_filter_names_and_rejects_other_dot_commands() {
        for name in [".x", ".y", ".z", ".xy", ".yx", ".xz", ".zx", ".yz", ".zy"] {
            assert!(PointFilter::parse(name).is_some());
            assert!(
                PointFilter::parse(&format!(".w{}", &name[1..]))
                    .unwrap()
                    .world()
            );
        }
        for name in [".xyz", ".xx", ".w", ".foo", "x", ".x y", ".WXYZ"] {
            assert!(PointFilter::parse(name).is_none(), "{name}");
        }
    }

    #[test]
    fn combines_sources_in_local_or_world_coordinates() {
        let plane = Frame3::try_from_directions(
            point(10.0, 20.0, 30.0),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut local = PointFilterSession::new(PointFilter::parse(".x").unwrap(), plane);
        assert_eq!(local.offer_point(point(1.0, 2.0, 3.0)).unwrap(), None);
        assert_eq!(
            local.preview_point(point(4.0, 5.0, 6.0)).unwrap(),
            point(4.0, 2.0, 6.0)
        );
        assert_eq!(
            local.offer_point(point(4.0, 5.0, 6.0)).unwrap(),
            Some(point(4.0, 2.0, 6.0))
        );
        let mut world = PointFilterSession::new(PointFilter::parse(".wx").unwrap(), plane);
        assert_eq!(world.offer_point(point(1.0, 2.0, 3.0)).unwrap(), None);
        assert_eq!(
            world.preview_point(point(4.0, 5.0, 6.0)).unwrap(),
            point(1.0, 5.0, 6.0)
        );
        assert_eq!(
            world.offer_point(point(4.0, 5.0, 6.0)).unwrap(),
            Some(point(1.0, 5.0, 6.0))
        );
    }
}
