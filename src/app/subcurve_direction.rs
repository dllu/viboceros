//! Cursor direction is transient command input, separate from document history.
use super::*;
impl VibocerosApp {
    pub(super) fn continue_subcurve_direction(&mut self, input: &str) -> bool {
        let input = input.trim().trim_start_matches('_');
        let locked = if input.eq_ignore_ascii_case("D") {
            Some(true)
        } else if let Some((name, value)) = input.split_once('=') {
            if !name.eq_ignore_ascii_case("Direction") {
                return false;
            }
            if value.trim_start_matches('_').eq_ignore_ascii_case("Locked") {
                Some(true)
            } else if value.trim_start_matches('_').eq_ignore_ascii_case("Free") {
                Some(false)
            } else {
                None
            }
        } else {
            return false;
        };
        let reference = self
            .subcurve_prompt
            .as_ref()
            .and_then(|p| Some((p.source?, p.start?, p.hover_parameter)))
            .or_else(|| {
                let p = self
                    .intersection_prompt
                    .as_ref()?
                    .uv_subcurves
                    .pending
                    .as_ref()?;
                Some((p.object?, p.start?, p.hover_parameter))
            });
        let Some((source, start, hover)) = reference else {
            return false;
        };
        let Some(locked) = locked else {
            self.push_log("Direction expects Free or Locked".into());
            return true;
        };
        let forward = if locked {
            let Some(hover) = hover else {
                self.push_log("Move the cursor along the curve before locking direction".into());
                return true;
            };
            let result = self
                .document
                .object(source)
                .and_then(|o| o.geometry().curve_ref())
                .map(|c| viboceros_command::subcurve_input::cursor_forward(c, start, hover));
            match result {
                Some(Ok(forward)) => Some(forward),
                Some(Err(error)) => {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
                None => return true,
            }
        } else {
            None
        };
        if let Some(p) = self.subcurve_prompt.as_mut() {
            p.locked_forward = forward;
        } else if let Some(p) = self
            .intersection_prompt
            .as_mut()
            .and_then(|p| p.uv_subcurves.pending.as_mut())
        {
            p.locked_forward = forward;
        }
        self.command_input.clear();
        self.push_log(format!(
            "Direction={}",
            if locked { "Locked" } else { "Free" }
        ));
        if let Some(prompt) = self.subcurve_prompt.as_ref() {
            self.push_log(prompt.hint().into());
        } else {
            self.log_intersection_prompt();
        }
        true
    }
    pub(super) fn update_subcurve_hover(&mut self, point: Point3) {
        let reference = self
            .subcurve_prompt
            .as_ref()
            .and_then(|p| p.source.zip(p.start))
            .or_else(|| {
                let pending = self
                    .intersection_prompt
                    .as_ref()?
                    .uv_subcurves
                    .pending
                    .as_ref()?;
                pending.object.zip(pending.start)
            });
        let Some((source, _)) = reference else {
            return;
        };
        let parameter = self
            .document
            .object(source)
            .and_then(|o| o.geometry().curve_ref())
            .and_then(|c| c.closest_parameter(point, self.document.tolerance()).ok());
        if let Some(p) = self.subcurve_prompt.as_mut() {
            p.hover_parameter = parameter;
        } else if let Some(p) = self
            .intersection_prompt
            .as_mut()
            .and_then(|p| p.uv_subcurves.pending.as_mut())
        {
            p.hover_parameter = parameter;
        }
    }
}
