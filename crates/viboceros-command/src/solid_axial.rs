//! Shared construction of circular solids from a base circle and an axis.
use crate::{CommandError, CommandRegistry, option_name_eq};
use viboceros_document::Geometry;
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Tolerance};

mod arguments;
mod command;
mod end_circle;
mod input;
pub(super) use command::run;
pub(super) use command::{ConeCommand, TruncatedConeCommand};

pub use arguments::from_arguments;
pub use input::Input;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Cone,
    Cylinder,
    TruncatedCone,
}

impl Kind {
    pub fn from_name(name: &str) -> Option<Self> {
        if option_name_eq(name, "Cone") {
            Some(Self::Cone)
        } else if option_name_eq(name, "Cylinder") || option_name_eq(name, "Cyl") {
            Some(Self::Cylinder)
        } else if option_name_eq(name, "TruncatedCone") {
            Some(Self::TruncatedCone)
        } else {
            None
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Cone => "Cone",
            Self::Cylinder => "Cylinder",
            Self::TruncatedCone => "TruncatedCone",
        }
    }

    pub const fn noun(self) -> &'static str {
        match self {
            Self::Cone => "cone",
            Self::Cylinder => "cylinder",
            Self::TruncatedCone => "truncated cone",
        }
    }

    pub const fn fit_command(self) -> &'static str {
        match self {
            Self::Cone => "Cone FitPoints",
            Self::Cylinder => "Cylinder FitPoints",
            Self::TruncatedCone => "TruncatedCone FitPoints",
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
        }
    }

    pub fn preferences(self, registry: &CommandRegistry) -> Preferences {
        match self {
            Self::Cone => registry.solid_cone_preferences.get().0,
            Self::Cylinder => registry.solid_cylinder_preferences.get().0,
            Self::TruncatedCone => registry.truncated_cone_preferences.get().0,
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
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ConeSettings(pub(super) Preferences);
#[derive(Clone, Copy)]
pub(super) struct CylinderSettings(pub(super) Preferences);
#[derive(Clone, Copy)]
pub(super) struct TruncatedConeSettings(pub(super) Preferences);

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

    pub fn wall(self) -> Result<NurbsSurface, viboceros_geometry::GeometryError> {
        match self.kind {
            Kind::Cone => {
                NurbsSurface::try_cone_from_apex(self.apex_frame()?, self.radius, self.height.abs())
            }
            Kind::TruncatedCone => NurbsSurface::try_truncated_cone_wall(
                self.frustum_frame()?,
                [self.radius, self.end_radius],
                self.height.abs(),
            ),
            Kind::Cylinder => {
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
                let height = (end - start).abs();
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
        match self.kind {
            Kind::Cylinder => {
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
            Kind::Cone => Ok(Geometry::Brep(Brep::try_cone_from_apex(
                self.apex_frame()?,
                self.radius,
                self.height.abs(),
                self.solid,
                tolerance,
            )?)),
            Kind::TruncatedCone => Ok(Geometry::Brep(Brep::try_truncated_cone_with_caps(
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
mod tests;
