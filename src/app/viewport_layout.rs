//! Docked viewport layout operations.

use super::*;
use viboceros_command::interface::InterfaceCommand;

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
