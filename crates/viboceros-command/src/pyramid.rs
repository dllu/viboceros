//! Pyramid base and height getters shared by the viewport and complete commands.
mod base;
mod command;
mod input;
#[cfg(test)]
mod tests;

pub use base::Base;
pub(super) use command::PyramidCommand;
pub use input::Input;

use crate::solid_axial::Direction;
use viboceros_geometry::{Brep, GeometryError, Point3, Tolerance};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum BaseMode {
    #[default]
    Inscribed,
    Circumscribed,
    Edge,
    Star,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub sides: usize,
    pub direction: Direction,
    pub solid: bool,
    pub height: f64,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            sides: 5,
            direction: Direction::Vertical,
            solid: true,
            height: 10.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    pub base: Base,
    pub apex: Point3,
    pub solid: bool,
}

impl Definition {
    pub fn brep(self, tolerance: Tolerance) -> Result<Brep, GeometryError> {
        Brep::try_pyramid_from_rim(
            self.base.frame,
            &self.base.rim()?,
            self.apex,
            self.solid,
            tolerance,
        )
    }
}
