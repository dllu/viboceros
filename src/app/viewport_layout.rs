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
                .skip(index + 1)
                .all(|other| !rectangles_overlap(*position, *other))
        }) {
            return result;
        }
    }
    super::named_view::default_viewport_positions(remaining.len())
}

impl VibocerosApp {
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
        self.viewport_positions = positions_after_close(&self.viewport_positions, removed);
        self.viewports.remove(removed);
        self.active_viewport = removed.min(self.viewports.len() - 1);
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
