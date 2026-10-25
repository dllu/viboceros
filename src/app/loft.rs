//! Ordered Loft selection and pole getters; geometry stays in the command layer.
use super::*;
use viboceros_command::loft::Options;
use viboceros_geometry::LoftStyle;

mod input;
mod selection;

#[derive(Clone, Copy, Debug)]
enum End {
    Start,
    Finish,
}

#[derive(Clone, Debug)]
struct State {
    sources: Vec<(ObjectId, viboceros_document::GeometrySnapshot)>,
    options: Options,
    finished_profiles: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Prompt {
    state: State,
    before: Vec<ObjectId>,
    undo: Vec<State>,
    picking: Option<End>,
    value: Option<&'static str>,
}

impl Prompt {
    pub(super) fn selecting(&self) -> bool {
        self.picking.is_none() && self.value.is_none() && !self.state.finished_profiles
    }

    pub(super) fn hint(&self) -> &'static str {
        match (self.picking, self.value) {
            (Some(End::Start), _) => "Pick the start point of the loft (Esc cancels)",
            (Some(End::Finish), _) => "Pick the end point of the loft (Esc cancels)",
            (_, Some(_)) => "Enter the option value (Esc cancels)",
            _ if self.state.finished_profiles => {
                "Enter creates the loft; Type/Closed/SplitAtTangents/Rebuild/Natural/Undo (Esc cancels)"
            }
            _ => {
                "Select curves in order; Point/Type/Closed/SplitAtTangents/Rebuild/Natural/Undo; Enter creates the loft"
            }
        }
    }
}

impl VibocerosApp {
    pub(super) fn start_loft(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Loft")
        }) {
            return false;
        }
        let mut arguments = &words[1..];
        let point_option = arguments.last().copied().filter(|s| {
            ["Point", "StartPoint", "EndPoint"]
                .iter()
                .any(|name| s.trim_start_matches('_').eq_ignore_ascii_case(name))
        });
        if point_option.is_some() {
            arguments = &arguments[..arguments.len() - 1];
        }
        let Ok(options) = Options::parse(arguments) else {
            return false;
        };
        let before = self.document.selected_object_ids().collect::<Vec<_>>();
        let sources = before
            .iter()
            .copied()
            .filter(|id| {
                self.document
                    .object(*id)
                    .is_some_and(|o| o.geometry().curve_ref().is_some())
            })
            .collect::<Vec<_>>();
        let required = if options.closed {
            3
        } else if options.ends.is_empty() {
            2
        } else {
            1
        };
        // Fully specified scripts retain their immediate execution behavior.
        if point_option.is_none()
            && !arguments.is_empty()
            && sources.len() >= required
            && sources.len() == before.len()
        {
            return false;
        }
        self.cancel_interactive_command(false);
        self.loft_prompt = Some(Prompt {
            state: State {
                sources: vec![],
                options,
                finished_profiles: false,
            },
            before,
            undo: vec![],
            picking: None,
            value: None,
        });
        self.select_loft_curves(sources, SelectionMode::Add);
        self.loft_prompt.as_mut().unwrap().undo.clear();
        if let Some(option) = point_option {
            self.begin_loft_point(option);
        }
        self.push_log(format!("> {input}"));
        self.log_loft();
        true
    }

    fn log_loft(&mut self) {
        if let Some(p) = &self.loft_prompt {
            self.push_log(format!(
                "Loft: {}. {}",
                p.hint(),
                p.state.options.arguments()
            ));
        }
    }

    fn finish_loft(&mut self) {
        let p = self.loft_prompt.as_ref().unwrap();
        let n = p.state.sources.len();
        let required = if p.state.options.closed {
            3
        } else if p.state.options.ends.is_empty() {
            2
        } else {
            1
        };
        if n < required {
            self.push_log(format!("Loft needs at least {required} curve section(s)"));
            return;
        }
        if p.state.sources.iter().any(|(id, snapshot)| {
            !self.document.is_object_selectable(*id)
                || !self
                    .document
                    .object(*id)
                    .is_some_and(|o| snapshot.shares_storage_with(o.geometry_snapshot()))
        }) {
            self.cancel_interactive_command(false);
            self.push_log("Loft sources changed; select them again".into());
            return;
        }
        self.sync_loft_selection();
        let p = self.loft_prompt.take().unwrap();
        let input = format!("Loft {}", p.state.options.arguments());
        if !self.try_execute_command(&input) {
            self.loft_prompt = Some(p);
            self.log_loft();
        }
    }

    pub(super) fn cancel_loft(&mut self, announce: bool) {
        if let Some(p) = self.loft_prompt.take() {
            self.restore_loft_selection(p.before);
            if announce {
                self.push_log("Loft canceled".into());
            }
        }
    }
}
