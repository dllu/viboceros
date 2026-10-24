//! Shared two-set prompt routing for intersections, Booleans, and curve/surface tools.

use super::boolean_solids::BooleanOptions;
use super::*;
use viboceros_command::ObjectSelectionFilter;
use viboceros_document::{ObjectId, SelectionMode};

#[derive(Clone, Debug)]
pub(super) struct TwoSetsPrompt {
    pub(super) curve_extraction: Option<String>,
    pub(super) curve_deviation: Option<String>,
    pub(super) deviation: Option<String>,
    pub(super) first: Option<Vec<ObjectId>>,
    pub(super) output_layer: &'static str,
    pub(super) original_selection: Vec<ObjectId>,
    pub(super) boolean: Option<BooleanOptions>,
    pub(super) uv_mapping: Option<super::apply_curves::UvMappingKind>,
    pub(super) uv_face: Option<usize>,
    pub(super) uv_subcurves: super::uv_subcurve_input::SubcurveInputs,
    pub(super) projection: Option<super::project::ProjectSession>,
}

impl TwoSetsPrompt {
    pub(super) fn plane_getter(&self) -> Option<&super::project_plane::PlaneGetter> {
        self.projection
            .as_ref()
            .and_then(|p| p.plane_getter.as_ref())
            .or_else(|| {
                self.boolean
                    .as_ref()?
                    .partition
                    .as_ref()?
                    .plane_getter
                    .as_ref()
            })
    }
    pub(super) fn plane_getter_mut(&mut self) -> Option<&mut super::project_plane::PlaneGetter> {
        if let Some(p) = self.projection.as_mut() {
            p.plane_getter.as_mut()
        } else {
            self.boolean
                .as_mut()?
                .partition
                .as_mut()?
                .plane_getter
                .as_mut()
        }
    }
    pub(super) fn set_plane_getter(&mut self, getter: Option<super::project_plane::PlaneGetter>) {
        if let Some(p) = self.projection.as_mut() {
            p.plane_getter = getter;
        } else {
            self.boolean
                .as_mut()
                .unwrap()
                .partition
                .as_mut()
                .unwrap()
                .plane_getter = getter;
        }
    }
    pub(super) fn name(&self) -> &'static str {
        if let Some(base) = &self.curve_extraction {
            return if base
                .split_whitespace()
                .next()
                .unwrap()
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtractPt")
            {
                "ExtractPt"
            } else {
                "ExtractControlPolygon"
            };
        }
        if self.curve_deviation.is_some() {
            return "CrvDeviation";
        }
        if self.deviation.is_some() {
            return "PointDeviation";
        }
        if let Some(session) = &self.projection {
            return session.name();
        }
        if let Some(kind) = self.uv_mapping {
            kind.name()
        } else {
            self.boolean
                .as_ref()
                .map_or("IntersectTwoSets", |o| o.kind.name())
        }
    }
    pub(super) fn filter(&self) -> ObjectSelectionFilter {
        if self.curve_extraction.is_some() {
            return if self.uv_subcurves.pending.is_some() {
                ObjectSelectionFilter::Curves
            } else if self.name() == "ExtractPt" {
                ObjectSelectionFilter::ExtractPoints
            } else {
                ObjectSelectionFilter::ControlPolygons
            };
        }
        if self.curve_deviation.is_some() {
            return ObjectSelectionFilter::Curves;
        }
        if self.deviation.is_some() {
            return if self.first.is_some() {
                ObjectSelectionFilter::DeviationTargets
            } else {
                ObjectSelectionFilter::DeviationSources
            };
        }
        if let Some(getter) = self.plane_getter() {
            return getter.filter();
        }
        if self.uv_subcurves.pending.is_some() {
            return ObjectSelectionFilter::Curves;
        }
        if self.projection.as_ref().is_some_and(|p| p.is_iplane()) {
            return ObjectSelectionFilter::Section;
        }
        if self.projection.is_some() {
            return if self.first.is_some() {
                ObjectSelectionFilter::ProjectTargets
            } else {
                ObjectSelectionFilter::ApplyCurves
            };
        }
        if let Some(kind) = self.uv_mapping {
            kind.filter(self.first.is_some())
        } else if let Some(options) = &self.boolean {
            options.kind.filter_for(self.first.is_some())
        } else {
            ObjectSelectionFilter::Parametric
        }
    }
    pub(super) fn hint(&self) -> &'static str {
        if self.curve_extraction.is_some() {
            if self.name() == "ExtractPt" {
                return self.uv_subcurves.hint().unwrap_or("Select objects; SubCrv picks a portion; Output and OutputLayer set options; Enter extracts, Esc cancels");
            }
            return self.uv_subcurves.hint().unwrap_or("Select objects; SubCrv picks a portion; OutputLayer/ SubDPriority set options; Enter extracts, Esc cancels");
        }
        if self.curve_deviation.is_some() {
            return self.uv_subcurves.hint().unwrap_or("Select two curve inputs; SubCrv picks a temporary portion; Enter measures, Esc cancels");
        }
        if self.deviation.is_some() {
            return if self.first.is_some() {
                "Select target curves, surfaces, polysurfaces or meshes; Enter measures, Esc cancels"
            } else {
                "Select objects to measure; Enter continues, Esc cancels"
            };
        }
        if let Some(getter) = self.plane_getter() {
            return getter.hint();
        }
        if let Some(hint) = self.uv_subcurves.hint() {
            return hint;
        }
        if self.projection.as_ref().is_some_and(|p| p.custom_requested) {
            return "Pick two points for the custom direction; Esc cancels";
        }
        if self.projection.as_ref().is_some_and(|p| p.is_iplane()) {
            return "Select curves, surfaces, B-reps or meshes; Enter chooses a plane, Esc cancels";
        }
        if self.projection.as_ref().is_some_and(|p| p.is_pull()) {
            return if self.first.is_some() {
                "Select target surfaces, B-reps or meshes (IP defines an infinite plane); Enter pulls, Esc cancels"
            } else {
                "Select curves and points (SubCrv selects a temporary range); Enter continues, Esc cancels"
            };
        }
        if self.projection.is_some() {
            return if self.first.is_some() {
                "Select target surfaces, B-reps or meshes (IP defines an infinite plane); Enter projects, Esc cancels"
            } else {
                "Select curves and points (SubCrv selects a temporary range); Enter continues, Esc cancels"
            };
        }
        if let Some(kind) = self.uv_mapping {
            kind.hint(self.first.is_some())
        } else if let Some(options) = &self.boolean {
            options.kind.hint(self.first.is_some())
        } else if self.first.is_some() {
            "Select second set; Enter intersects, Esc cancels"
        } else {
            "Select first set; Enter continues, Esc cancels"
        }
    }
}

fn output_layer_option(input: &str) -> Option<&'static str> {
    let (name, value) = input.trim_start_matches('_').split_once('=')?;
    if !name.eq_ignore_ascii_case("OutputLayer") {
        return None;
    }
    let value = value.trim_start_matches('_');
    if value.eq_ignore_ascii_case("Current") {
        Some("Current")
    } else if value.eq_ignore_ascii_case("FirstSet") {
        Some("FirstSet")
    } else if value.eq_ignore_ascii_case("SecondSet") {
        Some("SecondSet")
    } else {
        None
    }
}

impl VibocerosApp {
    pub(super) fn try_start_intersection_prompt(&mut self, input: &str) -> bool {
        if self.try_start_curve_extraction_subcurve(input) {
            return true;
        }
        if self.try_start_curve_deviation_subcurve_prompt(input) {
            return true;
        }
        if self.try_start_point_deviation(input) {
            return true;
        }
        if self.try_start_project_prompt(input) {
            return true;
        }
        if self.try_start_apply_curves_prompt(input) {
            return true;
        }
        if self.try_start_boolean_solids_prompt(input) {
            return true;
        }
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("IntersectTwoSets")
        }) {
            return false;
        }
        let arguments = words.collect::<Vec<_>>();
        let output_layer = match arguments.as_slice() {
            [] => "Current",
            [option] => {
                let Some(value) = output_layer_option(option) else {
                    return false;
                };
                value
            }
            _ => return false,
        };
        let original_selection = self.document.selected_object_ids().collect::<Vec<_>>();
        let first = self
            .document
            .selected_objects()
            .filter(|object| ObjectSelectionFilter::Parametric.accepts_object(object))
            .map(|object| object.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.intersection_prompt = Some(TwoSetsPrompt {
            curve_extraction: None,
            curve_deviation: None,
            deviation: None,
            projection: None,
            first: (!first.is_empty()).then_some(first),
            output_layer,
            original_selection,
            boolean: None,
            uv_mapping: None,
            uv_face: None,
            uv_subcurves: Default::default(),
        });
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_intersection_prompt();
        true
    }

    pub(super) fn log_intersection_prompt(&mut self) {
        if let Some(prompt) = &self.intersection_prompt {
            if prompt.projection.is_some() {
                let prompt = prompt.clone();
                self.log_project_prompt(&prompt);
                return;
            }
            if prompt.uv_mapping.is_some()
                || prompt.deviation.is_some()
                || prompt.curve_deviation.is_some()
            {
                self.push_log(format!("{}: {}", prompt.name(), prompt.hint()));
                return;
            }
            if let Some(options) = &prompt.boolean {
                let command = options.prompt_line(prompt.first.is_some());
                self.push_log(format!("{}: {}", command, prompt.hint()));
                return;
            }
            self.push_log(format!(
                "IntersectTwoSets: {}; OutputLayer={}",
                prompt.hint(),
                prompt.output_layer
            ));
        }
    }

    pub(super) fn try_continue_intersection_prompt(&mut self, input: &str) -> bool {
        let Some(mut prompt) = self.intersection_prompt.clone() else {
            return false;
        };
        if prompt.curve_extraction.is_some() {
            return self.continue_curve_extraction_sources(prompt, input);
        }
        if prompt.curve_deviation.is_some() {
            return self.continue_curve_deviation_sources(prompt, input);
        }
        if prompt.deviation.is_some() {
            return self.continue_point_deviation(prompt, input);
        }
        if prompt.projection.is_some() {
            return self.continue_project_prompt(prompt, input);
        }
        if prompt.uv_mapping.is_some() {
            return self.continue_apply_curves_prompt(prompt, input);
        }
        if prompt.boolean.is_some() {
            return self.continue_boolean_solids_prompt(prompt, input);
        }
        if input.is_empty() {
            let selected = self
                .document
                .selected_objects()
                .filter(|object| ObjectSelectionFilter::Parametric.accepts_object(object))
                .map(|object| object.id())
                .collect::<Vec<_>>();
            if selected.is_empty() {
                self.push_log("Select at least one curve, surface, or B-rep; Esc cancels".into());
                return true;
            }
            if let Some(first) = &prompt.first {
                let first = first
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let second = selected
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let command = format!(
                    "IntersectTwoSets {first} {second} OutputLayer={}",
                    prompt.output_layer
                );
                if !self.try_execute_command(&command) {
                    self.intersection_prompt = Some(prompt);
                    self.push_log("Adjust the second set or output layer; Enter retries".into());
                }
            } else {
                prompt.first = Some(selected);
                self.document.clear_selection();
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
            }
            self.command_input.clear();
            return true;
        }
        if input
            .trim_start_matches('_')
            .split_once('=')
            .is_some_and(|(name, _)| name.eq_ignore_ascii_case("OutputLayer"))
        {
            if let Some(value) = output_layer_option(input) {
                prompt.output_layer = value;
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
            } else {
                self.push_log("OutputLayer must be Current, FirstSet, or SecondSet".into());
            }
            self.command_input.clear();
            return true;
        }
        let normalized = input.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if matches!(normalized.as_str(), "selall" | "selnone") {
            if normalized == "selnone" {
                self.document.clear_selection();
            } else {
                let ids = self
                    .document
                    .selectable_objects()
                    .filter(|object| ObjectSelectionFilter::Parametric.accepts_object(object))
                    .map(|object| object.id())
                    .collect::<Vec<_>>();
                self.select_intersection_prompt_objects(ids, SelectionMode::Add);
            }
            self.command_input.clear();
            return true;
        }
        if self.commands.recognizes(
            input
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_start_matches(['_', '-']),
        ) {
            self.cancel_intersection_prompt(true);
            return false;
        }
        let ids = input
            .split(',')
            .map(str::parse::<ObjectId>)
            .collect::<Result<Vec<_>, _>>();
        if let Ok(ids) = ids {
            self.select_intersection_prompt_objects(ids, SelectionMode::Add);
            self.command_input.clear();
        } else {
            self.push_log("Pick objects, type object IDs, or press Enter to continue".into());
        }
        true
    }

    pub(super) fn select_intersection_prompt_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        if self
            .intersection_prompt
            .as_ref()
            .is_some_and(|p| p.plane_getter().is_some())
        {
            let ids = ids.into_iter().collect::<Vec<_>>();
            self.pick_project_plane_reference(&ids);
            return;
        }
        if self.intersection_prompt.as_ref().is_some_and(|p| {
            (p.curve_deviation.is_some() || p.curve_extraction.is_some())
                && p.uv_subcurves.pending.is_some()
        }) {
            self.pick_uv_subcurve_source(ids);
            return;
        }
        if let Some(prompt) = &self.intersection_prompt {
            if prompt.projection.is_some() {
                self.select_project_objects(ids, mode);
                return;
            }
            if prompt.uv_mapping.is_some() {
                self.select_apply_curves_objects(ids, mode);
                return;
            }
            if prompt.boolean.is_some() {
                self.select_boolean_solids_objects(ids, mode);
                return;
            }
        } else {
            return;
        }
        let requested = ids.into_iter().collect::<std::collections::BTreeSet<_>>();
        let ids = self
            .document
            .selectable_objects()
            .filter(|object| requested.contains(&object.id()))
            .filter(|object| {
                self.intersection_prompt
                    .as_ref()
                    .unwrap()
                    .filter()
                    .accepts_object(object)
            })
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let mode = if mode == SelectionMode::Replace {
            SelectionMode::Add
        } else {
            mode
        };
        match self.document.select_objects_direct(ids, mode) {
            Ok(count) => {
                self.sync_extract_point_input_order();
                self.push_log(format!("Selected {count} object(s); Enter continues"));
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn cancel_intersection_prompt(&mut self, announce: bool) {
        if let Some(prompt) = self.intersection_prompt.take() {
            self.clear_project_plane_state();
            self.command_input.clear();
            let name = prompt.name();
            if let Some(options) = prompt.boolean {
                self.cancel_boolean_solids_prompt(options, prompt.first, announce);
                return;
            }
            if prompt.curve_extraction.is_some() {
                self.component_selection.clear();
            }
            if announce {
                let _ = self
                    .document
                    .select_objects_direct(prompt.original_selection, SelectionMode::Replace);
                self.push_log(format!("Cancelled {name}"));
            }
        }
    }
}
