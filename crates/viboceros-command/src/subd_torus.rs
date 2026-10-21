//! Subdivision tori share the circle and guided-reference construction getters.
use crate::circle_input::{CircleDefinition, CircleInput, CircleSizeMode};
use crate::{Command, CommandContext, CommandError, CommandRegistry, remembered};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Circle3, Frame3, GeometryError, Point3, SubD};
pub const USAGE: &str = "SubDTorus [AroundFaces=n VerticalFaces=n] circle-input [Radius|Diameter FixInnerDimension=Yes|No] tube-size-or-point";
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
        if self.around_faces < 3
            || self.vertical_faces < 3
            || self
                .around_faces
                .checked_mul(self.vertical_faces)
                .is_none_or(|n| n > 100_000)
        {
            return Err(CommandError::Usage(
                "SubDTorus requires at least three faces in each direction and at most 100000 control vertices/faces",
            ));
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
    pub fn geometry(self, p: Preferences) -> Result<Geometry, CommandError> {
        p.validate()?;
        if self.circle.radius <= 0. {
            return Err(GeometryError::Degenerate {
                context: "SubDTorus fitted base circle",
            }
            .into());
        }
        Ok(Geometry::SubD(SubD::try_torus(
            self.circle.frame,
            self.major_radius(),
            self.tube_radius,
            [p.around_faces, p.vertical_faces],
        )?))
    }
}
mod arguments;
mod input;
mod tube;
pub use arguments::from_arguments;
pub use input::Input;
#[derive(Default)]
pub(super) struct SubDTorusCommand(remembered::Remembered<Preferences>);
impl Command for SubDTorusCommand {
    fn name(&self) -> &'static str {
        "SubDTorus"
    }
    fn subd_torus_preferences(&self) -> Option<Preferences> {
        Some(self.0.get())
    }
    fn remember_subd_torus_preferences(&self, p: Preferences) -> bool {
        self.0.set(p);
        true
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<crate::ObjectSelectionPrompt>, CommandError> {
        Ok(
            matches!(args,[s]if s.trim_start_matches(['_','-']).eq_ignore_ascii_case("FitPoints"))
                .then(selection_prompt),
        )
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let result = self.run_in_context(doc, args, context)?;
        let selected = doc.selected_object_ids().collect::<Vec<_>>();
        doc.release_command_selection_on_history_replay(selected)?;
        doc.clear_selection();
        Ok(result)
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let input = from_arguments(doc, args, context.construction_plane, self.0.get())?;
        let id = doc.add_geometry(input.definition.unwrap().geometry(input.preferences)?)?;
        self.0.set(input.preferences);
        Ok(format!("Added SubDTorus {id}"))
    }
}
pub fn selection_prompt() -> crate::ObjectSelectionPrompt {
    crate::ObjectSelectionPrompt {
        command: "SubDTorus FitPoints",
        filter: crate::ObjectSelectionFilter::Points,
        options: vec![],
        menus: vec![],
        choices: vec![],
        workflow: crate::ObjectSelectionWorkflow::OptionsDuringSelection,
    }
}
impl CommandRegistry {
    pub fn subd_torus_preferences(&self) -> Preferences {
        self.lookup
            .get(&crate::normalize_command_name("SubDTorus"))
            .and_then(|i| self.commands[*i].subd_torus_preferences())
            .unwrap_or_default()
    }
    pub fn remember_subd_torus_preferences(&self, p: Preferences) -> bool {
        self.lookup
            .get(&crate::normalize_command_name("SubDTorus"))
            .is_some_and(|i| self.commands[*i].remember_subd_torus_preferences(p))
    }
}
