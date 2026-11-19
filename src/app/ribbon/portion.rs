//! Hand off a temporary source from the shared SubCrv picker.
use super::*;
use crate::app::{object_selection::ObjectPromptPhase, subcurve_prompt::Temporary};

impl VibocerosApp {
    pub(in crate::app) fn start_ribbon_portion(&mut self) -> bool {
        if !self.object_prompt.as_ref().is_some_and(|pending| {
            pending.description.command == "Ribbon" && pending.phase == ObjectPromptPhase::Selecting
        }) {
            return false;
        }
        self.begin_temporary_subcurve_prompt(None, Temporary::Ribbon);
        true
    }

    pub(in crate::app) fn finish_ribbon_portion(
        &mut self,
        portion: Option<(CapturedCurve, Curve3)>,
    ) -> Result<(), viboceros_command::CommandError> {
        let Some((reference, curve)) = portion else {
            self.clear_temporary_subcurve_input();
            self.log_object_prompt();
            return Ok(());
        };
        let pending = self.object_prompt.as_ref().expect("Ribbon source prompt");
        let command = pending.command_override.as_deref().unwrap_or("Ribbon");
        let plane = self.viewports[self.active_viewport].construction_plane();
        let parsed = solver::parse(
            &command.split_whitespace().skip(1).collect::<Vec<_>>(),
            self.commands.ribbon_options(),
            self.document.units(),
            viboceros_command::CommandContext {
                construction_plane: plane,
            },
        )?;
        let source = self
            .document
            .object(reference.object_id())
            .filter(|_| reference.current(&self.document))
            .cloned()
            .ok_or(viboceros_command::CommandError::RibbonSourceUnavailable)?;
        let guide = curve.as_ref().to_nurbs()?;
        let portion = Portion {
            reference,
            curve,
            guide,
        };
        let postselected = pending.postselected;
        self.object_prompt = None;
        self.begin_ribbon_side(source, Some(portion), parsed, plane, postselected);
        if let Some(side) = parsed.side {
            self.accept_ribbon_point(side);
        }
        Ok(())
    }
}

impl Session {
    pub(in crate::app) fn portion_preview(&self, document: &Document) -> Option<&NurbsCurve> {
        self.portion
            .as_ref()
            .filter(|_| self.current(document))
            .map(|portion| &portion.guide)
    }
}
