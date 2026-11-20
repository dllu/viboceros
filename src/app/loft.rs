//! Ordered Loft selection and pole getters; geometry stays in the command layer.
use super::*;
use viboceros_command::{CommandError, curve_reference::CapturedCurve, loft::Options};
use viboceros_geometry::LoftStyle;

use super::curve_seam::Editor;
use crate::viewport::curve_seam::{Event, Marker};

mod input;
mod pole_preview;
mod preview;
mod selection;
mod simplification;

pub(super) use simplification::Values as SimplificationValues;

#[derive(Clone, Copy, Debug)]
enum End {
    Start,
    Finish,
}

type SectionKey = (ObjectId, Option<usize>);

#[derive(Clone, Debug)]
struct Section {
    source: CapturedCurve,
    reversed: Option<bool>,
}

fn section_key(source: &CapturedCurve) -> SectionKey {
    (source.object_id(), source.source().1)
}

#[derive(Clone, Debug)]
struct State {
    sources: Vec<Section>,
    options: Options,
    finished_profiles: bool,
}

#[derive(Debug)]
struct Seams {
    editor: Editor,
    oriented: bool,
}

#[derive(Debug)]
pub(super) struct Prompt {
    state: State,
    before: Vec<ObjectId>,
    undo: Vec<State>,
    picking: Option<End>,
    pole_preview: Option<pole_preview::Preview>,
    value: Option<&'static str>,
    scripted: bool,
    seams: Option<Seams>,
    review: Option<preview::Review>,
    seam_id_pending: bool,
    profile_id_pending: bool,
}

impl Prompt {
    pub(super) fn selecting(&self) -> bool {
        self.picking.is_none()
            && self.value.is_none()
            && !self.state.finished_profiles
            && self.seams.is_none()
            && self.review.is_none()
    }

    pub(super) fn hint(&self) -> &'static str {
        match (self.picking, self.value) {
            (Some(End::Start), _) => "Pick the start point of the loft (Esc cancels)",
            (Some(End::Finish), _) => "Pick the end point of the loft (Esc cancels)",
            (_, Some(_)) => "Enter the option value (Esc cancels)",
            _ if self.profile_id_pending || self.seam_id_pending => {
                "Enter the section curve object ID"
            }
            _ if self.review.as_ref().is_some_and(|review| review.aligning) => {
                "Click a section end to reverse it. Press Enter to return to the preview"
            }
            _ if self.review.is_some() => {
                "Choose loft options (Type Simplify RebuildCount RefitTolerance Closed SplitAtTangents AlignCurves). Press Enter to create the loft"
            }
            _ if self.seams.is_some() => self.seams.as_ref().unwrap().editor.hint(),
            _ => {
                "Select curves or surface edges in order, or use Point for a pole. Press Enter to continue"
            }
        }
    }

    pub(super) fn edge_highlights(&self) -> impl Iterator<Item = crate::viewport::EdgePick> + '_ {
        self.state.sources.iter().filter_map(|section| {
            let (object, edge) = section_key(&section.source);
            Some(crate::viewport::EdgePick {
                object,
                edge: edge?,
            })
        })
    }

    fn uses_script(&self) -> bool {
        self.scripted && self.edge_highlights().next().is_none()
    }

    pub(super) fn seam_markers(&self) -> Vec<Marker> {
        if let Some(review) = &self.review {
            return review.alignment_markers();
        }
        self.seams
            .as_ref()
            .map_or_else(Vec::new, |seams| seams.editor.markers())
    }

    pub(super) fn seam_overlay<'a>(
        &'a self,
        markers: &'a [Marker],
        viewport: usize,
    ) -> Option<crate::viewport::curve_seam::Overlay<'a>> {
        if let Some(review) = &self.review {
            return review.alignment_overlay(markers);
        }
        Some(self.seams.as_ref()?.editor.overlay(markers, viewport))
    }

    pub(super) fn preview_wires(&self) -> &[viboceros_geometry::NurbsCurve] {
        if let Some(preview) = &self.pole_preview {
            return preview.wires();
        }
        self.review
            .as_ref()
            .and_then(|review| review.draft.as_ref().ok())
            .map_or(&[], |draft| draft.wires.as_slice())
    }

    fn required(&self) -> usize {
        required_sections(self.state.options)
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
        let parsed = if arguments.is_empty() {
            Ok(self.loft_options)
        } else {
            Options::parse(arguments)
        };
        let Ok(mut options) = parsed else {
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
        let edges = self
            .component_selection
            .valid_picks(&self.document)
            .into_iter()
            .filter(|pick| pick.kind == viboceros_command::ComponentSelectionKind::BrepEdge)
            .map(|pick| (pick.object, Some(pick.index)))
            .collect::<Vec<_>>();
        let source_count = sources.len() + edges.len();
        if arguments.is_empty() && (source_count < 3 || point_option.is_some()) {
            options.closed = false;
        }
        let required = required_sections(options);
        let scripted = !arguments.is_empty() && point_option.is_none();
        // Fully specified scripts retain their immediate execution behavior.
        if scripted
            && edges.is_empty()
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
            pole_preview: None,
            value: None,
            scripted,
            seams: None,
            review: None,
            seam_id_pending: false,
            profile_id_pending: false,
        });
        self.apply_loft_picks(
            sources
                .into_iter()
                .map(|id| (id, None))
                .chain(edges)
                .map(|key| (key, None)),
            SelectionMode::Add,
        );
        self.loft_prompt.as_mut().unwrap().undo.clear();
        if let Some(option) = point_option {
            self.begin_loft_point(option);
        }
        self.push_log(format!("> {input}"));
        if point_option.is_none()
            && self
                .loft_prompt
                .as_ref()
                .is_some_and(|prompt| !prompt.uses_script() && sources_ready(prompt))
        {
            self.finish_loft();
        } else {
            self.log_loft();
        }
        true
    }

    fn remember_loft_options(&mut self) {
        let prompt = self.loft_prompt.as_ref().unwrap();
        if !prompt.uses_script() {
            self.loft_options = Options {
                ends: Default::default(),
                natural_seams: false,
                match_tangents: [false; 2],
                ..prompt.state.options
            };
            if let Some(review) = &prompt.review {
                self.loft_simplification = review.simplification;
            }
        }
    }

    fn log_loft(&mut self) {
        if let Some(p) = &self.loft_prompt {
            let mut text = format!("Loft: {}. {}", p.hint(), p.state.options.arguments());
            if let Some(review) = &p.review {
                text.push_str(&format!(
                    " RebuildCount={} RefitTolerance={}",
                    review.simplification.rebuild,
                    review
                        .simplification
                        .refit
                        .unwrap_or(self.document.tolerance().absolute()),
                ));
                for (index, name) in ["MatchStartTangent", "MatchEndTangent"]
                    .into_iter()
                    .enumerate()
                {
                    if review.tangent_sides()[index]
                        && p.state.options.tangent_capable()[index]
                        && !p.state.options.match_tangents[index]
                    {
                        text.push_str(&format!(" {name}=No"));
                    }
                }
            }
            self.push_log(text);
        }
    }

    fn finish_loft(&mut self) {
        if !self.validate_loft() {
            return;
        }
        let prompt = self.loft_prompt.as_ref().unwrap();
        let required = prompt.required();
        if prompt.state.sources.len() < required {
            self.push_log(format!("Loft needs at least {required} curve section(s)"));
            return;
        }
        if prompt.uses_script() {
            self.finish_scripted_loft();
        } else if prompt.review.is_some() {
            self.accept_loft_preview();
        } else if prompt.seams.is_some() {
            self.accept_loft_seams();
        } else {
            self.begin_loft_sections();
        }
    }

    fn finish_scripted_loft(&mut self) {
        self.sync_loft_selection();
        let p = self.loft_prompt.take().unwrap();
        let input = format!("Loft {}", p.state.options.arguments());
        if !self.try_execute_command(&input) {
            self.loft_prompt = Some(p);
            self.log_loft();
        }
    }

    pub(super) fn validate_loft(&mut self) -> bool {
        let Some(prompt) = &self.loft_prompt else {
            return false;
        };
        if prompt
            .state
            .sources
            .iter()
            .any(|section| !section.source.current(&self.document))
        {
            self.cancel_interactive_command(false);
            self.push_log("Loft sources changed; select them again".into());
            return false;
        }
        true
    }

    pub(super) fn cancel_loft(&mut self, announce: bool) {
        if let Some(p) = self.loft_prompt.take() {
            self.restore_curve_selection(p.before);
            if announce {
                self.push_log("Loft canceled".into());
            }
        }
    }
}

fn required_sections(options: Options) -> usize {
    if options.closed {
        3
    } else if options.ends.is_empty() {
        2
    } else {
        1
    }
}

fn sources_ready(prompt: &Prompt) -> bool {
    prompt.picking.is_none() && prompt.state.sources.len() >= prompt.required()
}
