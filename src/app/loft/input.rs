use super::*;

impl VibocerosApp {
    /// Selection's Point option takes precedence over the Point snap override.
    pub(in crate::app) fn continue_loft_point_option(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some()
            || !self
                .loft_prompt
                .as_ref()
                .is_some_and(|p| p.picking.is_none() && p.value.is_none())
            || !["Point", "StartPoint", "EndPoint"]
                .iter()
                .any(|name| input.trim_start_matches('_').eq_ignore_ascii_case(name))
        {
            return false;
        }
        self.begin_loft_point(input);
        self.command_input.clear();
        true
    }

    pub(super) fn begin_loft_point(&mut self, name: &str) {
        let p = self.loft_prompt.as_mut().unwrap();
        if p.state.options.closed || p.state.options.style == LoftStyle::Uniform {
            self.push_log(
                "Loft point ends require Closed=No and a style other than Uniform".into(),
            );
            return;
        }
        let name = name.trim_start_matches('_');
        let end = if name.eq_ignore_ascii_case("StartPoint")
            || (name.eq_ignore_ascii_case("Point")
                && p.state.sources.is_empty()
                && p.state.options.ends.start.is_none())
        {
            End::Start
        } else {
            End::Finish
        };
        if matches!(end, End::Finish) && p.state.sources.is_empty() {
            self.push_log("Select a curve before picking the Loft end point".into());
            return;
        }
        let count = p.state.sources.len()
            + usize::from(p.state.options.ends.start.is_some())
            + usize::from(p.state.options.ends.end.is_some());
        let new_end = match end {
            End::Start => p.state.options.ends.start.is_none(),
            End::Finish => p.state.options.ends.end.is_none(),
        };
        if count + usize::from(new_end) > viboceros_geometry::MAX_LOFT_SECTIONS {
            self.push_log("Too many Loft sections (limit 256 including point ends)".into());
            return;
        }
        p.picking = Some(end);
        p.value = None;
        self.active_command = Some(InteractiveCommand::LoftPoint);
        self.drafting_plane = Some(self.viewports[self.active_viewport].construction_plane());
        self.log_loft();
    }

    pub(in crate::app) fn accept_loft_point(&mut self, point: Point3) -> Option<bool> {
        let p = self.loft_prompt.as_mut()?;
        let end = p.picking?;
        p.undo.push(p.state.clone());
        match end {
            End::Start => p.state.options.ends.start = Some(point),
            End::Finish => {
                p.state.options.ends.end = Some(point);
                p.state.finished_profiles = true;
            }
        }
        p.picking = None;
        self.active_command = None;
        self.drafting_plane = None;
        self.log_loft();
        Some(true)
    }

    pub(in crate::app) fn continue_loft(&mut self, input: &str) -> bool {
        let Some(p) = self.loft_prompt.as_ref() else {
            return false;
        };
        let name = input.trim_start_matches(['_', '-']);
        if name.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
        } else if name.eq_ignore_ascii_case("Undo") {
            let p = self.loft_prompt.as_mut().unwrap();
            if p.picking.take().is_none()
                && p.value.take().is_none()
                && let Some(state) = p.undo.pop()
            {
                p.state = state;
            }
            self.active_command = None;
            self.drafting_plane = None;
            self.snaps.model_override = None;
            self.point_filter = None;
            self.point_constraint = None;
            self.sync_loft_selection();
            self.log_loft();
        } else if p.picking.is_some() {
            return false; // Typed coordinates, constraints, snaps, transparent commands.
        } else if p.selecting() && name.eq_ignore_ascii_case("SelAll") {
            let ids = self
                .document
                .selectable_objects()
                .map(|o| o.id())
                .collect::<Vec<_>>();
            self.select_loft_curves(ids, SelectionMode::Add);
        } else if p.selecting() && name.eq_ignore_ascii_case("SelNone") {
            let ids = p
                .state
                .sources
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            self.select_loft_curves(ids, SelectionMode::Remove);
        } else if input.is_empty() && p.value.is_none() {
            self.finish_loft();
        } else if let Some(option) = p.value {
            self.update_loft_options(&format!("{option}={input}"));
        } else if let Some(option) = [
            "Type",
            "Closed",
            "SplitAtTangents",
            "Rebuild",
            "RebuildPointCount",
            "RebuildCount",
            "Simplify",
            "Refit",
            "RefitTolerance",
        ]
        .into_iter()
        .find(|option| name.eq_ignore_ascii_case(option))
        {
            self.loft_prompt.as_mut().unwrap().value = Some(option);
            self.log_loft();
        } else if input.contains('=') || name.eq_ignore_ascii_case("Natural") {
            self.update_loft_options(input);
        } else if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_loft(false);
            return false;
        } else if let Ok(id) = input.parse::<ObjectId>() {
            self.select_loft_curves([id], SelectionMode::Add);
        } else {
            self.log_loft();
        }
        self.command_input.clear();
        true
    }

    fn update_loft_options(&mut self, input: &str) {
        let p = self.loft_prompt.as_mut().unwrap();
        match p
            .state
            .options
            .updated(&input.split_whitespace().collect::<Vec<_>>())
        {
            Ok(options)
                if p.state.sources.len()
                    + usize::from(options.ends.start.is_some())
                    + usize::from(options.ends.end.is_some())
                    > viboceros_geometry::MAX_LOFT_SECTIONS =>
            {
                self.push_log("Too many Loft sections (limit 256 including point ends)".into());
            }
            Ok(options) if options.ends.is_empty() || options.style != LoftStyle::Uniform => {
                p.undo.push(p.state.clone());
                p.state.options = options;
                p.value = None;
                self.log_loft();
            }
            Ok(_) => self.push_log("Uniform lofts do not accept point ends".into()),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}
