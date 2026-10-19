//! Placed references, selection confirmation, and shared parameter getters.
use super::*;
use crate::viewport::{ParametricPick, ViewportOutput};
use viboceros_command::{
    ComponentSelectionKind, ReparameterizeDomains, ReparameterizeReference, ReparameterizeSelection,
};
use viboceros_document::GeometrySnapshot;

#[derive(Clone, Debug)]
struct Source {
    object: ObjectId,
    geometry: GeometrySnapshot,
}
impl Source {
    fn current(&self, document: &Document) -> bool {
        document.is_object_selectable(self.object)
            && document
                .object(self.object)
                .is_some_and(|o| self.geometry.shares_storage_with(o.geometry_snapshot()))
    }
}
#[derive(Clone, Debug)]
struct Target {
    source: Source,
    pick: ParametricPick,
    defaults: Vec<f64>,
}
impl Target {
    fn current(&self, doc: &Document) -> bool {
        if !self.source.current(doc) {
            return false;
        }
        let Some(index) = self.pick.member else {
            return true;
        };
        let Geometry::BlockInstance(instance) = &*self.source.geometry else {
            return false;
        };
        instance.members().get(index).is_some_and(|leaf| {
            doc.block_member_display(
                doc.object(self.pick.object).unwrap().attributes(),
                &leaf.path,
            )
            .is_ok_and(|state| state.visible && !state.locked)
        })
    }
    fn reference(&self) -> ReparameterizeReference {
        let member = self.pick.member.map(|index| {
            let Geometry::BlockInstance(instance) = &*self.source.geometry else {
                unreachable!()
            };
            instance.members()[index]
                .path
                .iter()
                .map(|p| p.member_index)
                .collect()
        });
        ReparameterizeReference {
            object: self.pick.object,
            member,
            face: self.pick.face,
        }
    }
    fn label(&self, doc: &Document) -> String {
        let object = doc.object(self.pick.object).unwrap();
        let mut label = object.attributes().name().unwrap_or("Object").to_string();
        if let Some(index) = self.pick.member {
            let Geometry::BlockInstance(instance) = &*self.source.geometry else {
                unreachable!()
            };
            for location in &instance.members()[index].path {
                if let Some(definition) = doc.block_definition(location.definition) {
                    let member = &definition.members()[location.member_index];
                    label.push_str(&format!(
                        " > {}[{}]",
                        definition.name(),
                        location.member_index
                    ));
                    if let Some(name) = member.attributes().name() {
                        label.push_str(&format!(" {name}"));
                    }
                }
            }
        }
        if let Some(face) = self.pick.face {
            label.push_str(&format!(" > face {face}"));
        } else {
            label.push_str(" (curve)");
        }
        label
    }
}
#[derive(Debug)]
struct Choice {
    targets: Vec<Target>,
    highlighted: usize,
    pointer: egui::Pos2,
}
#[derive(Debug)]
pub(super) struct Prompt {
    targets: Vec<Target>,
    constraint: Option<Source>,
    selecting: bool,
    choice: Option<Choice>,
    values: Vec<f64>,
    automatic: bool,
    tolerance: Tolerance,
    hint: String,
}
impl Prompt {
    pub(super) fn selecting(&self) -> bool {
        self.selecting
    }
    pub(super) fn has_menu(&self) -> bool {
        self.choice.is_some()
    }
    pub(super) fn hint(&self) -> &str {
        &self.hint
    }
    pub(super) fn source_id(&self) -> Option<ObjectId> {
        self.constraint.as_ref().map(|s| s.object)
    }
    pub(super) fn preview_id(&self) -> Option<ObjectId> {
        self.constraint.as_ref().map(|s| s.object)
    }
    pub(super) fn highlights(&self) -> Vec<ParametricPick> {
        let mut picks = self.targets.iter().map(|t| t.pick).collect::<Vec<_>>();
        if let Some(choice) = &self.choice {
            picks.push(choice.targets[choice.highlighted].pick);
        }
        picks
    }
    fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.targets.iter().all(|t| t.current(doc))
            && self.constraint.as_ref().is_none_or(|s| s.current(doc))
            && self
                .choice
                .as_ref()
                .is_none_or(|c| c.targets.iter().all(|t| t.current(doc)))
    }
    fn default(&self, index: usize) -> Option<f64> {
        let defaults = &self.targets.first()?.defaults;
        let value = *defaults.get(index)?;
        self.targets
            .iter()
            .all(|t| &t.defaults == defaults)
            .then_some(value)
    }
    fn update_hint(&mut self) {
        self.hint = if self.selecting {
            if self.constraint.is_some() {
                format!(
                    "Pick curves or surface faces in the chosen object; {} selected; Enter when done; Esc cancels",
                    self.targets.len()
                )
            } else {
                format!(
                    "Select curves or surface faces; {} selected; Enter when done; Esc cancels",
                    self.targets.len()
                )
            }
        } else {
            let names: &[&str] = if self.targets[0].defaults.len() == 4 {
                &[
                    "U domain start",
                    "U domain end",
                    "V domain start",
                    "V domain end",
                ]
            } else {
                &["Domain start", "Domain end"]
            };
            let index = self.values.len();
            let default = self
                .default(index)
                .map(|v| format!(" <{v}>; Enter accepts the current value"))
                .unwrap_or_default();
            format!(
                "{}{default}{}; Esc cancels",
                names[index],
                if index == 0 { "; Automatic" } else { "" }
            )
        };
    }
}
fn numbers(input: &str) -> Option<Vec<f64>> {
    input
        .split_whitespace()
        .flat_map(|word| word.split(','))
        .map(|word| word.parse::<f64>().ok().filter(|v| v.is_finite()))
        .collect()
}
fn target(doc: &Document, pick: ParametricPick) -> Option<Target> {
    let object = doc
        .object(pick.object)
        .filter(|_| doc.is_object_selectable(pick.object))?;
    let geometry = if let Some(index) = pick.member {
        let Geometry::BlockInstance(instance) = object.geometry() else {
            return None;
        };
        let leaf = instance.members().get(index)?;
        let display = doc
            .block_member_display(object.attributes(), &leaf.path)
            .ok()?;
        if !display.visible || display.locked {
            return None;
        }
        &*leaf.geometry
    } else {
        object.geometry()
    };
    let (face, defaults) = match geometry {
        Geometry::NurbsSurface(surface) if pick.face.is_none_or(|i| i == 0) => (
            Some(0),
            vec![
                *surface.domain_u().start(),
                *surface.domain_u().end(),
                *surface.domain_v().start(),
                *surface.domain_v().end(),
            ],
        ),
        Geometry::Brep(brep) => {
            let index = pick
                .face
                .or_else(|| (brep.faces().len() == 1).then_some(0))?;
            let surface = brep.faces().get(index)?.surface();
            (
                Some(index),
                vec![
                    *surface.domain_u().start(),
                    *surface.domain_u().end(),
                    *surface.domain_v().start(),
                    *surface.domain_v().end(),
                ],
            )
        }
        geometry if pick.face.is_none() => {
            let domain = geometry.curve_ref()?.domain();
            (None, vec![*domain.start(), *domain.end()])
        }
        _ => return None,
    };
    Some(Target {
        source: Source {
            object: pick.object,
            geometry: object.geometry_snapshot().clone(),
        },
        pick: ParametricPick { face, ..pick },
        defaults,
    })
}
impl VibocerosApp {
    pub(super) fn picking_reparameterize(&self) -> bool {
        self.reparameterize_prompt
            .as_ref()
            .is_some_and(Prompt::selecting)
    }
    pub(super) fn start_reparameterize(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Reparameterize")
        }) {
            return false;
        }
        let args = words.collect::<Vec<_>>();
        if args.iter().any(|w| w.contains('=')) {
            return false;
        }
        let automatic = matches!(args.as_slice(),[w]if w.trim_start_matches(['_','-']).eq_ignore_ascii_case("Automatic"));
        let values = if automatic {
            Some(vec![])
        } else {
            numbers(&args.join(" "))
        };
        let Some(values) =
            values.filter(|v| v.len() <= 4 && v.chunks_exact(2).all(|p| p[1] > p[0]))
        else {
            return false;
        };
        let components = self.component_selection.checked_picks(&self.document);
        let objects = self
            .document
            .selected_objects()
            .filter(|o| {
                self.document.is_object_selectable(o.id())
                    && (o.geometry().curve_ref().is_some()
                        || matches!(
                            o.geometry(),
                            Geometry::NurbsSurface(_)
                                | Geometry::Brep(_)
                                | Geometry::BlockInstance(_)
                        ))
            })
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        self.reparameterize_prompt = Some(Prompt {
            targets: vec![],
            constraint: None,
            selecting: true,
            choice: None,
            values,
            automatic,
            tolerance: self.document.tolerance(),
            hint: String::new(),
        });
        self.push_log(format!("> {input}"));
        match components {
            Ok(picks) if !picks.is_empty() => {
                for pick in picks
                    .into_iter()
                    .filter(|p| p.kind == ComponentSelectionKind::BrepFace)
                {
                    self.add_reparameterize_pick(ParametricPick {
                        object: pick.object,
                        member: None,
                        face: Some(pick.index),
                    });
                }
                self.confirm_reparameterize_selection();
            }
            Ok(_) => {
                for id in &objects {
                    if let Some(t) = target(
                        &self.document,
                        ParametricPick {
                            object: *id,
                            member: None,
                            face: None,
                        },
                    ) {
                        self.reparameterize_prompt.as_mut().unwrap().targets.push(t);
                    } else if objects.len() == 1 {
                        self.reparameterize_prompt.as_mut().unwrap().constraint = Some(Source {
                            object: *id,
                            geometry: self
                                .document
                                .object(*id)
                                .unwrap()
                                .geometry_snapshot()
                                .clone(),
                        });
                    }
                }
                if !objects.is_empty()
                    && self.reparameterize_prompt.as_ref().unwrap().targets.len() == objects.len()
                {
                    self.confirm_reparameterize_selection();
                } else {
                    self.log_reparameterize();
                }
            }
            Err(error) => {
                self.component_selection.clear();
                self.push_log(format!("Error: {error}"));
                self.log_reparameterize();
            }
        }
        true
    }
    fn log_reparameterize(&mut self) {
        if let Some(prompt) = &mut self.reparameterize_prompt {
            prompt.update_hint();
            let hint = prompt.hint.clone();
            self.push_log(format!("Reparameterize: {hint}"));
        }
    }
    pub(super) fn validate_reparameterize(&mut self) {
        if self
            .reparameterize_prompt
            .as_ref()
            .is_some_and(|p| !p.current(&self.document))
        {
            self.reparameterize_prompt = None;
            self.selection_menu = None;
            self.push_log(
                "Reparameterize source or tolerance changed; select the source again".into(),
            );
        }
    }
    pub(super) fn pick_reparameterize_object(&mut self, id: Option<ObjectId>) -> bool {
        if self.reparameterize_prompt.is_none() {
            return false;
        }
        if self.picking_reparameterize()
            && let Some(id) = id
        {
            let pick = ParametricPick {
                object: id,
                member: None,
                face: None,
            };
            if target(&self.document, pick).is_some() {
                self.add_reparameterize_pick(pick);
            } else if self
                .reparameterize_prompt
                .as_ref()
                .unwrap()
                .constraint
                .is_none()
                && let Some(object) = self
                    .document
                    .object(id)
                    .filter(|_| self.document.is_object_selectable(id))
                && matches!(
                    object.geometry(),
                    Geometry::Brep(_) | Geometry::BlockInstance(_)
                )
            {
                self.reparameterize_prompt.as_mut().unwrap().constraint = Some(Source {
                    object: id,
                    geometry: object.geometry_snapshot().clone(),
                });
                self.log_reparameterize();
            }
        }
        true
    }
    pub(super) fn accept_reparameterize_face(&mut self, id: ObjectId, face: usize) -> bool {
        if self.reparameterize_prompt.is_none() {
            return false;
        }
        if self.picking_reparameterize() {
            self.add_reparameterize_pick(ParametricPick {
                object: id,
                member: None,
                face: Some(face),
            });
        }
        true
    }
    fn add_reparameterize_pick(&mut self, pick: ParametricPick) {
        self.validate_reparameterize();
        let Some(prompt) = &self.reparameterize_prompt else {
            return;
        };
        if !prompt.selecting
            || prompt
                .constraint
                .as_ref()
                .is_some_and(|s| s.object != pick.object)
        {
            return;
        }
        let Some(target) = target(&self.document, pick) else {
            return;
        };
        let prompt = self.reparameterize_prompt.as_mut().unwrap();
        if !prompt.targets.iter().any(|t| t.pick == target.pick) {
            prompt.targets.push(target);
        }
        prompt.choice = None;
        self.selection_menu = None;
        self.log_reparameterize();
    }
    fn confirm_reparameterize_selection(&mut self) {
        let prompt = self.reparameterize_prompt.as_mut().unwrap();
        if prompt.targets.is_empty() {
            self.log_reparameterize();
            return;
        }
        let dimension = prompt.targets[0].defaults.len();
        if prompt.targets.iter().any(|t| t.defaults.len() != dimension) {
            self.push_log("Can only reparameterize curves or surfaces, not both".into());
            self.reparameterize_prompt = None;
            return;
        }
        prompt.selecting = false;
        prompt.choice = None;
        self.finish_reparameterize_if_ready();
    }
    fn finish_reparameterize_if_ready(&mut self) {
        let prompt = self.reparameterize_prompt.as_ref().unwrap();
        if prompt.selecting {
            self.log_reparameterize();
            return;
        }
        let dimension = prompt.targets[0].defaults.len();
        if prompt.values.len() > dimension {
            self.reparameterize_prompt.as_mut().unwrap().values.clear();
            self.push_log("Too many domain values for these objects; enter the range again".into());
        } else if prompt.automatic || prompt.values.len() == dimension {
            let domains = if prompt.automatic {
                ReparameterizeDomains::Automatic
            } else {
                ReparameterizeDomains::Explicit(prompt.values.clone())
            };
            let selection = ReparameterizeSelection::prepare(
                &self.document,
                prompt.targets.iter().map(Target::reference),
                domains,
            );
            match selection.and_then(|s| s.commit(&mut self.document)) {
                Ok(report) => {
                    self.reparameterize_prompt = None;
                    self.component_selection.clear();
                    self.push_log(report);
                    return;
                }
                Err(error) => {
                    let prompt = self.reparameterize_prompt.as_mut().unwrap();
                    prompt.values.clear();
                    prompt.automatic = false;
                    self.push_log(format!("Error: {error}"));
                }
            }
        }
        self.log_reparameterize();
    }
    pub(super) fn continue_reparameterize(&mut self, input: &str) -> bool {
        if self.reparameterize_prompt.is_none() {
            return false;
        }
        let input = input.trim();
        let first = input
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches(['_', '-']);
        if first.eq_ignore_ascii_case("Cancel") {
            self.reparameterize_prompt = None;
            self.command_input.clear();
            return true;
        }
        if self.commands.recognizes(first) {
            self.reparameterize_prompt = None;
            return false;
        }
        self.validate_reparameterize();
        let Some(prompt) = &mut self.reparameterize_prompt else {
            return true;
        };
        self.command_input.clear();
        if prompt.selecting {
            if input.is_empty() {
                self.confirm_reparameterize_selection();
            } else if input
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Automatic")
            {
                prompt.automatic = true;
                self.log_reparameterize();
            } else {
                self.log_reparameterize();
            }
            return true;
        }
        if input
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Automatic")
        {
            if prompt.values.is_empty() {
                prompt.automatic = true;
                self.finish_reparameterize_if_ready();
            } else {
                self.push_log("Automatic is available at the domain start prompt".into());
            }
            return true;
        }
        let dimension = prompt.targets[0].defaults.len();
        let new_values = if input.is_empty() {
            prompt.default(prompt.values.len()).map(|v| vec![v])
        } else {
            numbers(input)
        };
        let Some(new_values) =
            new_values.filter(|v| !v.is_empty() && v.len() + prompt.values.len() <= dimension)
        else {
            self.push_log(
                "Enter finite domain values; differing source domains have no shared default"
                    .into(),
            );
            return true;
        };
        let mut values = prompt.values.clone();
        values.extend(new_values);
        if values.chunks_exact(2).any(|p| p[1] <= p[0]) {
            self.push_log("The domain end must be greater than its start".into());
            return true;
        }
        prompt.values = values;
        self.finish_reparameterize_if_ready();
        true
    }
    pub(super) fn reparameterize_viewport_hit(&mut self, output: &ViewportOutput) -> bool {
        if !self.picking_reparameterize() {
            return false;
        }
        if let Some(picks) = &output.parametric_click {
            let targets = picks
                .iter()
                .filter_map(|p| target(&self.document, *p))
                .collect::<Vec<_>>();
            match targets.len() {
                0 => {}
                1 => self.add_reparameterize_pick(targets[0].pick),
                _ => {
                    self.reparameterize_prompt.as_mut().unwrap().choice = Some(Choice {
                        targets,
                        highlighted: 0,
                        pointer: output.selection_pointer.unwrap_or(egui::Pos2::ZERO),
                    });
                }
            }
            return true;
        }
        // Retain scripted/test object selection entry points.
        if let Some(choice) = &output.selection_choice {
            self.selection_menu = Some(SelectionMenu {
                choice: choice.clone(),
                highlighted: 0,
            });
            return true;
        }
        if let Some(click) = output.selection_click {
            if let Some((id, face)) = output
                .face_click
                .filter(|(id, _)| click.object_id.is_none_or(|chosen| chosen == *id))
            {
                self.accept_reparameterize_face(id, face);
            } else {
                self.pick_reparameterize_object(click.object_id);
            }
            return true;
        }
        false
    }
    pub(super) fn dismiss_reparameterize_menu(&mut self) -> bool {
        self.reparameterize_prompt
            .as_mut()
            .is_some_and(|p| p.choice.take().is_some())
    }
    pub(super) fn show_reparameterize_menu(
        &mut self,
        ui: &egui::Ui,
        outputs: &[ViewportOutput],
    ) -> bool {
        let Some(choice) = self
            .reparameterize_prompt
            .as_ref()
            .and_then(|p| p.choice.as_ref())
        else {
            return false;
        };
        let mut picked = None;
        let mut hovered = None;
        egui::Window::new("Selection Menu")
            .id(egui::Id::new("parametric_selection_menu"))
            .fixed_pos(choice.pointer + egui::vec2(12., 12.))
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                egui::ScrollArea::vertical()
                    .max_height(260.)
                    .show(ui, |ui| {
                        for (i, target) in choice.targets.iter().enumerate() {
                            let response = ui.selectable_label(
                                i == choice.highlighted,
                                format!("{}. {}", i + 1, target.label(&self.document)),
                            );
                            if response.hovered() {
                                hovered = Some(i);
                            }
                            if response.clicked() {
                                picked = Some(Some(i));
                            }
                        }
                        if ui.button("None").clicked() {
                            picked = Some(None);
                        }
                    });
            });
        let choice = self
            .reparameterize_prompt
            .as_mut()
            .unwrap()
            .choice
            .as_mut()
            .unwrap();
        if let Some(index) = hovered {
            choice.highlighted = index;
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            choice.highlighted = (choice.highlighted + 1) % choice.targets.len();
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            choice.highlighted =
                (choice.highlighted + choice.targets.len() - 1) % choice.targets.len();
        }
        let wants_keyboard = ui.ctx().egui_wants_keyboard_input();
        if picked.is_none()
            && ui.input(|i| {
                (!wants_keyboard && i.key_pressed(egui::Key::Enter))
                    || i.pointer.secondary_clicked()
            })
        {
            picked = Some(Some(choice.highlighted));
        }
        if let Some(picked) = picked {
            let choice = self
                .reparameterize_prompt
                .as_mut()
                .unwrap()
                .choice
                .take()
                .unwrap();
            if let Some(index) = picked {
                self.add_reparameterize_pick(choice.targets[index].pick);
            }
            return true;
        }
        if ui.input(|i| i.pointer.primary_clicked()) {
            if ui
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|p| p.distance(choice.pointer) <= 3.)
                && outputs.iter().any(|o| o.parametric_click.is_some())
            {
                choice.highlighted = (choice.highlighted + 1) % choice.targets.len();
                return true;
            }
            self.reparameterize_prompt.as_mut().unwrap().choice = None;
            return !outputs
                .iter()
                .any(|o| o.parametric_click.as_ref().is_some_and(|p| !p.is_empty()));
        }
        false
    }
}
