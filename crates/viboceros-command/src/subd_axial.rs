//! Shared circle, axis and constructive history inputs for round SubD primitives.
use crate::circle_input::{CircleDefinition, CircleInput, CircleSizeMode};
use crate::{Command, CommandContext, CommandError, CommandRegistry, remembered};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{
    Circle3, Frame3, GeometryError, Point3, SubD, SubDCylinderCap, Tolerance,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Cylinder,
    Cone,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cylinder => "SubDCylinder",
            Self::Cone => "SubDCone",
        }
    }
    pub fn usage(self) -> &'static str {
        match self {
            Self::Cylinder => crate::subd_cylinder::USAGE,
            Self::Cone => crate::subd_cone::USAGE,
        }
    }
    pub fn defaults(self) -> Preferences {
        Preferences {
            height: if self == Self::Cone { 10. } else { 1. },
            ..Default::default()
        }
    }
    pub fn preferences(self, commands: &CommandRegistry) -> Preferences {
        match self {
            Self::Cylinder => commands.subd_cylinder_preferences(),
            Self::Cone => commands.subd_cone_preferences(),
        }
    }
    pub fn remember(self, commands: &CommandRegistry, p: Preferences) {
        match self {
            Self::Cylinder => commands.remember_subd_cylinder_preferences(p),
            Self::Cone => commands.remember_subd_cone_preferences(p),
        };
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Vertical,
    None,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub around_faces: usize,
    pub vertical_faces: usize,
    pub solid: bool,
    pub cap: SubDCylinderCap,
    pub direction: Direction,
    pub both_sides: bool,
    pub radius: f64,
    pub height: f64,
    pub diameter: bool,
    pub base_angle: f64,
    pub apex_angle: f64,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            around_faces: 8,
            vertical_faces: 4,
            solid: true,
            cap: SubDCylinderCap::Triangles,
            direction: Direction::Vertical,
            both_sides: false,
            radius: 1.,
            height: 1.,
            diameter: false,
            base_angle: 60.,
            apex_angle: 60.,
        }
    }
}
impl Preferences {
    pub fn validate(self) -> Result<(), CommandError> {
        self.validate_for(Kind::Cylinder)
    }
    pub fn validate_for(self, kind: Kind) -> Result<(), CommandError> {
        let n = self.around_faces;
        let h = self.vertical_faces;
        let cap = if self.solid {
            self.cap
        } else {
            SubDCylinderCap::None
        };
        let triangles = cap == SubDCylinderCap::Triangles
            || cap == SubDCylinderCap::Quads && n > 4 && !n.is_multiple_of(2);
        let center = triangles && n > 3 || cap == SubDCylinderCap::Quads && n > 4;
        let caps = if cap == SubDCylinderCap::None {
            Some(0)
        } else if triangles && n > 3 {
            n.checked_mul(2)
        } else if n <= 4 {
            Some(2)
        } else {
            Some(n)
        };
        let vertices = h
            .checked_add(1)
            .and_then(|h| n.checked_mul(h))
            .and_then(|n| n.checked_add(if center { 2 } else { 0 }));
        let faces = n
            .checked_mul(h)
            .and_then(|f| caps.and_then(|c| f.checked_add(c)));
        let (vertices, faces) = if kind == Kind::Cone {
            (
                h.checked_mul(n).and_then(|n| {
                    n.checked_add(
                        1 + usize::from(
                            cap != SubDCylinderCap::None
                                && !(self.around_faces == 3
                                    || self.around_faces == 4 && cap == SubDCylinderCap::Quads),
                        ),
                    )
                }),
                n.checked_mul(h).and_then(|wall| {
                    wall.checked_add(if cap == SubDCylinderCap::None {
                        0
                    } else if n == 3 || n == 4 && cap == SubDCylinderCap::Quads {
                        1
                    } else if cap == SubDCylinderCap::Quads && n.is_multiple_of(2) {
                        n / 2
                    } else {
                        n
                    })
                }),
            )
        } else {
            (vertices, faces)
        };
        if n < 3
            || h < 2
            || vertices.is_none_or(|v| v > 100_000)
            || faces.is_none_or(|v| v > 100_000)
            || self.cap == SubDCylinderCap::None
        {
            return Err(CommandError::Usage(match kind {
                Kind::Cylinder => {
                    "SubDCylinder requires at least 3 around faces, 2 vertical faces and at most 100000 control vertices/faces"
                }
                Kind::Cone => {
                    "SubDCone requires at least 3 around faces, 2 vertical faces and at most 100000 control vertices/faces"
                }
            }));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    pub circle: CircleDefinition,
    pub kind: Kind,
    pub frame: Frame3,
    pub height: f64,
}
impl Definition {
    pub fn geometry(self, p: Preferences) -> Result<Geometry, CommandError> {
        p.validate_for(self.kind)?;
        let build = if self.kind == Kind::Cylinder {
            SubD::try_cylinder
        } else {
            SubD::try_cone
        };
        Ok(Geometry::SubD(build(
            self.frame,
            self.circle.radius,
            self.height,
            [p.around_faces, p.vertical_faces],
            if p.solid {
                p.cap
            } else {
                SubDCylinderCap::None
            },
        )?))
    }
}
mod adapter;
mod arguments;
mod input;
pub(super) use adapter::AxialCommand;
pub use adapter::selection_prompt_for;
pub use arguments::from_arguments_for;
pub use input::Input;
