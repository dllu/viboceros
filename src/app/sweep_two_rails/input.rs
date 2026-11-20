use super::*;

impl VibocerosApp {
    pub(in crate::app) fn continue_sweep_two_rails(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        let Some(session) = &self.sweep_two_rails else {
            return false;
        };
        let name = input.trim_start_matches(['_', '-']);
        if name.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
        } else if name.eq_ignore_ascii_case("Undo") {
            let session = self.sweep_two_rails.as_mut().unwrap();
            if session.picking.take().is_some() {
                self.finish_sweep_two_rail_point_getter();
            } else if session.value.take().is_none()
                && let Some(state) = session.undo.pop()
            {
                session.state = state;
                self.sync_sweep_two_rail_picks();
            }
            self.log_sweep_two_rails();
        } else if session.picking.is_some() {
            return false;
        } else if let Some(option) = session.value {
            self.set_sweep_two_rail_option(option, input);
        } else if input.is_empty() || name.eq_ignore_ascii_case("Enter") {
            if session.state.review {
                self.commit_sweep_two_rails();
            } else if !session.ready() {
                self.log_sweep_two_rails();
            } else {
                let session = self.sweep_two_rails.as_mut().unwrap();
                session.undo.push(session.state.clone());
                session.state.review = true;
                self.log_sweep_two_rails();
            }
        } else if let Some(word) = name
            .split_whitespace()
            .next()
            .filter(|word| chain_option(word))
        {
            self.start_sweep_two_rail_chain(name[word.len()..].trim());
        } else if let Some((option, value)) = name.split_once('=') {
            if let Some(end) = End::from_name(option) {
                self.set_sweep_two_rail_point_argument(end, name);
            } else {
                self.set_sweep_two_rail_option(option, value);
            }
        } else if let Some(option) = [
            "Simplify",
            "Rebuild",
            "Refit",
            "MaintainHeight",
            "RefitRails",
        ]
        .into_iter()
        .find(|option| option.eq_ignore_ascii_case(name))
        {
            self.sweep_two_rails.as_mut().unwrap().value = Some(option);
            self.log_sweep_two_rails();
        } else if let Ok(id) = name.parse::<ObjectId>() {
            self.select_sweep_two_rail_curves([id], SelectionMode::Add);
        } else if name.eq_ignore_ascii_case("SelAll") && session.selecting() {
            let ids = self
                .document
                .selectable_objects()
                .map(|o| o.id())
                .collect::<Vec<_>>();
            self.select_sweep_two_rail_curves(ids, SelectionMode::Add);
        } else if name.eq_ignore_ascii_case("SelNone") && session.selecting() {
            self.select_sweep_two_rail_curves(session.ids(), SelectionMode::Remove);
        } else if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_object_prompt(false);
            return false;
        } else {
            self.log_sweep_two_rails();
        }
        self.command_input.clear();
        true
    }

    fn set_sweep_two_rail_option(&mut self, option: &str, value: &str) {
        let height = option.eq_ignore_ascii_case("MaintainHeight");
        if !height && !option.eq_ignore_ascii_case("RefitRails") {
            self.set_sweep_two_rail_fit(option, value);
            return;
        }
        match Options::parse_toggle(value) {
            Ok(enabled) => {
                let session = self.sweep_two_rails.as_mut().unwrap();
                session.value = None;
                let mut next = session.state.shape;
                let toggle = if height {
                    &mut next.maintain_height
                } else {
                    &mut next.refit_rails
                };
                if enabled != *toggle {
                    *toggle = enabled;
                    session.undo.push(session.state.clone());
                    session.state.shape = next;
                    session.preview.invalidate();
                }
                self.log_sweep_two_rails();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn set_sweep_two_rail_fit(&mut self, option: &str, value: &str) {
        let session = self.sweep_two_rails.as_mut().unwrap();
        let fit = if option.eq_ignore_ascii_case("Simplify") {
            match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                "rebuild" => Ok(SectionFit::Rebuild(session.state.rebuild)),
                "refit" => Ok(SectionFit::Refit(session.state.refit)),
                _ => SectionFit::parse(option, value),
            }
        } else {
            SectionFit::parse(option, value)
        };
        match fit {
            Ok(fit) => {
                session.value = None;
                if fit != session.state.shape.fit {
                    session.undo.push(session.state.clone());
                    session.state.shape.fit = fit;
                    match fit {
                        SectionFit::Rebuild(count) => session.state.rebuild = count,
                        SectionFit::Refit(distance) => session.state.refit = distance,
                        SectionFit::Unchanged => {}
                    }
                    session.preview.invalidate();
                }
                self.log_sweep_two_rails();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}
