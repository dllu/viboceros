//! Subdivision tori share the circle and guided-reference construction getters.
use crate::torus_input::{self, Kind};
pub use crate::torus_input::{Definition, Input, Preferences, from_arguments};
use crate::{Command, CommandContext, CommandError, CommandRegistry, remembered};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{GeometryError, SubD};
pub const USAGE: &str = Kind::SubD.usage();

impl Definition {
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
    torus_input::selection_prompt(Kind::SubD)
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
