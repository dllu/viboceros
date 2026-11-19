//! A curve pick, option editing and one final side point; drafts stay off-model.
use super::*;
use viboceros_command::curve_reference::CapturedCurve;
use viboceros_command::ribbon::{self as solver, Options};
use viboceros_document::Object;
use viboceros_geometry::{Curve3, NurbsCurve};
mod input;
mod portion;
mod preview;
pub(super) use input::Getter;

struct Portion {
    reference: CapturedCurve,
    curve: Curve3,
    guide: NurbsCurve,
}

pub(super) struct Session {
    source: Object,
    portion: Option<Portion>,
    selection: Vec<ObjectId>,
    pub(super) postselected: bool,
    plane: Frame3,
    tolerance: Tolerance,
    layer: viboceros_document::LayerId,
    units: viboceros_geometry::LengthUnitSystem,
    options: Options,
    menu: Option<Getter>,
    through_point: bool,
    preview: preview::Preview,
}
impl Session {
    pub(super) fn hint(&self) -> &'static str {
        match self.menu {
            Some(getter) => getter.hint(),
            _ if self.through_point => "Through point; pick or type the offset location",
            _ if self.options.loose => {
                "Side to ribbon; Distance, ThroughPoint, Loose, BothSides, InCPlane, OutputLayer"
            }
            _ => {
                "Side to ribbon; Distance, ThroughPoint, Loose, Corner, Trim, BothSides, InCPlane, OutputLayer"
            }
        }
    }
    fn current(&self, doc: &Document) -> bool {
        doc.tolerance() == self.tolerance
            && doc.current_layer_id() == self.layer
            && doc.units() == &self.units
            && doc.selected_object_ids().eq(self.selection.iter().copied())
            && self
                .portion
                .as_ref()
                .is_none_or(|p| p.reference.current(doc))
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
        let use_portion = words[1..].iter().any(|word| is_subcurve_option(word));
        let args = words[1..]
            .iter()
            .copied()
            .filter(|word| !is_subcurve_option(word))
            .collect::<Vec<_>>();
        let plane = self.viewports[self.active_viewport].construction_plane();
        let context = viboceros_command::CommandContext {
            construction_plane: plane,
        };
        let parsed = match solver::parse(
            &args,
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
        if parsed.side.is_none() || (use_portion && self.document.selected_object_count() == 0) {
            self.commands.remember_ribbon_options(parsed.options);
        }
        if self.document.selected_object_count() == 0 {
            if use_portion {
                if self.try_start_object_prompt(&ribbon_command_line(parsed)) {
                    self.start_ribbon_portion();
                }
                return true;
            }
            return false;
        }
        if use_portion {
            self.push_log("SubCrv is available while selecting the Ribbon source".into());
        }
        if parsed.side.is_some() && !use_portion {
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
        self.begin_ribbon_side(sources[0].clone(), None, parsed, plane, postselected);
        if let Some(side) = parsed.side {
            self.accept_ribbon_point(side);
        }
        true
    }

    fn begin_ribbon_side(
        &mut self,
        source: Object,
        portion: Option<Portion>,
        parsed: solver::Input,
        plane: Frame3,
        postselected: bool,
    ) {
        self.cancel_interactive_command(false);
        self.ribbon_session = Some(Session {
            source,
            portion,
            selection: self.document.selected_object_ids().collect(),
            postselected,
            plane,
            tolerance: self.document.tolerance(),
            layer: self.document.current_layer_id(),
            units: self.document.units().clone(),
            options: parsed.options,
            menu: None,
            through_point: parsed.through_point,
            preview: preview::Preview::default(),
        });
        self.active_command = Some(InteractiveCommand::Ribbon { anchor: None });
        self.drafting_plane = Some(plane);
        self.command_input.clear();
        self.ribbon_prompt();
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
        if let Some(getter) = self.ribbon_source_getter() {
            return Some(self.accept_ribbon_distance(getter, point));
        }
        self.ribbon_session.as_ref()?;
        if !self.validate_ribbon() {
            return Some(false);
        }
        if let Some(getter) = self.ribbon_session.as_ref().unwrap().menu {
            return Some(self.accept_ribbon_distance(getter, point));
        }
        let s = self.ribbon_session.take().unwrap();
        let context = viboceros_command::CommandContext {
            construction_plane: s.plane,
        };
        let result = if let Some(portion) = &s.portion {
            self.commands.commit_ribbon_portion(
                &mut self.document,
                &portion.reference,
                portion.curve.as_ref(),
                solver::Input {
                    options: s.options,
                    side: Some(point),
                    through_point: s.through_point,
                },
                context,
            )
        } else {
            let input = ribbon_command_line(solver::Input {
                options: s.options,
                side: Some(point),
                through_point: s.through_point,
            });
            if s.postselected {
                self.commands
                    .execute_postselected(&mut self.document, &input, context)
            } else {
                self.commands
                    .execute_in_context(&mut self.document, &input, context)
            }
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

fn ribbon_command_line(input: solver::Input) -> String {
    let mut command = format!("Ribbon {}", input.options.arguments());
    if input.through_point {
        command.push_str(" ThroughPoint");
    }
    if let Some(point) = input.side {
        command.push(' ');
        command.push_str(&format_model_point(point));
    }
    command
}

fn is_subcurve_option(word: &str) -> bool {
    word.trim_start_matches(['_', '-'])
        .eq_ignore_ascii_case("SubCrv")
}
