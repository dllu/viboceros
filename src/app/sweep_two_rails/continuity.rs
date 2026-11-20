use super::*;
use viboceros_command::sweep_two_rails::{EDGE_CONTINUITY_NAMES, EDGE_END_NAMES, EdgeOptions};

impl Session {
    pub(super) fn can_match_edges(&self) -> bool {
        self.state.review
            && self.state.seams.is_none()
            && self.state.rails.iter().any(Rail::has_surface_support)
            && (self.state.shape.edge.active()
                || self.state.shape.fit != SectionFit::Unchanged
                || self.state.picks.iter().all(|pick| pick.polynomial))
    }
}

impl VibocerosApp {
    pub(super) fn continue_sweep_two_rail_edge_options(&mut self, input: &str) -> bool {
        let session = self.sweep_two_rails.as_ref().unwrap();
        let is_menu = |name: &str| matches!(name, "EdgeContinuity" | "PreserveEnds");
        let is_choice =
            |name: &str| EDGE_CONTINUITY_NAMES.contains(&name) || EDGE_END_NAMES.contains(&name);
        let current = session.value;
        // Leave other pending option values and the closed-profile editor to their own getters.
        if session.seam_editor().is_some()
            || current.is_some_and(|name| !is_menu(name) && !is_choice(name))
        {
            return false;
        }
        let (name, value) = input
            .split_once('=')
            .map_or((input, None), |(name, value)| (name, Some(value)));
        let named = ["EdgeContinuity", "PreserveEnds"]
            .into_iter()
            .chain(EDGE_CONTINUITY_NAMES)
            .chain(EDGE_END_NAMES)
            .find(|option| option.eq_ignore_ascii_case(name));
        if current.is_none() && named.is_none() {
            return false;
        }
        if !session.can_match_edges() {
            self.push_log("Sweep2: edge continuity is available in review for surface-edge rails and open polynomial profiles".into());
            return true;
        }
        if input.is_empty() || input.eq_ignore_ascii_case("Enter") {
            self.sweep_two_rails.as_mut().unwrap().value = None;
        } else if let Some((choice, value)) =
            named.filter(|name| is_choice(name)).zip(value).or_else(|| {
                current
                    .filter(|name| is_choice(name))
                    .map(|name| (name, input))
            })
        {
            self.apply_sweep_two_rail_edge_option(choice, value);
            return true;
        } else if let Some(option) = named {
            self.sweep_two_rails.as_mut().unwrap().value = Some(option);
        }
        self.log_sweep_two_rails();
        true
    }

    fn apply_sweep_two_rail_edge_option(&mut self, option: &str, value: &str) {
        let session = self.sweep_two_rails.as_mut().unwrap();
        let mut next = session.state.shape.edge;
        let result = if let Some(side) = EDGE_CONTINUITY_NAMES
            .iter()
            .position(|&name| name == option)
        {
            EdgeOptions::parse_mode(value).map(|mode| next.continuity[side] = mode)
        } else {
            let end = EDGE_END_NAMES
                .iter()
                .position(|&name| name == option)
                .unwrap();
            Options::parse_toggle(value).map(|adjust| next.adjust_ends[end] = adjust)
        };
        if let Err(error) = result {
            self.push_log(format!("Error: {error}"));
            return;
        }
        session.value = None;
        if next != session.state.shape.edge {
            session.undo.push(session.state.clone());
            session.state.shape.edge = next;
            session.preview.invalidate();
        }
        self.log_sweep_two_rails();
    }
}
