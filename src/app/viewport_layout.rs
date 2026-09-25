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
}
