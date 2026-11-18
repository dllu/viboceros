//! Shared base-circle and tube getters for solid, subdivision and mesh tori.
use crate::circle_input::{CircleDefinition, CircleInput, CircleSizeMode};
use crate::{CommandError, option_name_eq};
use viboceros_document::Document;
use viboceros_geometry::{
    Circle3, Frame3, GeometryError, MAX_MESH_TORUS_FACES, MeshTorusOptions, NurbsSurface, Point3,
    Tolerance, TriangleMesh,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Solid,
    SubD,
    Mesh,
}

impl Kind {
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Solid, Self::SubD, Self::Mesh]
            .into_iter()
            .find(|kind| option_name_eq(name, kind.name()))
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Solid => "Torus",
            Self::SubD => "SubDTorus",
            Self::Mesh => "MeshTorus",
        }
    }

    pub const fn fit_command(self) -> &'static str {
        match self {
            Self::Solid => "Torus FitPoints",
            Self::SubD => "SubDTorus FitPoints",
            Self::Mesh => "MeshTorus FitPoints",
        }
    }

    pub const fn usage(self) -> &'static str {
        match self {
            Self::Solid => {
                "Torus circle-input [Radius|Diameter FixInnerDimension=Yes|No] tube-size-or-point"
            }
            Self::SubD => {
                "SubDTorus [AroundFaces=n VerticalFaces=n] circle-input [Radius|Diameter FixInnerDimension=Yes|No] tube-size-or-point"
            }
            Self::Mesh => {
                "MeshTorus [AroundFaces=n VerticalFaces=n] circle-input [Radius|Diameter FixInnerDimension=Yes|No] tube-size-or-point"
            }
        }
    }

    pub const fn projects_base_snaps(self) -> bool {
        matches!(self, Self::Solid | Self::Mesh)
    }

    pub fn default_preferences(self) -> Preferences {
        match self {
            Self::Solid | Self::SubD => Preferences::default(),
            Self::Mesh => Preferences {
                around_faces: crate::DEFAULT_MESH_TORUS_FACE_COUNT,
                vertical_faces: crate::DEFAULT_MESH_TORUS_FACE_COUNT,
                tube_diameter: true,
                ..Preferences::default()
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub around_faces: usize,
    pub vertical_faces: usize,
    pub radius: f64,
    pub diameter: bool,
    pub tube_radius: f64,
    pub tube_diameter: bool,
    pub fix_inner_dimension: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            around_faces: 10,
            vertical_faces: 6,
            radius: 1.,
            diameter: false,
            tube_radius: 1.,
            tube_diameter: false,
            fix_inner_dimension: false,
        }
    }
}
impl Preferences {
    pub fn validate(self) -> Result<(), CommandError> {
        self.validate_for(Kind::SubD)
    }

    pub fn validate_for(self, kind: Kind) -> Result<(), CommandError> {
        let max_faces = match kind {
            Kind::Solid => return Ok(()),
            Kind::SubD => 100_000,
            Kind::Mesh => MAX_MESH_TORUS_FACES,
        };
        if self.around_faces < 3
            || self.vertical_faces < 3
            || self
                .around_faces
                .checked_mul(self.vertical_faces)
                .is_none_or(|n| n > max_faces)
        {
            return Err(CommandError::Usage(kind.usage()));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    pub circle: CircleDefinition,
    pub tube_radius: f64,
    pub fix_inner_dimension: bool,
}
impl Definition {
    pub fn major_radius(self) -> f64 {
        self.circle.radius
            + if self.fix_inner_dimension {
                self.tube_radius
            } else {
                0.
            }
    }
    pub fn surface(self) -> Result<NurbsSurface, GeometryError> {
        NurbsSurface::try_torus(self.circle.frame, self.major_radius(), self.tube_radius)
    }

    pub fn mesh(self, p: Preferences, tolerance: Tolerance) -> Result<TriangleMesh, GeometryError> {
        TriangleMesh::try_torus_grid(
            self.circle.frame,
            self.major_radius(),
            self.tube_radius,
            MeshTorusOptions {
                around_count: p.around_faces,
                vertical_count: p.vertical_faces,
            },
            tolerance,
        )
    }
}
mod arguments;
mod input;
mod tube;
pub use arguments::{from_arguments, parse};
pub use input::Input;

pub fn selection_prompt(kind: Kind) -> crate::ObjectSelectionPrompt {
    crate::ObjectSelectionPrompt {
        command: kind.fit_command(),
        filter: crate::ObjectSelectionFilter::Points,
        options: vec![],
        menus: vec![],
        choices: vec![],
        workflow: crate::ObjectSelectionWorkflow::OptionsDuringSelection,
    }
}
