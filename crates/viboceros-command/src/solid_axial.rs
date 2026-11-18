//! Shared construction of circular solids from a base circle and an axis.
use crate::{CommandError, CommandRegistry, option_name_eq};
use viboceros_document::Geometry;
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Tolerance};

mod arguments;
mod command;
mod end_circle;
mod input;
mod mesh;
mod mesh_topology;
pub(super) use command::run;
pub(super) use command::{ConeCommand, TruncatedConeCommand};
pub(super) use mesh::MeshCommand;

pub use arguments::from_arguments;
pub use input::Input;
pub use mesh_topology::MeshTopology;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shape {
    Cone,
    Cylinder,
    TruncatedCone,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Cone,
    Cylinder,
    TruncatedCone,
    MeshCone,
    MeshCylinder,
    MeshTruncatedCone,
}

impl Kind {
    pub fn from_name(name: &str) -> Option<Self> {
        if option_name_eq(name, "Cyl") {
            return Some(Self::Cylinder);
        }
        [
            Self::Cone,
            Self::Cylinder,
            Self::TruncatedCone,
            Self::MeshCone,
            Self::MeshCylinder,
            Self::MeshTruncatedCone,
        ]
        .into_iter()
        .find(|kind| option_name_eq(name, kind.name()))
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Cone => "Cone",
            Self::Cylinder => "Cylinder",
            Self::TruncatedCone => "TruncatedCone",
            Self::MeshCone => "MeshCone",
            Self::MeshCylinder => "MeshCylinder",
            Self::MeshTruncatedCone => "MeshTruncatedCone",
        }
    }

    pub const fn noun(self) -> &'static str {
        match self.shape() {
            Shape::Cone => "cone",
            Shape::Cylinder => "cylinder",
            Shape::TruncatedCone => "truncated cone",
        }
    }

    pub const fn shape(self) -> Shape {
        match self {
            Self::Cone | Self::MeshCone => Shape::Cone,
            Self::Cylinder | Self::MeshCylinder => Shape::Cylinder,
            Self::TruncatedCone | Self::MeshTruncatedCone => Shape::TruncatedCone,
        }
    }

    pub const fn is_mesh(self) -> bool {
        matches!(
            self,
            Self::MeshCone | Self::MeshCylinder | Self::MeshTruncatedCone
        )
    }

    pub const fn fit_command(self) -> &'static str {
        match self {
            Self::Cone => "Cone FitPoints",
            Self::Cylinder => "Cylinder FitPoints",
            Self::TruncatedCone => "TruncatedCone FitPoints",
            Self::MeshCone => "MeshCone FitPoints",
            Self::MeshCylinder => "MeshCylinder FitPoints",
            Self::MeshTruncatedCone => "MeshTruncatedCone FitPoints",
        }
    }

    pub const fn usage(self) -> &'static str {
        match self {
            Self::Cone => {
                "Cone [DirectionConstraint=None|Vertical|AroundCurve Solid=Yes|No] base-circle height-or-apex"
            }
            Self::Cylinder => {
                "Cylinder [DirectionConstraint=None|Vertical|AroundCurve Solid=Yes|No] base-circle [BothSides=Yes|No] height-or-end"
            }
            Self::TruncatedCone => {
                "TruncatedCone [DirectionConstraint=None|Vertical|AroundCurve Solid=Yes|No] base-circle height-or-end [Radius|Diameter] top-size"
            }
            Self::MeshCone => crate::MESH_CONE_USAGE,
            Self::MeshCylinder => crate::MESH_CYLINDER_USAGE,
            Self::MeshTruncatedCone => crate::MESH_TRUNCATED_CONE_USAGE,
        }
    }

    pub fn preferences(self, registry: &CommandRegistry) -> Preferences {
        match self {
            Self::Cone => registry.solid_cone_preferences.get().0,
            Self::Cylinder => registry.solid_cylinder_preferences.get().0,
            Self::TruncatedCone => registry.truncated_cone_preferences.get().0,
            mesh => {
                registry.mesh_axial_preferences[mesh.shape() as usize]
                    .get()
                    .0
            }
        }
    }

    pub fn initial_preferences(self, registry: &CommandRegistry) -> Preferences {
        let preferences = self.preferences(registry);
        if self.is_mesh() {
            preferences.for_mesh_start()
        } else {
            preferences
        }
    }

    pub fn remember(self, registry: &CommandRegistry, preferences: Preferences) {
        match self {
            Self::Cone => registry
                .solid_cone_preferences
                .set(ConeSettings(preferences)),
            Self::Cylinder => registry
                .solid_cylinder_preferences
                .set(CylinderSettings(preferences)),
            Self::TruncatedCone => registry
                .truncated_cone_preferences
                .set(TruncatedConeSettings(preferences)),
            mesh => registry.mesh_axial_preferences[mesh.shape() as usize]
                .set(MeshSettings(preferences)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Direction {
    None,
    #[default]
    Vertical,
    AroundCurve,
}

impl Direction {
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Vertical => "Vertical",
            Self::AroundCurve => "AroundCurve",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        [Self::None, Self::Vertical, Self::AroundCurve]
            .into_iter()
            .find(|choice| option_name_eq(value, choice.label()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub direction: Direction,
    pub solid: bool,
    pub both_sides: bool,
    pub radius: f64,
    pub diameter: bool,
    pub height: f64,
    pub end_size: f64,
    pub topology: MeshTopology,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            direction: Direction::Vertical,
            solid: true,
            both_sides: false,
            radius: 1.,
            diameter: false,
            height: 1.,
            end_size: 0.,
            topology: MeshTopology::default(),
        }
    }
}

impl Preferences {
    fn for_mesh_start(mut self) -> Self {
        if self.direction == Direction::None {
            self.direction = Direction::Vertical;
        }
        self.both_sides = false;
        self
    }
}

#[derive(Clone, Copy)]
pub(super) struct ConeSettings(pub(super) Preferences);
#[derive(Clone, Copy)]
pub(super) struct CylinderSettings(pub(super) Preferences);
#[derive(Clone, Copy)]
pub(super) struct TruncatedConeSettings(pub(super) Preferences);
#[derive(Clone, Copy)]
pub(super) struct MeshSettings(pub(super) Preferences);

impl Default for MeshSettings {
    fn default() -> Self {
        Self(Preferences {
            end_size: 1.,
            ..Default::default()
        })
    }
}

impl Default for ConeSettings {
    fn default() -> Self {
        Self(Preferences {
            height: 10.,
            ..Default::default()
        })
    }
}

impl Default for CylinderSettings {
    fn default() -> Self {
        Self(Preferences {
            height: 5.,
            radius: 5.,
            ..Default::default()
        })
    }
}

impl Default for TruncatedConeSettings {
    fn default() -> Self {
        Self(Preferences {
            height: 0.,
            ..Default::default()
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    pub kind: Kind,
    pub frame: Frame3,
    pub radius: f64,
    pub height: f64,
    pub solid: bool,
    pub both_sides: bool,
    pub end_radius: f64,
    pub topology: MeshTopology,
}

impl Definition {
    pub fn height_range(self) -> [f64; 2] {
        if self.both_sides {
            [-self.height, self.height]
        } else {
            [0., self.height]
        }
    }

    fn apex_frame(self) -> Result<Frame3, viboceros_geometry::GeometryError> {
        let apex = self.frame.point_at([0., 0., self.height])?;
        let y = if self.height > 0. {
            self.frame.y_axis().opposite()
        } else {
            self.frame.y_axis()
        };
        Frame3::try_from_unit_directions(
            apex,
            self.frame.x_axis(),
            y,
            Tolerance::NUMERICAL_VALIDATION,
        )
    }

    fn frustum_frame(self) -> Result<Frame3, viboceros_geometry::GeometryError> {
        let normal = if self.height > 0.0 {
            self.frame.z_axis()
        } else {
            self.frame.z_axis().opposite()
        };
        Frame3::try_from_unit_x_and_normal(
            self.frame.origin(),
            self.frame.x_axis(),
            normal,
            Tolerance::NUMERICAL_VALIDATION,
        )
    }

    fn cylinder_path(self) -> Result<(Frame3, f64), viboceros_geometry::GeometryError> {
        let [start, end] = self.height_range();
        let origin = self.frame.point_at([0., 0., start])?;
        let y = if end < start {
            self.frame.y_axis().opposite()
        } else {
            self.frame.y_axis()
        };
        let frame = Frame3::try_from_unit_directions(
            origin,
            self.frame.x_axis(),
            y,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        Ok((frame, (end - start).abs()))
    }

    pub fn wall(self) -> Result<NurbsSurface, viboceros_geometry::GeometryError> {
        match self.kind.shape() {
            Shape::Cone => {
                NurbsSurface::try_cone_from_apex(self.apex_frame()?, self.radius, self.height.abs())
            }
            Shape::TruncatedCone => NurbsSurface::try_truncated_cone_wall(
                self.frustum_frame()?,
                [self.radius, self.end_radius],
                self.height.abs(),
            ),
            Shape::Cylinder => {
                let (frame, height) = self.cylinder_path()?;
                let wall = NurbsSurface::try_cylinder(frame, self.radius, 0., height)?;
                wall.try_reparameterized(0.0..=std::f64::consts::TAU * self.radius, 0.0..=height)
            }
        }
    }

    pub fn geometry(
        self,
        use_extrusions: bool,
        tolerance: Tolerance,
    ) -> Result<Geometry, CommandError> {
        if self.kind.is_mesh() {
            return Ok(Geometry::Mesh(self.mesh(tolerance)?));
        }
        match self.kind.shape() {
            Shape::Cylinder => {
                let brep = Brep::try_cylinder_extrusion(
                    self.frame,
                    self.radius,
                    self.height_range(),
                    self.solid,
                    tolerance,
                )?;
                Ok(Geometry::Brep(if use_extrusions {
                    brep
                } else {
                    brep.try_extrusion_brep_form(tolerance)?
                }))
            }
            Shape::Cone => Ok(Geometry::Brep(Brep::try_cone_from_apex(
                self.apex_frame()?,
                self.radius,
                self.height.abs(),
                self.solid,
                tolerance,
            )?)),
            Shape::TruncatedCone => Ok(Geometry::Brep(Brep::try_truncated_cone_with_caps(
                self.frustum_frame()?,
                [self.radius, self.end_radius],
                self.height.abs(),
                self.solid,
                tolerance,
            )?)),
        }
    }
}

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

#[cfg(test)]
mod mesh_tests;
#[cfg(test)]
mod tests;
