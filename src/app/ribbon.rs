//! A curve pick, option editing and one final side point; drafts stay off-model.
use super::*;
use viboceros_command::ribbon::{self as solver, Options};
use viboceros_document::Object;
mod input;

pub(super) struct Session {
    source: Object,
    selection: Vec<ObjectId>,
    pub(super) postselected: bool,
    plane: Frame3,
    tolerance: Tolerance,
    layer: viboceros_document::LayerId,
    units: viboceros_geometry::LengthUnitSystem,
    options: Options,
    menu: Option<&'static str>,
    distance_start: Option<Point3>,
}
impl Session {
    pub(super) fn hint(&self) -> &'static str {
        match self.menu {
            Some("Distance") if self.distance_start.is_some() => "Second point for ribbon distance",
            Some("Distance") => {
                "Positive distance or first distance point; Enter accepts the default"
            }
            Some("Tolerance") => "Positive tolerance, or Default / 0 for document tolerance",
            Some("Corner") => "Sharp, Round, Smooth, Chamfer or None",
            Some("OutputLayer") => "Current or Input",
            _ => {
                "Side to ribbon; Distance, Loose, Corner, BothSides, InCPlane, OutputLayer; Esc cancels"
            }
        }
    }
    fn current(&self, doc: &Document) -> bool {
        doc.tolerance() == self.tolerance
            && doc.current_layer_id() == self.layer
            && doc.units() == &self.units
            && doc.selected_object_ids().eq(self.selection.iter().copied())
            && doc.object(self.source.id()).is_some_and(|o| {
                o.geometry_snapshot()
                    .shares_storage_with(self.source.geometry_snapshot())
                    && o.attributes() == self.source.attributes()
                    && o.group_ids() == self.source.group_ids()
            })
    }
}

impl VibocerosApp {
    pub(super) fn start_ribbon(&mut self, input: &str, postselected: bool) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Ribbon")
        }) {
            return false;
        }
        let plane = self.viewports[self.active_viewport].construction_plane();
        let context = viboceros_command::CommandContext {
            construction_plane: plane,
        };
        let parsed = match solver::parse(
            &words[1..],
            self.commands.ribbon_options(),
            self.document.units(),
            context,
        ) {
            Ok(p) => p,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        if parsed.side.is_some() || self.document.selected_object_count() == 0 {
            return false;
        }
        let sources = self
            .document
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .cloned()
            .collect::<Vec<_>>();
        if sources.len() != 1 {
            self.push_log("Select one curve for Ribbon".into());
            return true;
        }
        self.cancel_interactive_command(false);
        self.ribbon_session = Some(Session {
            source: sources[0].clone(),
            selection: self.document.selected_object_ids().collect(),
            postselected,
            plane,
            tolerance: self.document.tolerance(),
            layer: self.document.current_layer_id(),
            units: self.document.units().clone(),
            options: parsed.options,
            menu: None,
            distance_start: None,
        });
        self.active_command = Some(InteractiveCommand::Ribbon { anchor: None });
        self.drafting_plane = Some(plane);
        self.command_input.clear();
        self.ribbon_prompt();
        true
    }
    fn ribbon_prompt(&mut self) {
        if let Some(s) = &self.ribbon_session {
            self.push_log(format!("Ribbon: {}; {}", s.options.arguments(), s.hint()));
        }
    }
    fn validate_ribbon(&mut self) -> bool {
        if self
            .ribbon_session
            .as_ref()
            .is_some_and(|s| s.current(&self.document))
        {
            return true;
        }
        self.cancel_interactive_command(false);
        self.push_log("Ribbon inputs changed; select the curve again".into());
        false
    }
    pub(super) fn accept_ribbon_point(&mut self, point: Point3) -> Option<bool> {
        self.ribbon_session.as_ref()?;
        if !self.validate_ribbon() {
            return Some(false);
        }
        let s = self.ribbon_session.as_mut().unwrap();
        if s.menu == Some("Distance") {
            if let Some(start) = s.distance_start {
                match start.distance_to(point) {
                    Ok(d) if d > 0. => {
                        s.options.distance = d;
                        s.menu = None;
                        s.distance_start = None;
                        self.active_command = Some(InteractiveCommand::Ribbon { anchor: None });
                    }
                    _ => {
                        self.push_log("Ribbon distance must be positive".into());
                        return Some(false);
                    }
                }
            } else {
                s.distance_start = Some(point);
                self.active_command = Some(InteractiveCommand::Ribbon {
                    anchor: Some(point),
                });
            }
            self.command_input.clear();
            self.ribbon_prompt();
            return Some(true);
        }
        if s.menu.is_some() {
            return Some(false);
        }
        let s = self.ribbon_session.take().unwrap();
        let input = format!(
            "Ribbon {} {}",
            s.options.arguments(),
            format_model_point(point)
        );
        let context = viboceros_command::CommandContext {
            construction_plane: s.plane,
        };
        let result = if s.postselected {
            self.commands
                .execute_postselected(&mut self.document, &input, context)
        } else {
            self.commands
                .execute_in_context(&mut self.document, &input, context)
        };
        match result {
            Ok(message) => {
                self.cancel_interactive_command(false);
                self.push_log(message);
                self.command_input.clear();
                Some(true)
            }
            Err(e) => {
                self.ribbon_session = Some(s);
                self.push_log(format!("Error: {e}"));
                Some(false)
            }
        }
    }
}
