//! Command-line handling for the mesh offset getters.
use super::*;
use options::{option_parts, toggle};

impl VibocerosApp {
    pub(in crate::app) fn continue_offset_mesh(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some() || self.offset_mesh_session.is_none() {
            return false;
        }
        self.validate_offset_mesh();
        let Some(s) = &mut self.offset_mesh_session else {
            return true;
        };
        let input = input.trim();
        let token = input.trim_start_matches('_');
        if token.eq_ignore_ascii_case("Cancel") || input == "!" {
            if input == "!" || !self.cancel_offset_mesh_flip() {
                self.cancel_interactive_command(true);
            }
            return true;
        }
        if input.is_empty() || token.eq_ignore_ascii_case("Enter") {
            match s.stage {
                Stage::Offset => {
                    self.commit_offset_mesh(None);
                }
                Stage::Flip => {
                    for id in std::mem::take(&mut s.flip_picks) {
                        s.flip_input(id);
                    }
                    s.stage = Stage::Offset;
                    self.update_offset_mesh_preview(None);
                    self.log_offset_mesh();
                }
                Stage::BasePoint | Stage::DirectionMethod => {
                    s.stage = Stage::Offset;
                    self.log_offset_mesh();
                }
            }
            self.command_input.clear();
            return true;
        }
        if s.stage == Stage::Offset
            && let Ok(distance) =
                viboceros_command::offset_surface::distance(input, self.document.units())
        {
            return self.set_offset_mesh_distance(distance);
        }
        if self.try_run_view_command(input) {
            return true;
        }
        if input.split_whitespace().count() > 1 {
            if input
                .split_whitespace()
                .next()
                .is_some_and(|name| self.commands.recognizes(name))
            {
                self.cancel_interactive_command(false);
                return false;
            }
            for word in input.split_whitespace() {
                if self.offset_mesh_session.is_none() || !self.continue_offset_mesh(word) {
                    break;
                }
            }
            return true;
        }
        let s = self.offset_mesh_session.as_mut().unwrap();
        let (key, value) = option_parts(input);
        match (s.stage, key.as_str(), value) {
            (Stage::DirectionMethod, _, None) => match s.preferences.set_method(token) {
                Ok(()) => s.stage = Stage::Offset,
                Err(e) => self.push_log(e.into()),
            },
            (Stage::Offset, "distance", Some(value)) => {
                match viboceros_command::offset_surface::distance(value, self.document.units()) {
                    Ok(distance) => return self.set_offset_mesh_distance(distance),
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            (Stage::Offset, "setbasepoint", None) => s.stage = Stage::BasePoint,
            (Stage::Offset, "directionmethod", None) => s.stage = Stage::DirectionMethod,
            (Stage::Offset, "directionmethod", Some(value)) => {
                if let Err(e) = s.preferences.set_method(value) {
                    self.push_log(e.into());
                }
            }
            (Stage::Offset, "flip", None) if s.preferences.method == Method::Normals => {
                s.stage = Stage::Flip;
            }
            (Stage::Offset, "flip", Some(id)) if s.preferences.method == Method::Normals => {
                self.flip_offset_mesh(id.parse().ok());
            }
            (Stage::Offset, "flipall", value) if s.preferences.method == Method::Normals => {
                match toggle(value, s.flip_all) {
                    Ok(value) => {
                        s.flip_all = value;
                        s.candidate = None;
                    }
                    Err(e) => self.push_log(e.into()),
                }
            }
            (Stage::Flip, _, None) if let Ok(id) = token.parse::<ObjectId>() => {
                self.flip_offset_mesh(Some(id));
            }
            (Stage::Offset, _, value)
                if let Some(result) = s.preferences.set_toggle(&key, value) =>
            {
                if let Err(e) = result {
                    self.push_log(e.into());
                }
            }
            (_, _, _) => {
                if s.wants_point()
                    && (self.try_one_shot_snap(input)
                        || self.try_continue_point_filter(input)
                        || self.try_continue_point_constraint(input)
                        || self.try_continue_point_input(input))
                {
                    return true;
                }
                if self.commands.recognizes(token) {
                    self.cancel_interactive_command(false);
                    return false;
                }
                self.push_log(format!("Error: unexpected OffsetMesh input {input}"));
            }
        }
        self.update_offset_mesh_preview(None);
        self.command_input.clear();
        self.log_offset_mesh();
        true
    }

    fn set_offset_mesh_distance(&mut self, value: Real) -> bool {
        let s = self.offset_mesh_session.as_mut().unwrap();
        if !value.is_finite() || value.abs() <= self.document.tolerance().absolute() {
            self.push_log("Error: enter a nonzero offset distance".into());
        } else {
            s.preferences.distance = value;
            s.cursor = None;
            s.prepare(s.options(None, false));
        }
        self.command_input.clear();
        self.log_offset_mesh();
        true
    }

    fn flip_offset_mesh(&mut self, id: Option<ObjectId>) {
        self.validate_offset_mesh();
        let Some(s) = &mut self.offset_mesh_session else {
            return;
        };
        if let Some(id) = id.filter(|&id| s.sources.ids().any(|source| source == id)) {
            if s.flipping() {
                if !s.flip_picks.remove(&id) {
                    s.flip_picks.insert(id);
                }
            } else {
                s.flip_input(id);
                self.update_offset_mesh_preview(None);
            }
        } else {
            self.push_log("Select a mesh from the offset inputs".into());
        }
    }
}
