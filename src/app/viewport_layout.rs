//! Docked viewport layout operations.

use super::*;
use viboceros_command::interface::{FourViewProjection, InterfaceCommand, ViewportTabAlignment};

#[derive(Clone, Copy)]
enum ViewportTabAction {
    Select(usize),
    Cycle(bool),
    Align(ViewportTabAlignment),
    Rename(usize),
    Maximize(usize),
    Close(usize),
    New,
}

pub(super) struct ViewportTabRename {
    index: usize,
    text: String,
    focus_requested: bool,
}

fn split_rect(position: [f64; 4], horizontal: bool) -> Option<([f64; 4], [f64; 4])> {
    let [left, right, top, bottom] = position;
    if horizontal {
        let middle = top + (bottom - top) * 0.5;
        (middle > top && middle < bottom)
            .then_some(([left, right, top, middle], [left, right, middle, bottom]))
    } else {
        let middle = left + (right - left) * 0.5;
        (middle > left && middle < right)
            .then_some(([left, middle, top, bottom], [middle, right, top, bottom]))
    }
}

fn unique_viewport_title(viewports: &[Viewport], base: &str) -> String {
    let base = if base.trim().is_empty() {
        "Viewport"
    } else {
        base
    };
    let base = base
        .rsplit_once(" (")
        .and_then(|(stem, suffix)| {
            suffix
                .strip_suffix(')')
                .and_then(|digits| digits.parse::<usize>().ok())
                .filter(|number| *number >= 2)
                .map(|_| stem)
        })
        .unwrap_or(base);
    for suffix in 2.. {
        let candidate = format!("{base} ({suffix})");
        if !viewports
            .iter()
            .any(|viewport| viewport.view_label().eq_ignore_ascii_case(&candidate))
        {
            return candidate;
        }
    }
    unreachable!("a finite viewport list cannot use every positive suffix")
}

fn remap_viewport_index(index: usize, removed: usize) -> Option<usize> {
    (index != removed).then_some(index - usize::from(index > removed))
}

fn rectangles_overlap(position: [f64; 4], other: [f64; 4]) -> bool {
    let overlap_x = position[1].min(other[1]) - position[0].max(other[0]);
    let overlap_y = position[3].min(other[3]) - position[2].max(other[2]);
    overlap_x > 1e-12 && overlap_y > 1e-12
}

fn region_is_covered(region: [f64; 4], rectangles: &[[f64; 4]]) -> bool {
    let [left, right, top, bottom] = region;
    if !(left < right && top < bottom) {
        return false;
    }
    let mut x_cuts = vec![left, right];
    for rectangle in rectangles {
        if rectangles_overlap(region, *rectangle) {
            x_cuts.push(rectangle[0].clamp(left, right));
            x_cuts.push(rectangle[1].clamp(left, right));
        }
    }
    x_cuts.sort_by(f64::total_cmp);
    x_cuts.dedup_by(|a, b| (*a - *b).abs() <= 1e-12);
    if x_cuts.len() < 2 {
        return false;
    }
    x_cuts.windows(2).all(|window| {
        let middle = window[0] + (window[1] - window[0]) * 0.5;
        let mut spans = rectangles
            .iter()
            .filter(|rectangle| rectangle[0] <= middle && middle < rectangle[1])
            .filter_map(|rectangle| {
                let start = rectangle[2].max(top);
                let end = rectangle[3].min(bottom);
                (start < end).then_some((start, end))
            })
            .collect::<Vec<_>>();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut cursor = top;
        for (start, end) in spans {
            if start > cursor + 1e-12 {
                return false;
            }
            cursor = cursor.max(end);
        }
        cursor >= bottom - 1e-12
    })
}

fn positions_after_close(positions: &[[f64; 4]], removed: usize) -> Vec<[f64; 4]> {
    let hole = positions[removed];
    let remaining = positions
        .iter()
        .enumerate()
        .filter_map(|(index, position)| (index != removed).then_some(*position))
        .collect::<Vec<_>>();
    if remaining.len() == 1 {
        return vec![[0.0, 1.0, 0.0, 1.0]];
    }
    // A neighboring strip can contain one view or several views created by
    // later splits. Expand the whole strip when it exactly covers an edge.
    for side in 0..4 {
        let (edge, start, end) = match side {
            0 | 1 => (if side == 0 { hole[1] } else { hole[0] }, hole[2], hole[3]),
            _ => (if side == 2 { hole[3] } else { hole[2] }, hole[0], hole[1]),
        };
        let mut neighbors = remaining
            .iter()
            .enumerate()
            .filter_map(|(index, position)| {
                let (adjacent, first, last) = match side {
                    0 => (position[0], position[2], position[3]),
                    1 => (position[1], position[2], position[3]),
                    2 => (position[2], position[0], position[1]),
                    _ => (position[3], position[0], position[1]),
                };
                ((adjacent - edge).abs() <= 1e-12
                    && first >= start - 1e-12
                    && last <= end + 1e-12
                    && first < last)
                    .then_some((index, first, last))
            })
            .collect::<Vec<_>>();
        neighbors.sort_by(|a, b| a.1.total_cmp(&b.1));
        if neighbors.is_empty() || (neighbors[0].1 - start).abs() > 1e-12 {
            continue;
        }
        let mut cursor = start;
        if neighbors.iter().any(|(_, first, last)| {
            let contiguous = (*first - cursor).abs() <= 1e-12;
            cursor = *last;
            !contiguous
        }) || (cursor - end).abs() > 1e-12
        {
            continue;
        }
        let mut result = remaining.clone();
        for (index, _, _) in &neighbors {
            match side {
                0 => result[*index][0] = hole[0],
                1 => result[*index][1] = hole[1],
                2 => result[*index][2] = hole[2],
                _ => result[*index][3] = hole[3],
            }
        }
        if result.iter().enumerate().all(|(index, position)| {
            result
                .iter()
                .enumerate()
                .skip(index + 1)
                .all(|(other_index, other)| {
                    !rectangles_overlap(*position, *other)
                        || rectangles_overlap(remaining[index], remaining[other_index])
                        || rectangles_overlap(hole, remaining[index])
                        || rectangles_overlap(hole, remaining[other_index])
                })
        }) {
            return result;
        }
    }
    if region_is_covered(hole, &remaining) {
        return remaining;
    }
    super::named_view::default_viewport_positions(remaining.len())
}

impl VibocerosApp {
    pub(super) fn restore_four_view_projection(&mut self, projection: FourViewProjection) {
        self.four_view_projection = projection;
        let kinds = match projection {
            FourViewProjection::FirstAngle => [
                ViewKind::Front,
                ViewKind::Left,
                ViewKind::Top,
                ViewKind::Perspective,
            ],
            FourViewProjection::ThirdAngle => [
                ViewKind::Top,
                ViewKind::Perspective,
                ViewKind::Front,
                ViewKind::Right,
            ],
        };
        let source = &self.viewports[self.active_viewport];
        let viewports = kinds
            .into_iter()
            .map(|kind| {
                let mut viewport = Viewport::new_for_layout(source, kind);
                if let Some(previous) = self.viewports.iter().find(|view| view.kind() == kind) {
                    viewport.display_mode = previous.display_mode;
                    viewport.set_grid_settings(previous.grid_settings());
                } else if kind == ViewKind::Left
                    && let Some(right) = self
                        .viewports
                        .iter()
                        .find(|view| view.kind() == ViewKind::Right)
                {
                    viewport.set_grid_settings(right.grid_settings());
                }
                viewport
            })
            .collect();
        self.viewports = viewports;
        self.viewport_positions = DEFAULT_VIEWPORT_POSITIONS.to_vec();
        self.active_viewport = match projection {
            FourViewProjection::FirstAngle => 3,
            FourViewProjection::ThirdAngle => 1,
        };
        self.maximized_viewport = None;
        self.viewport_tab_rename = None;
        self.push_log(format!(
            "Restored {} four-view projection",
            projection.label()
        ));
    }

    pub(super) fn activate_model_viewport(&mut self, index: usize) {
        if index >= self.viewports.len() {
            return;
        }
        self.active_viewport = index;
        if self.maximized_viewport.is_some() {
            self.maximized_viewport = Some(index);
        }
    }

    pub(super) fn set_viewport_tab_alignment(&mut self, alignment: ViewportTabAlignment) {
        self.viewport_tab_alignment = alignment;
        self.push_log(format!("Viewport tabs aligned {}", alignment.label()));
    }

    pub(super) fn show_viewport_tabs(&mut self, root: &mut egui::Ui) -> Option<egui::Rect> {
        if !self.viewport_tabs_visible {
            return None;
        }
        let mut action = None;
        let alignment = self.viewport_tab_alignment;
        let panel = match alignment {
            ViewportTabAlignment::Bottom => egui::Panel::bottom("viewport_tabs_bottom"),
            ViewportTabAlignment::Top => egui::Panel::top("viewport_tabs_top"),
            ViewportTabAlignment::Left => {
                egui::Panel::left("viewport_tabs_left").default_size(160.0)
            }
            ViewportTabAlignment::Right => {
                egui::Panel::right("viewport_tabs_right").default_size(160.0)
            }
        }
        .show(root, |ui| {
            if alignment.is_vertical() {
                ui.vertical(|ui| self.viewport_tabs_content(ui, &mut action, true));
            } else {
                ui.horizontal(|ui| self.viewport_tabs_content(ui, &mut action, false));
            }
        });
        if action.is_none()
            && root
                .ctx()
                .pointer_hover_pos()
                .is_some_and(|pointer| panel.response.rect.contains(pointer))
        {
            let scroll = root.input(|input| {
                input
                    .raw
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        egui::Event::MouseWheel { delta, .. } => Some(delta.y),
                        _ => None,
                    })
                    .sum::<f32>()
            });
            if scroll != 0.0 {
                action = Some(ViewportTabAction::Cycle(scroll < 0.0));
            }
        }
        if let Some(action) = action {
            self.apply_viewport_tab_action(action);
        }
        self.show_viewport_tab_rename(root.ctx());
        Some(panel.response.rect)
    }

    fn viewport_tabs_content(
        &self,
        ui: &mut egui::Ui,
        action: &mut Option<ViewportTabAction>,
        vertical: bool,
    ) {
        ui.label("Model views");
        if ui.button("+").on_hover_text("New viewport").clicked() {
            *action = Some(ViewportTabAction::New);
        }
        ui.separator();
        let scroll = if vertical {
            egui::ScrollArea::vertical()
        } else {
            egui::ScrollArea::horizontal()
        };
        scroll.id_salt("model_viewport_tabs").show(ui, |ui| {
            let layout = if vertical {
                egui::Layout::top_down(egui::Align::Min)
            } else {
                egui::Layout::left_to_right(egui::Align::Center)
            };
            ui.with_layout(layout, |ui| {
                for (index, viewport) in self.viewports.iter().enumerate() {
                    let response = ui.selectable_label(
                        self.active_viewport == index,
                        format!("{} {}", index + 1, viewport.view_label()),
                    );
                    if response.clicked() {
                        *action = Some(ViewportTabAction::Select(index));
                    }
                    if response.double_clicked() {
                        *action = Some(ViewportTabAction::Rename(index));
                    }
                    response.context_menu(|ui| {
                        if ui.button("Activate").clicked() {
                            *action = Some(ViewportTabAction::Select(index));
                            ui.close();
                        }
                        if ui.button("Rename").clicked() {
                            *action = Some(ViewportTabAction::Rename(index));
                            ui.close();
                        }
                        if ui
                            .button(if self.maximized_viewport == Some(index) {
                                "Restore layout"
                            } else {
                                "Maximize"
                            })
                            .clicked()
                        {
                            *action = Some(ViewportTabAction::Maximize(index));
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.viewports.len() > 1,
                                egui::Button::new("Close viewport"),
                            )
                            .clicked()
                        {
                            *action = Some(ViewportTabAction::Close(index));
                            ui.close();
                        }
                        ui.menu_button("Tab position", |ui| {
                            for alignment in ViewportTabAlignment::ALL {
                                if ui
                                    .selectable_label(
                                        self.viewport_tab_alignment == alignment,
                                        alignment.label(),
                                    )
                                    .clicked()
                                {
                                    *action = Some(ViewportTabAction::Align(alignment));
                                    ui.close();
                                }
                            }
                        });
                    });
                }
            });
        });
    }

    fn show_viewport_tab_rename(&mut self, context: &egui::Context) {
        let Some(edit) = &mut self.viewport_tab_rename else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        egui::Window::new("Rename viewport")
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                ui.label(format!("Viewport {}", edit.index + 1));
                let response = ui.add(
                    egui::TextEdit::singleline(&mut edit.text)
                        .desired_width(240.0)
                        .hint_text("Viewport title"),
                );
                if edit.focus_requested {
                    response.request_focus();
                    edit.focus_requested = false;
                }
                if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    save = true;
                }
                if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                    cancel = true;
                }
                ui.horizontal(|ui| {
                    save |= ui.button("Rename").clicked();
                    cancel |= ui.button("Cancel").clicked();
                });
            });
        if cancel {
            self.viewport_tab_rename = None;
        } else if save {
            self.commit_viewport_tab_rename();
        }
    }

    fn commit_viewport_tab_rename(&mut self) {
        let Some(edit) = self.viewport_tab_rename.take() else {
            return;
        };
        let title = edit.text.trim();
        if title.is_empty() || title.chars().any(char::is_control) {
            self.push_log("Error: Viewport title must contain printable text".into());
            self.viewport_tab_rename = Some(edit);
        } else if let Some(viewport) = self.viewports.get_mut(edit.index) {
            viewport.set_view_title(title);
            self.push_log(format!("Viewport title: {title}"));
        }
    }

    fn apply_viewport_tab_action(&mut self, action: ViewportTabAction) {
        match action {
            ViewportTabAction::Select(index) => self.activate_model_viewport(index),
            ViewportTabAction::Cycle(next) => self.apply_interface_command(if next {
                InterfaceCommand::NextViewport
            } else {
                InterfaceCommand::PrevViewport
            }),
            ViewportTabAction::Align(alignment) => self.set_viewport_tab_alignment(alignment),
            ViewportTabAction::Rename(index) => {
                if let Some(viewport) = self.viewports.get(index) {
                    self.viewport_tab_rename = Some(ViewportTabRename {
                        index,
                        text: viewport.view_label().to_owned(),
                        focus_requested: true,
                    });
                    self.activate_model_viewport(index);
                }
            }
            ViewportTabAction::Maximize(index) => {
                let restore = self.maximized_viewport == Some(index);
                self.activate_model_viewport(index);
                if !restore {
                    self.maximized_viewport = None;
                }
                self.apply_interface_command(InterfaceCommand::MaxViewport);
            }
            ViewportTabAction::Close(index) => {
                self.activate_model_viewport(index);
                self.close_active_viewport();
            }
            ViewportTabAction::New => self.new_viewport(),
        }
    }

    pub(super) fn new_viewport(&mut self) {
        let source_index = self.active_viewport;
        let source = &self.viewports[source_index];
        let mut viewport = Viewport::new_for_layout(source, ViewKind::Top);
        viewport.new_viewport_parent = Some(source_index);
        self.viewports.push(viewport);
        self.viewport_positions.push([0.25, 0.75, 0.25, 0.75]);
        self.active_viewport = self.viewports.len() - 1;
        self.maximized_viewport = None;
        self.push_log(format!("Created viewport {} (Top)", self.viewports.len()));
    }

    pub(super) fn try_run_viewport_properties_command(&mut self, input: &str) -> bool {
        let end = input.find(char::is_whitespace).unwrap_or(input.len());
        let command = input[..end].trim_start_matches(['\'', '_', '-']);
        if !command.eq_ignore_ascii_case("ViewportProperties") {
            return false;
        }
        self.push_log(format!("> {input}"));
        let result = (|| {
            let tail = input[end..].trim();
            let option_end = tail
                .find(|character: char| character.is_whitespace() || character == '=')
                .unwrap_or(tail.len());
            let option = tail[..option_end].trim_start_matches('_');
            if !option.eq_ignore_ascii_case("Title") {
                return Err("Usage: -ViewportProperties Title name".to_owned());
            }
            let raw = tail[option_end..].trim_start();
            let raw = raw.strip_prefix('=').unwrap_or(raw).trim();
            let title = if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
                &raw[1..raw.len() - 1]
            } else if raw.contains('"') {
                return Err("Usage: -ViewportProperties Title name".to_owned());
            } else {
                raw
            };
            let title = title.trim();
            if title.is_empty() || title.chars().any(char::is_control) {
                return Err("Viewport title must contain printable text".to_owned());
            }
            self.viewports[self.active_viewport].set_view_title(title);
            Ok(format!("Viewport title: {title}"))
        })();
        match result {
            Ok(message) => {
                self.push_log(message);
                self.command_input.clear();
            }
            Err(message) => self.push_log(format!("Error: {message}")),
        }
        true
    }

    pub(super) fn split_active_viewport(&mut self, command: InterfaceCommand) {
        let horizontal = command == InterfaceCommand::SplitViewportHorizontal;
        let index = self.active_viewport;
        let Some((first, second)) = split_rect(self.viewport_positions[index], horizontal) else {
            self.push_log("Error: Active viewport is too narrow to split".into());
            return;
        };
        let title = unique_viewport_title(&self.viewports, self.viewports[index].view_label());
        let duplicate = self.viewports[index].duplicate_for_layout(&title);
        self.viewport_positions[index] = first;
        self.viewport_positions.push(second);
        self.viewports.push(duplicate);
        self.maximized_viewport = None;
        self.push_log(format!(
            "Split viewport {} {}; created viewport {} ({title})",
            index + 1,
            if horizontal {
                "horizontally"
            } else {
                "vertically"
            },
            self.viewports.len()
        ));
    }

    pub(super) fn close_active_viewport(&mut self) {
        if self.viewports.len() == 1 {
            self.push_log("Error: Cannot close the last viewport".into());
            return;
        }
        let removed = self.active_viewport;
        let closed_title = self.viewports[removed].view_label().to_owned();
        let parent = self.viewports[removed]
            .new_viewport_parent
            .and_then(|index| remap_viewport_index(index, removed));
        self.viewport_positions = positions_after_close(&self.viewport_positions, removed);
        self.viewports.remove(removed);
        self.viewport_tab_rename = self.viewport_tab_rename.take().and_then(|mut edit| {
            edit.index = remap_viewport_index(edit.index, removed)?;
            Some(edit)
        });
        for viewport in &mut self.viewports {
            viewport.new_viewport_parent = viewport
                .new_viewport_parent
                .and_then(|index| remap_viewport_index(index, removed));
        }
        self.active_viewport = parent.unwrap_or_else(|| removed.min(self.viewports.len() - 1));
        self.maximized_viewport = self
            .maximized_viewport
            .and_then(|index| remap_viewport_index(index, removed));
        self.zoom_factor_pending = self
            .zoom_factor_pending
            .and_then(|index| remap_viewport_index(index, removed));
        self.snap_size_pending = self.snap_size_pending.and_then(|(target, index)| {
            remap_viewport_index(index, removed).map(|index| (target, index))
        });
        self.zoom_target = self.zoom_target.and_then(|state| match state {
            ZoomTargetState::PickTarget => Some(state),
            ZoomTargetState::PickWindow { target, viewport } => {
                remap_viewport_index(viewport, removed)
                    .map(|viewport| ZoomTargetState::PickWindow { target, viewport })
            }
        });
        self.circular_selection = self.circular_selection.and_then(|state| match state {
            CircularSelectionState::PickCenter(_) => Some(state),
            CircularSelectionState::PickRadius {
                mode,
                center,
                viewport,
            } => remap_viewport_index(viewport, removed).map(|viewport| {
                CircularSelectionState::PickRadius {
                    mode,
                    center,
                    viewport,
                }
            }),
        });
        self.fence_selection = self.fence_selection.take().and_then(|mut state| {
            state.viewport = match state.viewport {
                Some(index) => Some(remap_viewport_index(index, removed)?),
                None => None,
            };
            Some(state)
        });
        self.lasso_selection = self.lasso_selection.take().and_then(|mut state| {
            state.viewport = match state.viewport {
                Some(index) => Some(remap_viewport_index(index, removed)?),
                None => None,
            };
            Some(state)
        });
        self.selection_menu = self.selection_menu.take().and_then(|mut menu| {
            menu.choice.viewport = remap_viewport_index(menu.choice.viewport, removed)?;
            Some(menu)
        });
        self.plane_prompt = self.plane_prompt.take().and_then(|mut prompt| {
            prompt.viewport = remap_viewport_index(prompt.viewport, removed)?;
            Some(prompt)
        });
        self.push_log(format!(
            "Closed viewport {closed_title}; {} viewport(s) remain",
            self.viewports.len()
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_can_reactivate_covered_views_with_duplicate_titles() {
        let mut app = super::super::tests::test_app();
        app.apply_viewport_tab_action(ViewportTabAction::New);
        app.apply_viewport_tab_action(ViewportTabAction::New);
        assert_eq!(app.viewports[0].view_label(), "Top");
        assert_eq!(app.viewports[4].view_label(), "Top");
        assert_eq!(app.viewports[5].view_label(), "Top");
        assert_eq!(app.active_viewport, 5);
        app.apply_viewport_tab_action(ViewportTabAction::Select(4));
        assert_eq!(app.active_viewport, 4);
        app.apply_viewport_tab_action(ViewportTabAction::Select(0));
        assert_eq!(app.active_viewport, 0);
        assert_eq!(app.viewports.len(), 6);
    }

    #[test]
    fn tabs_switch_maximized_view_and_close_selected_view() {
        let mut app = super::super::tests::test_app();
        app.apply_viewport_tab_action(ViewportTabAction::Maximize(0));
        assert_eq!(app.maximized_viewport, Some(0));
        app.apply_viewport_tab_action(ViewportTabAction::Select(2));
        assert_eq!(app.maximized_viewport, Some(2));
        app.apply_viewport_tab_action(ViewportTabAction::Maximize(1));
        assert_eq!(app.maximized_viewport, Some(1));
        app.apply_viewport_tab_action(ViewportTabAction::Maximize(1));
        assert_eq!(app.maximized_viewport, None);
        app.apply_viewport_tab_action(ViewportTabAction::Close(1));
        assert_eq!(app.viewports.len(), 3);
        assert!(app.active_viewport < app.viewports.len());
        assert_eq!(app.maximized_viewport, None);
    }

    #[test]
    fn tab_rename_targets_the_chosen_view_after_another_view_closes() {
        let mut app = super::super::tests::test_app();
        app.apply_viewport_tab_action(ViewportTabAction::New);
        app.apply_viewport_tab_action(ViewportTabAction::Rename(4));
        app.viewport_tab_rename.as_mut().unwrap().text = "Detail".into();
        app.apply_viewport_tab_action(ViewportTabAction::Close(0));
        assert_eq!(app.viewport_tab_rename.as_ref().unwrap().index, 3);
        app.commit_viewport_tab_rename();
        assert_eq!(app.viewports[3].view_label(), "Detail");
        assert_eq!(app.viewports[0].view_label(), "Perspective");
    }

    #[test]
    fn wheel_over_tabs_cycles_views_without_stealing_viewport_scroll() {
        let mut app = super::super::tests::test_app();
        let context = egui::Context::default();
        let frame = |app: &mut VibocerosApp, pointer: egui::Pos2, delta: f32| {
            context
                .run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 600.0),
                        )),
                        events: vec![
                            egui::Event::PointerMoved(pointer),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                delta: egui::vec2(0.0, delta),
                                phase: egui::TouchPhase::Move,
                                modifiers: Default::default(),
                            },
                        ],
                        ..Default::default()
                    },
                    |ui| {
                        let _ = app.show_viewport_tabs(ui);
                    },
                )
                .drop_without_applying_deltas();
        };
        frame(&mut app, egui::pos2(100.0, 100.0), -40.0);
        assert_eq!(app.active_viewport, 0);
        frame(&mut app, egui::pos2(100.0, 580.0), -40.0);
        assert_eq!(app.active_viewport, 1);
        app.maximized_viewport = Some(1);
        frame(&mut app, egui::pos2(100.0, 580.0), 40.0);
        assert_eq!(app.active_viewport, 0);
        assert_eq!(app.maximized_viewport, Some(0));
        app.viewport_tab_alignment = ViewportTabAlignment::Left;
        frame(&mut app, egui::pos2(80.0, 300.0), -40.0);
        assert_eq!(app.active_viewport, 1);
        app.viewport_tab_alignment = ViewportTabAlignment::Right;
        frame(&mut app, egui::pos2(720.0, 300.0), -40.0);
        assert_eq!(app.active_viewport, 2);
        app.viewport_tab_alignment = ViewportTabAlignment::Top;
        frame(&mut app, egui::pos2(100.0, 10.0), 40.0);
        assert_eq!(app.active_viewport, 1);
        assert_eq!(app.maximized_viewport, Some(1));
    }

    #[test]
    fn tab_panel_uses_the_requested_window_edge() {
        let mut app = super::super::tests::test_app();
        let context = egui::Context::default();
        let mut rectangles = Vec::new();
        for alignment in ViewportTabAlignment::ALL {
            app.viewport_tab_alignment = alignment;
            let mut rect = None;
            context
                .run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 600.0),
                        )),
                        ..Default::default()
                    },
                    |ui| rect = app.show_viewport_tabs(ui),
                )
                .drop_without_applying_deltas();
            rectangles.push(rect.unwrap());
        }
        assert!(rectangles[0].center().y > 300.0);
        assert!(rectangles[1].center().y < 300.0);
        assert!(rectangles[2].center().x < 400.0, "{rectangles:?}");
        assert!(rectangles[3].center().x > 400.0);
        assert!(rectangles[2].height() > rectangles[2].width());
        assert!(rectangles[3].height() > rectangles[3].width());
    }
}
