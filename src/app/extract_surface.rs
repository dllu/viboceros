//! Face picks accumulate without geometry edits until Enter accepts the batch.
use super::*;
use viboceros_command::{ComponentSelectionKind, ExtractSurfaceSelection};

pub(super) fn options(
    arguments: &[&str],
    mut copy: bool,
    mut current: bool,
) -> Option<(bool, bool)> {
    let mut seen = [false; 2];
    let mut i = 0;
    while i < arguments.len() {
        let (name, value, consumed) = if let Some((name, value)) = arguments[i].split_once('=') {
            (name, value, 1)
        } else {
            (arguments[i], *arguments.get(i + 1)?, 2)
        };
        let name = name.trim_start_matches(['_', '-']);
        let value = value.trim_start_matches('_');
        let index = if name.eq_ignore_ascii_case("Copy") {
            copy = if value.eq_ignore_ascii_case("Yes") {
                true
            } else if value.eq_ignore_ascii_case("No") {
                false
            } else {
                return None;
            };
            0
        } else if name.eq_ignore_ascii_case("OutputLayer") {
            current = if value.eq_ignore_ascii_case("Current") {
                true
            } else if value.eq_ignore_ascii_case("Input") {
                false
            } else {
                return None;
            };
            1
        } else {
            return None;
        };
        if seen[index] {
            return None;
        };
        seen[index] = true;
        i += consumed;
    }
    Some((copy, current))
}

impl VibocerosApp {
    pub(super) fn picking_extract_faces(&self) -> bool {
        matches!(
            self.active_command,
            Some(InteractiveCommand::ExtractSrf { .. })
        )
    }

    pub(super) fn start_extract_faces(&mut self, copy: bool, current: bool) {
        self.component_selection.valid_picks(&self.document);
        self.active_command = Some(InteractiveCommand::ExtractSrf {
            copy,
            output_on_current_layer: current,
        });
        self.push_log("ExtractSrf: select faces; Ctrl/Command removes; Enter extracts; Copy=Yes|No, OutputLayer=Input|Current (Esc cancels)".into());
    }

    pub(super) fn try_continue_extract_faces(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ExtractSrf {
            copy,
            output_on_current_layer: current,
        }) = self.active_command
        else {
            return false;
        };
        let input = input.trim();
        if input.is_empty() {
            self.finish_extract_faces(copy, current);
        } else if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
        } else if ["None", "SelNone"]
            .iter()
            .any(|word| input.trim_start_matches('_').eq_ignore_ascii_case(word))
        {
            self.component_selection.clear();
            self.document.clear_selection();
        } else if let Some((copy, current)) =
            options(&input.split_whitespace().collect::<Vec<_>>(), copy, current)
        {
            self.active_command = Some(InteractiveCommand::ExtractSrf {
                copy,
                output_on_current_layer: current,
            });
            self.push_log(format!(
                "Copy={} OutputLayer={}",
                if copy { "Yes" } else { "No" },
                if current { "Current" } else { "Input" }
            ));
        } else if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.cancel_interactive_command(false);
            return false;
        } else if self.try_continue_point_input(input) {
            return true;
        } else {
            self.push_log("Select faces, enter Copy=Yes|No or OutputLayer=Input|Current; Enter extracts, Esc cancels".into());
        }
        self.command_input.clear();
        true
    }

    fn finish_extract_faces(&mut self, copy: bool, current: bool) {
        let result = self
            .component_selection
            .checked_picks(&self.document)
            .map_err(str::to_owned)
            .and_then(|picks| {
                ExtractSurfaceSelection::prepare(
                    &self.document,
                    picks
                        .into_iter()
                        .filter(|p| p.kind == ComponentSelectionKind::BrepFace)
                        .map(|p| (p.object, p.index)),
                    copy,
                    current,
                )
                .and_then(|s| s.commit(&mut self.document))
                .map_err(|e| e.to_string())
            });
        match result {
            Ok(message) => {
                self.active_command = None;
                self.component_selection.clear();
                self.push_log(message);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn pick_extract_face_point(&mut self, point: Point3) {
        let result = (|| -> Result<_, viboceros_geometry::GeometryError> {
            let mut best = None;
            for object in self.document.selectable_objects() {
                let candidate = match object.geometry() {
                    Geometry::Brep(b) => b
                        .closest_face_parameters(point, self.document.tolerance())?
                        .map(|(index, u, v)| {
                            b.faces()[index]
                                .surface()
                                .evaluate(u, v)
                                .map(|p| (index, p))
                        })
                        .transpose()?,
                    Geometry::NurbsSurface(s) => {
                        let (u, v) = s.closest_parameters(point, self.document.tolerance())?;
                        Some((0, s.evaluate(u, v)?))
                    }
                    _ => None,
                };
                if let Some((index, p)) = candidate {
                    let distance = p.distance_to(point)?;
                    if best.is_none_or(|(_, _, old)| distance < old) {
                        best = Some((object.id(), index, distance));
                    }
                }
            }
            Ok(best)
        })();
        let best = match result {
            Ok(best) => best,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return;
            }
        };
        if let Some((object, index, _)) = best {
            self.accept_component_click(crate::viewport::ComponentClick {
                picks: vec![crate::viewport::ComponentPick {
                    object,
                    index,
                    kind: ComponentSelectionKind::BrepFace,
                }],
                preselection: false,
                modifiers: Default::default(),
            });
        } else {
            self.push_log("Error: no extractable surface face".into());
        }
    }
}
