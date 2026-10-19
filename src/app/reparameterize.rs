//! Curve/face selection and numeric UV getters, committing through the registry.
use super::*;
use crate::viewport::{ComponentPick, ViewportOutput};
use viboceros_command::{ComponentSelectionKind, ObjectSelectionFilter};
use viboceros_document::GeometrySnapshot;

#[derive(Debug)]
struct Source {
    object: ObjectId,
    geometry: GeometrySnapshot,
}
impl Source {
    fn current(&self, document: &Document) -> bool {
        document.is_object_selectable(self.object)
            && document.object(self.object).is_some_and(|object| {
                self.geometry
                    .shares_storage_with(object.geometry_snapshot())
            })
    }
}

#[derive(Debug)]
struct Target {
    source: Source,
    face: Option<usize>,
    defaults: Vec<f64>,
}

#[derive(Debug)]
pub(super) struct Prompt {
    target: Option<Target>,
    constraint: Option<Source>,
    values: Vec<f64>,
    automatic: bool,
    tolerance: Tolerance,
    hint: String,
}
impl Prompt {
    pub(super) fn selecting(&self) -> bool {
        self.target.is_none()
    }
    pub(super) fn hint(&self) -> &str {
        &self.hint
    }
    pub(super) fn source_id(&self) -> Option<ObjectId> {
        self.target
            .as_ref()
            .map(|target| target.source.object)
            .or_else(|| self.constraint.as_ref().map(|source| source.object))
    }
    pub(super) fn preview_id(&self) -> Option<ObjectId> {
        self.target
            .as_ref()
            .filter(|target| target.face.is_none())
            .map(|target| target.source.object)
            .or_else(|| self.constraint.as_ref().map(|source| source.object))
    }
    pub(super) fn highlight(&self) -> Option<ComponentPick> {
        let target = self.target.as_ref()?;
        Some(ComponentPick {
            object: target.source.object,
            kind: ComponentSelectionKind::BrepFace,
            index: target.face?,
        })
    }
    fn current(&self, document: &Document) -> bool {
        self.tolerance == document.tolerance()
            && self
                .target
                .as_ref()
                .is_none_or(|t| t.source.current(document))
            && self.constraint.as_ref().is_none_or(|s| s.current(document))
    }
    fn update_hint(&mut self) {
        self.hint = if let Some(target) = &self.target {
            let names: &[&str] = if target.face.is_some() {
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
            format!(
                "{} <{}>{}; Enter accepts the current value, Esc cancels",
                names[index],
                target.defaults[index],
                if index == 0 { "; Automatic" } else { "" },
            )
        } else if self.constraint.is_some() {
            "Pick a face on the chosen polysurface; Esc cancels".into()
        } else {
            "Pick one curve or surface; Automatic; Esc cancels".into()
        };
    }
}

fn numbers(input: &str) -> Option<Vec<f64>> {
    input
        .split_whitespace()
        .flat_map(|word| word.split(','))
        .map(|word| word.parse::<f64>().ok().filter(|value| value.is_finite()))
        .collect()
}

impl VibocerosApp {
    pub(super) fn picking_reparameterize(&self) -> bool {
        self.reparameterize_prompt
            .as_ref()
            .is_some_and(Prompt::selecting)
    }
    pub(super) fn start_reparameterize(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|word| {
            word.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Reparameterize")
        }) {
            return false;
        }
        let arguments = words.collect::<Vec<_>>();
        // Explicit object/face recipes retain the registry's scripting path.
        if arguments.iter().any(|word| word.contains('=')) {
            return false;
        }
        let automatic = matches!(arguments.as_slice(), [word]
            if word.trim_start_matches(['_', '-']).eq_ignore_ascii_case("Automatic"));
        let values = if automatic {
            Some(vec![])
        } else {
            numbers(&arguments.join(" "))
        };
        let Some(values) = values.filter(|values| {
            values.len() <= 4 && values.chunks_exact(2).all(|pair| pair[1] > pair[0])
        }) else {
            return false;
        };
        let components = self.component_selection.checked_picks(&self.document);
        let objects = self
            .document
            .selected_objects()
            .filter(|o| {
                self.document.is_object_selectable(o.id())
                    && ObjectSelectionFilter::Parametric.accepts_object(o)
            })
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        self.reparameterize_prompt = Some(Prompt {
            target: None,
            constraint: None,
            values,
            automatic,
            tolerance: self.document.tolerance(),
            hint: String::new(),
        });
        self.push_log(format!("> {input}"));
        match components {
            Ok(picks) if picks.is_empty() => {
                if let [id] = objects.as_slice() {
                    self.pick_reparameterize_object(Some(*id));
                } else {
                    self.log_reparameterize();
                }
            }
            Ok(picks) => match picks.as_slice() {
                [pick] if pick.kind == ComponentSelectionKind::BrepFace => {
                    self.accept_reparameterize_face(pick.object, pick.index);
                }
                _ => {
                    self.push_log("Reparameterize requires one curve or surface face".into());
                    self.log_reparameterize();
                }
            },
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
        if !self.picking_reparameterize() {
            return true;
        }
        let Some(id) = id else {
            return true;
        };
        self.select_reparameterize_target(id, None);
        true
    }
    pub(super) fn accept_reparameterize_face(&mut self, id: ObjectId, face: usize) -> bool {
        if self.reparameterize_prompt.is_none() {
            return false;
        }
        if self.picking_reparameterize() {
            self.select_reparameterize_target(id, Some(face));
        }
        true
    }
    fn select_reparameterize_target(&mut self, id: ObjectId, face: Option<usize>) {
        self.validate_reparameterize();
        let Some(prompt) = &self.reparameterize_prompt else {
            return;
        };
        if prompt
            .constraint
            .as_ref()
            .is_some_and(|source| source.object != id)
        {
            self.push_log("Select a face on the chosen polysurface".into());
            return;
        }
        let Some(object) = self
            .document
            .object(id)
            .filter(|_| self.document.is_object_selectable(id))
        else {
            return;
        };
        let source = Source {
            object: id,
            geometry: object.geometry_snapshot().clone(),
        };
        let (face, defaults) = match object.geometry() {
            Geometry::NurbsSurface(surface) if face.is_none_or(|i| i == 0) => (
                Some(0),
                vec![
                    *surface.domain_u().start(),
                    *surface.domain_u().end(),
                    *surface.domain_v().start(),
                    *surface.domain_v().end(),
                ],
            ),
            Geometry::Brep(brep) => {
                let face = face.or_else(|| (brep.faces().len() == 1).then_some(0));
                let Some(face) = face else {
                    self.reparameterize_prompt.as_mut().unwrap().constraint = Some(source);
                    self.selection_menu = None;
                    self.log_reparameterize();
                    return;
                };
                let Some(face_surface) = brep.faces().get(face) else {
                    return;
                };
                let surface = face_surface.surface();
                (
                    Some(face),
                    vec![
                        *surface.domain_u().start(),
                        *surface.domain_u().end(),
                        *surface.domain_v().start(),
                        *surface.domain_v().end(),
                    ],
                )
            }
            geometry if face.is_none() => {
                let Some(curve) = geometry.curve_ref() else {
                    return;
                };
                let domain = curve.domain();
                (None, vec![*domain.start(), *domain.end()])
            }
            _ => return,
        };
        self.selection_menu = None;
        let prompt = self.reparameterize_prompt.as_mut().unwrap();
        prompt.constraint = None;
        prompt.target = Some(Target {
            source,
            face,
            defaults,
        });
        self.finish_reparameterize_if_ready();
    }
    fn finish_reparameterize_if_ready(&mut self) {
        let prompt = self.reparameterize_prompt.as_ref().unwrap();
        let target = prompt.target.as_ref().unwrap();
        if prompt.values.len() > target.defaults.len() {
            self.reparameterize_prompt.as_mut().unwrap().values.clear();
            self.push_log("Too many domain values for this object; enter the range again".into());
        } else if prompt.automatic || prompt.values.len() == target.defaults.len() {
            let values = if prompt.automatic {
                "Automatic".into()
            } else {
                prompt
                    .values
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let face = target
                .face
                .map(|i| format!(" Face={i}"))
                .unwrap_or_default();
            let input = format!(
                "Reparameterize Object={}{face} {values}",
                target.source.object
            );
            self.push_log(format!("> {input}"));
            match self.commands.execute(&mut self.document, &input) {
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
        if input
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Automatic")
        {
            if prompt.values.is_empty() {
                prompt.automatic = true;
                if prompt.target.is_some() {
                    self.finish_reparameterize_if_ready();
                } else {
                    self.log_reparameterize();
                }
            } else {
                self.push_log("Automatic is available at the domain start prompt".into());
            }
            return true;
        }
        let Some(target) = &prompt.target else {
            self.log_reparameterize();
            return true;
        };
        let new_values = if input.is_empty() {
            Some(vec![target.defaults[prompt.values.len()]])
        } else {
            numbers(input)
        };
        let Some(new_values) = new_values.filter(|values| {
            !values.is_empty() && values.len() + prompt.values.len() <= target.defaults.len()
        }) else {
            self.push_log("Enter finite domain values".into());
            return true;
        };
        let mut values = prompt.values.clone();
        values.extend(new_values);
        if values.chunks_exact(2).any(|pair| pair[1] <= pair[0]) {
            self.push_log("The domain end must be greater than its start".into());
            return true;
        }
        prompt.values = values;
        self.finish_reparameterize_if_ready();
        true
    }
    /// Resolve overlap choices before a face hit so a curve crossing a surface
    /// can still be chosen through the normal object selection menu.
    pub(super) fn reparameterize_viewport_hit(&mut self, output: &ViewportOutput) -> bool {
        if !self.picking_reparameterize() {
            return false;
        }
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
}
