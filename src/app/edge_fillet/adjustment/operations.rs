//! Pick and place additional size handles on the original selected edges.
use super::*;
use crate::viewport::edge_blend::HandleMode;
use viboceros_geometry::NurbsCurve;

#[derive(Clone, Copy)]
pub(super) enum HandleOperation {
    Place { size: f64, entering_size: bool },
    Copy,
    Remove,
}

impl VibocerosApp {
    pub(in crate::app::edge_fillet) fn start_edge_handle_operation(
        &mut self,
        mut prompt: Prompt,
        option: &str,
    ) -> bool {
        let operation = if option.eq_ignore_ascii_case("AddHandle") {
            HandleOperation::Place {
                size: prompt.radius,
                entering_size: false,
            }
        } else if option.eq_ignore_ascii_case("CopyHandle") {
            HandleOperation::Copy
        } else if option.eq_ignore_ascii_case("RemoveHandle") {
            if !self.has_added_edge_handles() {
                self.push_log("Use AddHandle to place a handle first.".into());
                self.command_input.clear();
                return true;
            }
            HandleOperation::Remove
        } else {
            return false;
        };
        self.edge_blend_adjustment.active = None;
        self.edge_blend_adjustment.drag_size = None;
        self.edge_blend_adjustment.operation = Some(operation);
        self.edge_blend_adjustment.placement_hover = None;
        prompt.size_input = None;
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.log_edge_blend_prompt();
        true
    }

    pub(in crate::app::edge_fillet) fn has_added_edge_handles(&self) -> bool {
        self.edge_blend_adjustment
            .sizes
            .iter()
            .any(|(&pick, (snapshot, sizes))| {
                !sizes.interior.is_empty()
                    && current(self.edge_blend_source_document(), pick, snapshot)
            })
    }

    pub(in crate::app::edge_fillet) fn edge_handle_operation_hint(
        &self,
        prompt: Prompt,
    ) -> Option<String> {
        Some(match self.edge_blend_adjustment.operation? {
            HandleOperation::Place {
                size,
                entering_size: false,
            } => format!(
                "Click or type a location along a selected edge. {}={size} sets the next size. Enter returns to handle adjustment.",
                placement_size_option(prompt)
            ),
            HandleOperation::Place {
                entering_size: true,
                ..
            } => "Enter the size for the next handle.".into(),
            HandleOperation::Copy => {
                "Pick the size handle to copy. Esc returns to handle adjustment.".into()
            }
            HandleOperation::Remove => {
                "Click or window-select added handles to remove. Enter returns to size adjustment."
                    .into()
            }
        })
    }

    pub(in crate::app::edge_fillet) fn continue_edge_placement_size(
        &mut self,
        mut prompt: Prompt,
        input: &str,
    ) -> bool {
        let Some(HandleOperation::Place {
            size,
            entering_size,
        }) = self.edge_blend_adjustment.operation
        else {
            return false;
        };
        if input.is_empty()
            || ["Enter", "Cancel"]
                .iter()
                .any(|option| input.eq_ignore_ascii_case(option))
        {
            return false;
        }
        let (option, value) = input
            .split_once(|ch: char| ch == '=' || ch.is_whitespace())
            .unwrap_or((input, ""));
        let named = option.eq_ignore_ascii_case(placement_size_option(prompt));
        if !entering_size && !named {
            return false;
        }
        let value = if named { value.trim() } else { input };
        let operation = if value.is_empty() {
            HandleOperation::Place {
                size,
                entering_size: true,
            }
        } else {
            match size_input::parse_size(value, &self.document) {
                Ok(size) => HandleOperation::Place {
                    size,
                    entering_size: false,
                },
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    self.command_input.clear();
                    return true;
                }
            }
        };
        self.edge_blend_adjustment.operation = Some(operation);
        self.edge_blend_adjustment.active = None;
        prompt.size_input = None;
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.log_edge_blend_prompt();
        true
    }

    pub(in crate::app) fn dismiss_edge_handle_operation(&mut self) -> bool {
        if !self.nested_measurement_captures_input() {
            return false;
        }
        let Some(operation) = self.edge_blend_adjustment.operation.take() else {
            return false;
        };
        self.edge_blend_adjustment.placement_hover = None;
        if let HandleOperation::Place {
            size,
            entering_size: true,
        } = operation
        {
            self.edge_blend_adjustment.operation = Some(HandleOperation::Place {
                size,
                entering_size: false,
            });
        }
        self.edge_blend_adjustment.active = None;
        self.edge_blend_adjustment.drag_size = None;
        if let Some(prompt) = &mut self.edge_fillet_prompt {
            prompt.size_input = None;
        }
        self.command_input.clear();
        self.log_edge_blend_prompt();
        true
    }

    pub(in crate::app) fn edge_handle_placement_curves<'a>(
        &self,
        document: &'a Document,
    ) -> Result<Vec<(ComponentPick, &'a NurbsCurve)>, &'static str> {
        if !matches!(
            self.edge_blend_adjustment.operation,
            Some(HandleOperation::Place {
                entering_size: false,
                ..
            })
        ) {
            return Ok(Vec::new());
        }
        Ok(self
            .component_selection
            .checked_picks(document)?
            .into_iter()
            .filter_map(|pick| {
                let Geometry::Brep(brep) = document.object(pick.object)?.geometry() else {
                    return None;
                };
                Some((pick, brep.edges().get(pick.index)?.curve()))
            })
            .collect())
    }

    pub(in crate::app) fn edge_handle_mode<'a>(
        &self,
        curves: &'a [(ComponentPick, &'a NurbsCurve)],
    ) -> HandleMode<'a> {
        match self.edge_blend_adjustment.operation {
            None => HandleMode::Edit,
            Some(HandleOperation::Copy) => HandleMode::Copy,
            Some(HandleOperation::Remove) => HandleMode::Remove,
            Some(HandleOperation::Place { size, .. }) => HandleMode::Place(curves, size),
        }
    }

    pub(in crate::app) fn update_edge_blend_placement_hover(
        &mut self,
        hover: Option<SizeHandleId>,
    ) -> bool {
        if self.edge_blend_adjustment.placement_hover == hover {
            return false;
        }
        self.edge_blend_adjustment.placement_hover = hover;
        self.edge_fillet_prompt
            .is_some_and(|prompt| prompt.toggles.preview)
    }

    pub(in crate::app::edge_fillet) fn edge_blend_placement_trial(&self) -> Option<ProfileEdit> {
        let HandleOperation::Place {
            size,
            entering_size: false,
        } = self.edge_blend_adjustment.operation?
        else {
            return None;
        };
        let SizeHandleId {
            pick,
            location: HandleLocation::Interior(parameter),
        } = self.edge_blend_adjustment.placement_hover?
        else {
            return None;
        };
        Some(ProfileEdit::Place {
            pick,
            parameter,
            size,
        })
    }

    pub(super) fn use_edge_handle_operation(&mut self, gesture: HandleGesture) -> bool {
        let (Some(prompt), Some(operation)) = (
            self.edge_fillet_prompt.filter(|p| p.adjusting),
            self.edge_blend_adjustment.operation,
        ) else {
            return false;
        };
        if matches!(operation, HandleOperation::Remove) {
            return match gesture {
                HandleGesture::Pick(id) => self.remove_edge_size_handles(vec![id]),
                HandleGesture::Select(ids, _) => self.remove_edge_size_handles(ids),
                _ => false,
            };
        }
        let id = match gesture {
            HandleGesture::Pick(id) | HandleGesture::Place { id, .. } => id,
            HandleGesture::Drag { .. } | HandleGesture::Select(..) => return false,
        };
        if !self
            .component_selection
            .checked_picks(self.edge_blend_source_document())
            .is_ok_and(|picks| picks.contains(&id.pick))
        {
            return false;
        }
        let Some(source) = self
            .edge_blend_source_document()
            .object(id.pick.object)
            .map(|object| object.geometry_snapshot().clone())
        else {
            return false;
        };
        let mut sizes = self.edge_blend_sizes(prompt, id.pick, None);
        match (operation, gesture, id.location) {
            (HandleOperation::Copy, HandleGesture::Pick(_), location) => {
                let Some(size) = location.size(&sizes) else {
                    return false;
                };
                self.edge_blend_adjustment.operation = Some(HandleOperation::Place {
                    size,
                    entering_size: false,
                });
                self.log_edge_blend_prompt();
                true
            }
            (
                HandleOperation::Place {
                    size,
                    entering_size: false,
                },
                HandleGesture::Place { point, .. },
                HandleLocation::Interior(t),
            ) if 0. < t && t < 1. => {
                if add_interior_handle(&mut sizes, t, size) {
                    self.edge_blend_adjustment
                        .sizes
                        .insert(id.pick, (source, sizes));
                }
                if !self.pick_edge_size_handle(id) {
                    return false;
                }
                self.last_point = Some(point);
                true
            }
            _ => false,
        }
    }
    fn remove_edge_size_handles(&mut self, ids: Vec<SizeHandleId>) -> bool {
        let document = edit::source_document(&self.edge_blend_edit, &self.document);
        let Ok(picks) = self.component_selection.checked_picks(document) else {
            return false;
        };
        let picks = picks.into_iter().collect::<BTreeSet<_>>();
        let mut stations = BTreeMap::<_, BTreeSet<_>>::new();
        for id in &ids {
            if let HandleLocation::Interior(t) = id.location
                && picks.contains(&id.pick)
            {
                stations.entry(id.pick).or_default().insert(t.to_bits());
            }
        }
        let mut removed = 0;
        for (pick, parameters) in stations {
            if let Some((source, sizes)) = self.edge_blend_adjustment.sizes.get_mut(&pick)
                && current(document, pick, source)
            {
                let previous = sizes.interior.len();
                sizes
                    .interior
                    .retain(|(t, _)| !parameters.contains(&t.to_bits()));
                removed += previous - sizes.interior.len();
            }
        }
        if removed > 0 {
            self.edge_blend_adjustment
                .selection
                .update(ids, SelectionMode::Remove);
            self.command_input.clear();
            self.push_log(format!(
                "Removed {removed} size handle(s). Pick more or press Enter."
            ));
        }
        true
    }
}

fn placement_size_option(prompt: Prompt) -> &'static str {
    if matches!(prompt.kind, Kind::Chamfer(_)) {
        "CurrentChamferDistance"
    } else {
        "CurrentRadius"
    }
}
