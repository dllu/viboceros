//! Per-command preference ownership and transactional model admission.
use super::*;
pub(crate) struct AxialCommand {
    kind: Kind,
    preferences: remembered::Remembered<Preferences>,
}
impl AxialCommand {
    pub(crate) fn new(kind: Kind) -> Self {
        Self {
            kind,
            preferences: remembered::Remembered::new(kind.defaults()),
        }
    }
}
impl Default for AxialCommand {
    fn default() -> Self {
        Self::new(Kind::Cylinder)
    }
}
impl Command for AxialCommand {
    fn name(&self) -> &'static str {
        self.kind.name()
    }
    fn subd_cylinder_preferences(&self) -> Option<Preferences> {
        (self.kind == Kind::Cylinder).then(|| self.preferences.get())
    }
    fn remember_subd_cylinder_preferences(&self, p: Preferences) -> bool {
        if self.kind != Kind::Cylinder {
            return false;
        }
        self.preferences.set(p);
        true
    }
    fn subd_cone_preferences(&self) -> Option<Preferences> {
        (self.kind == Kind::Cone).then(|| self.preferences.get())
    }
    fn remember_subd_cone_preferences(&self, p: Preferences) -> bool {
        if self.kind != Kind::Cone {
            return false;
        }
        self.preferences.set(p);
        true
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<crate::ObjectSelectionPrompt>, CommandError> {
        Ok(matches!(args, [s] if s.trim_start_matches(['_', '-']).eq_ignore_ascii_case("FitPoints")).then(||selection_prompt_for(self.kind)))
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let result = self.run_in_context(doc, args, context)?;
        let selection = doc.selected_object_ids().collect::<Vec<_>>();
        doc.release_command_selection_on_history_replay(selection)?;
        doc.clear_selection();
        Ok(result)
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let input = from_arguments_for(
            self.kind,
            doc,
            args,
            context.construction_plane,
            self.preferences.get(),
        )?;
        let id = doc.add_geometry(input.definition.unwrap().geometry(input.preferences)?)?;
        self.preferences.set(input.preferences);
        Ok(format!("Added {} {id}", self.kind.name()))
    }
}
pub fn selection_prompt_for(kind: Kind) -> crate::ObjectSelectionPrompt {
    crate::ObjectSelectionPrompt {
        command: match kind {
            Kind::Cylinder => "SubDCylinder FitPoints",
            Kind::Cone => "SubDCone FitPoints",
        },
        filter: crate::ObjectSelectionFilter::Points,
        options: vec![],
        menus: vec![],
        choices: vec![],
        workflow: crate::ObjectSelectionWorkflow::OptionsDuringSelection,
    }
}
