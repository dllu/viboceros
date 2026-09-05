//! Route typed and picked points through the same interactive command state.

use super::*;
use viboceros_drafting::PointInput;

impl VibocerosApp {
    pub(super) fn try_continue_point_input(&mut self, input: &str) -> bool {
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            return false;
        }
        let Some(parsed) = PointInput::parse(input) else {
            return false;
        };
        let point = parsed.and_then(|input| {
            input.resolve(
                self.viewports[self.active_viewport].construction_plane(),
                self.last_point,
            )
        });
        match point {
            Ok(point) => {
                self.push_log(format!("> {input}"));
                self.accept_drafting_point(point);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn accept_drafting_point(&mut self, point: Point3) -> bool {
        if self.apply_drafting_point(point) {
            self.last_point = Some(point);
            self.command_input.clear();
            true
        } else {
            false
        }
    }
}
