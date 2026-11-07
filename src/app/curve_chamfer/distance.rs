//! First-distance acceptance retains the measurement anchor for the second getter.
use super::*;
impl VibocerosApp {
    fn accept_curve_chamfer_distance(&mut self, value: Real) {
        let s = self.curve_chamfer_session.as_mut().unwrap();
        let index = if let Some(Pending::Distance(i)) = s.pending {
            i
        } else {
            0
        };
        s.options.distances[index] = value;
        s.pending = if index == 0 {
            Some(Pending::Distance(1))
        } else {
            None
        };
        if index == 1 {
            s.start = None;
        }
        self.active_command = Some(InteractiveCommand::Chamfer { anchor: s.start });
        self.curve_chamfer_prompt();
    }
    pub(in crate::app) fn accept_curve_chamfer_point(&mut self, point: Point3) -> Option<bool> {
        if self.plane_prompt.is_some() {
            return None;
        }
        let s = self.curve_chamfer_session.as_mut()?;
        if !s.wants_point() {
            return Some(false);
        }
        if let Some(start) = s.start {
            match start.distance_to(point) {
                Ok(d) => self.accept_curve_chamfer_distance(d),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return Some(false);
                }
            }
        } else {
            s.start = Some(point);
            self.active_command = Some(InteractiveCommand::Chamfer {
                anchor: Some(point),
            });
            self.curve_chamfer_prompt();
        }
        Some(true)
    }
    pub(super) fn continue_curve_chamfer_distance(&mut self, text: &str) -> bool {
        let s = self.curve_chamfer_session.as_ref().unwrap();
        if text
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Distances")
        {
            let s = self.curve_chamfer_session.as_mut().unwrap();
            s.pending = Some(Pending::Distance(0));
            s.start = None;
            self.active_command = Some(InteractiveCommand::Chamfer { anchor: None });
            self.curve_chamfer_prompt();
            return true;
        }
        if text.is_empty() && s.wants_point() {
            self.accept_curve_chamfer_distance(s.options.distances[0]);
            return true;
        }
        if let Ok(value) = parse_distance(text, self.document.units()) {
            self.accept_curve_chamfer_distance(value);
            return true;
        }
        if s.wants_point() {
            if self.try_continue_point_constraint(text)
                || self.try_continue_point_filter(text)
                || self.try_continue_point_input(text)
            {
                return true;
            }
            self.push_log("Enter a nonnegative chamfer distance or a distance point".into());
            return true;
        }
        false
    }
}
