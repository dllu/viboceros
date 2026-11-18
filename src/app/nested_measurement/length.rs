//! Collect Length inputs privately while a numeric getter is suspended.
use super::*;
use std::collections::BTreeMap;
use viboceros_command::chain_selection::Target;
use viboceros_command::curve_reference::CapturedCurve;
use viboceros_command::{ComponentPick, length_edge_kind, measure_length_targets};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{Curve3, NurbsCurve};

struct Portion {
    source: CapturedCurve,
    guide: NurbsCurve,
    length: f64,
}

#[derive(Default)]
pub(super) struct Inputs {
    chosen: BTreeMap<Target, GeometrySnapshot>,
    portions: Vec<Portion>,
    choices: Option<(Vec<Target>, SelectionMode, bool)>,
    choosing_units: bool,
}

impl Inputs {
    pub(super) fn hint(&self) -> &'static str {
        if self.choosing_units {
            "Enter a Length unit name or Model_Units"
        } else if self.choices.is_some() {
            "Choose a Length input by number"
        } else {
            "Select curves or edges for Length; Units / SubCrv / SelChain; Enter measures"
        }
    }
}

impl Session {
    pub(in crate::app) fn length_guides(&self) -> Vec<&NurbsCurve> {
        self.length
            .portions
            .iter()
            .map(|portion| &portion.guide)
            .collect()
    }
}

impl VibocerosApp {
    pub(in crate::app) fn nested_length_wants_source(&self) -> bool {
        self.plane_prompt.is_none()
            && self.subcurve_prompt.is_none()
            && self.nested_measurement.as_ref().is_some_and(|session| {
                session.kind == Kind::Length && !session.length.choosing_units
            })
    }

    pub(in crate::app) fn nested_length_highlights(&self) -> Vec<Target> {
        let Some(session) = &self.nested_measurement else {
            return Vec::new();
        };
        let input = &session.length;
        input
            .chosen
            .keys()
            .copied()
            .chain(
                input
                    .choices
                    .iter()
                    .flat_map(|(choices, _, _)| choices.iter().copied()),
            )
            .collect()
    }

    pub(in crate::app) fn log_nested_length(&mut self) {
        let units = self.nested_length_units.unwrap_or("Model_Units");
        self.push_log(format!("Select curves or edges for Length, then press Enter (Units={units} / SubCrv / SelChain)"));
    }

    pub(in crate::app) fn pick_nested_length(
        &mut self,
        targets: Vec<Target>,
        mode: SelectionMode,
        direct: bool,
    ) {
        if !self.nested_length_wants_source() || targets.is_empty() {
            return;
        }
        if targets.len() == 1 {
            self.add_nested_length_targets(targets, mode, direct);
        } else {
            let labels = targets
                .iter()
                .enumerate()
                .map(|(i, target)| {
                    let kind = match target {
                        Target::Curve(_) => "curve".to_owned(),
                        Target::Edge(_, index) => format!("edge {}", index + 1),
                    };
                    let name = self
                        .document
                        .object(target.object())
                        .and_then(|o| o.attributes().name());
                    format!(
                        "{} {}",
                        i + 1,
                        name.map_or(kind.clone(), |name| format!("{name} ({kind})"))
                    )
                })
                .collect::<Vec<_>>();
            self.nested_measurement.as_mut().unwrap().length.choices =
                Some((targets, mode, direct));
            self.push_log(format!("Choose a Length input ({})", labels.join(", ")));
        }
        self.command_input.clear();
    }

    pub(in crate::app) fn add_nested_length_targets(
        &mut self,
        targets: Vec<Target>,
        mode: SelectionMode,
        direct: bool,
    ) {
        let Some(session) = &mut self.nested_measurement else {
            return;
        };
        let inputs = &mut session.length;
        let result = (|| -> Result<Vec<_>, CommandError> {
            let mut captured = BTreeMap::new();
            let mut edges = Vec::new();
            for target in targets {
                let id = target.object();
                let object = self
                    .document
                    .object(id)
                    .filter(|_| self.document.is_object_selectable(id))
                    .ok_or(CommandError::Usage("Length input is unavailable"))?;
                match target {
                    Target::Curve(_) if object.geometry().curve_ref().is_some() => {
                        captured.insert(target, object.geometry_snapshot().clone());
                        if !direct
                            && let Some(group) =
                                object.top_group().and_then(|id| self.document.group(id))
                        {
                            for id in group.members() {
                                if let Some(object) = self.document.object(id)
                                    && self.document.is_object_selectable(id)
                                    && object.geometry().curve_ref().is_some()
                                {
                                    captured.insert(
                                        Target::Curve(id),
                                        object.geometry_snapshot().clone(),
                                    );
                                }
                            }
                        }
                    }
                    Target::Edge(_, index) => {
                        let kind = length_edge_kind(object.geometry())
                            .ok_or(CommandError::UnsupportedLengthGeometry)?;
                        edges.push(ComponentPick {
                            object: id,
                            kind,
                            index,
                        });
                        captured.insert(target, object.geometry_snapshot().clone());
                    }
                    _ => return Err(CommandError::UnsupportedLengthGeometry),
                }
            }
            viboceros_command::component_selection::stage_sources(&self.document, edges)
                .map_err(CommandError::Usage)?;
            Ok(captured.into_iter().collect())
        })();
        match result {
            Ok(captured) => {
                inputs.choices = None;
                for (target, snapshot) in captured {
                    match mode {
                        SelectionMode::Remove => {
                            inputs.chosen.remove(&target);
                        }
                        SelectionMode::Toggle if inputs.chosen.contains_key(&target) => {
                            inputs.chosen.remove(&target);
                        }
                        _ => {
                            inputs.chosen.insert(target, snapshot);
                        }
                    }
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn continue_nested_length(&mut self, text: &str) -> bool {
        if self.chain_prompt.is_some() {
            self.continue_chain(text);
            return true;
        }
        let inputs = &mut self.nested_measurement.as_mut().unwrap().length;
        let enter = text.is_empty() || text.eq_ignore_ascii_case("Enter");
        if let Some((choices, mode, direct)) = &inputs.choices {
            if enter {
                inputs.choices = None;
            } else if let Ok(number) = text.parse::<usize>() {
                if let Some(&target) = number.checked_sub(1).and_then(|i| choices.get(i)) {
                    let (mode, direct) = (*mode, *direct);
                    self.add_nested_length_targets(vec![target], mode, direct);
                } else {
                    let count = choices.len();
                    self.push_log(format!("Choose a Length input from 1 to {count}"));
                }
            }
        } else if inputs.choosing_units
            || text
                .split('=')
                .next()
                .is_some_and(|key| key.eq_ignore_ascii_case("Units"))
        {
            if enter {
                inputs.choosing_units = false;
                self.log_nested_length();
            } else if !inputs.choosing_units && text.eq_ignore_ascii_case("Units") {
                inputs.choosing_units = true;
                self.push_log("Length units: enter a unit name or Model_Units".into());
            } else {
                let value = text.split_once('=').map_or(text, |(_, value)| value);
                match viboceros_command::radius_display_units(value) {
                    Ok(units) => {
                        self.nested_length_units = units;
                        inputs.choosing_units = false;
                        self.log_nested_length();
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
        } else if enter {
            if inputs.chosen.is_empty() && inputs.portions.is_empty() {
                return self.cancel_nested_measurement();
            }
            self.finish_nested_length();
        } else if text.eq_ignore_ascii_case("SubCrv") || text.eq_ignore_ascii_case("SubCurve") {
            self.begin_nested_length_subcurve_prompt();
        } else if !self.start_chain(text) {
            if let Some((name, value)) = text.split_once('=')
                && name.eq_ignore_ascii_case("Edge")
            {
                let target = value.split_once(',').and_then(|(id, edge)| {
                    Some(Target::Edge(id.parse().ok()?, edge.parse().ok()?))
                });
                if let Some(target) = target {
                    self.add_nested_length_targets(vec![target], SelectionMode::Add, true);
                } else {
                    self.push_log("Enter Edge=object-id,edge-index".into());
                }
            } else if let Some(id) = text.parse().ok().or_else(|| {
                let (name, value) = text.split_once(char::is_whitespace)?;
                name.trim_start_matches(['\'', '_'])
                    .eq_ignore_ascii_case("SelID")
                    .then(|| value.trim().parse().ok())
                    .flatten()
            }) {
                self.add_nested_length_targets(vec![Target::Curve(id)], SelectionMode::Add, true);
            } else {
                self.log_nested_length();
            }
        }
        self.command_input.clear();
        true
    }

    pub(in crate::app) fn continue_after_nested_length_portion(
        &mut self,
        portion: Option<(CapturedCurve, Curve3)>,
    ) -> Result<(), CommandError> {
        if let Some((source, curve)) = portion {
            let length = curve.as_ref().length(self.document.tolerance())?;
            let guide = curve.as_ref().to_nurbs()?;
            self.nested_measurement
                .as_mut()
                .expect("Length parent")
                .length
                .portions
                .push(Portion {
                    source,
                    guide,
                    length,
                });
        }
        self.subcurve_prompt = None;
        self.active_command = None;
        self.drafting_plane = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.snaps.model_override = None;
        self.log_nested_length();
        Ok(())
    }

    fn finish_nested_length(&mut self) {
        let result = (|| -> Result<_, CommandError> {
            let inputs = &self.nested_measurement.as_ref().unwrap().length;
            if inputs.chosen.iter().any(|(target, original)| {
                !self.document.is_object_selectable(target.object())
                    || self.document.object(target.object()).is_none_or(|object| {
                        !object.geometry_snapshot().shares_storage_with(original)
                    })
            }) || inputs
                .portions
                .iter()
                .any(|p| !p.source.current(&self.document))
            {
                return Err(CommandError::Usage("Length inputs changed"));
            }
            let mut total = measure_length_targets(&self.document, inputs.chosen.keys().copied())?;
            for portion in &inputs.portions {
                total.add(portion.length)?;
            }
            Ok((inputs.chosen.len() + inputs.portions.len(), total.total()?))
        })();
        match result {
            Ok((count, value)) => {
                self.push_log(format!("Measured {count} input(s). Total length = {value}"));
                self.finish_nested_value(value);
            }
            Err(error) => {
                self.nested_measurement.as_mut().unwrap().length = Inputs::default();
                self.push_log(format!("Error: {error}"));
                self.log_nested_length();
            }
        }
    }
}
