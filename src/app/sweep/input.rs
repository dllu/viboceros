use super::*;
impl VibocerosApp {
    pub(in crate::app) fn continue_sweep_point_option(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some()
            || self.chain_prompt.is_some()
            || !self.sweep_session.as_ref().is_some_and(|p| {
                p.picking.is_none() && p.value.is_none() && p.seam_editor().is_none()
            })
            || !["Point", "StartPoint", "EndPoint"]
                .iter()
                .any(|n| input.trim_start_matches('_').eq_ignore_ascii_case(n))
        {
            return false;
        }
        self.begin_sweep_point(input);
        self.command_input.clear();
        true
    }
    pub(super) fn begin_sweep_point(&mut self, name: &str) {
        let p = self.sweep_session.as_ref().unwrap();
        if p.state.rail.is_none()
            && (p.state.options.has_explicit_rail() || p.state.sources.is_empty())
        {
            self.push_log("Select or specify a rail before picking a Sweep1 point end".into());
            return;
        }
        let start = name
            .trim_start_matches('_')
            .eq_ignore_ascii_case("StartPoint")
            || (name.trim_start_matches('_').eq_ignore_ascii_case("Point")
                && p.profiles() == 0
                && p.state.options.ends.start.is_none());
        if !start && p.profiles() == 0 {
            self.push_log("Select a profile before picking the Sweep1 end point".into());
            return;
        }
        let ends = usize::from(p.state.options.ends.start.is_some())
            + usize::from(p.state.options.ends.end.is_some());
        let new = if start {
            p.state.options.ends.start.is_none()
        } else {
            p.state.options.ends.end.is_none()
        };
        if p.profiles() + ends + usize::from(new) > viboceros_geometry::Sweep1::MAX_SECTIONS {
            self.push_log("Too many Sweep1 sections (limit 256 including point ends)".into());
            return;
        }
        let p = self.sweep_session.as_mut().unwrap();
        p.picking = Some(if start { End::Start } else { End::Finish });
        p.preview.clear_wires();
        p.value = None;
        self.active_command = Some(InteractiveCommand::SweepPoint);
        self.drafting_plane = Some(self.viewports[self.active_viewport].construction_plane());
        self.push_log(
            if start {
                "Sweep1: pick the start point"
            } else {
                "Sweep1: pick the end point"
            }
            .into(),
        );
    }
    pub(in crate::app) fn accept_sweep_point(&mut self, point: Point3) -> Option<bool> {
        let end = self.sweep_session.as_ref()?.picking?;
        if !self.validate_sweep_sources() {
            return Some(false);
        }
        let p = self.sweep_session.as_mut().unwrap();
        p.undo.push(p.state.clone());
        end.set_point(&mut p.state.options, point);
        if end == End::Finish && p.state.stage.seams().is_none() {
            p.state.stage = ProfileStage::PointEndPicked;
        }
        p.picking = None;
        self.active_command = None;
        self.drafting_plane = None;
        self.sync_sweep_selection();
        self.log_sweep_input();
        Some(true)
    }
    pub(in crate::app) fn continue_sweep_input(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        let Some(p) = self.sweep_session.as_ref() else {
            return false;
        };
        let name = input.trim_start_matches(['_', '-']);
        if name.eq_ignore_ascii_case("Undo") {
            let p = self.sweep_session.as_mut().unwrap();
            if !std::mem::take(&mut p.seam_id_pending)
                && p.picking.take().is_none()
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
            self.sync_sweep_selection();
            self.log_sweep_input();
        } else if name.eq_ignore_ascii_case("Cancel") || name == "!" {
            self.cancel_interactive_command(true);
        } else if p.picking.is_some() {
            return false;
        } else if (name.is_empty() || name.eq_ignore_ascii_case("Enter")) && p.value.is_none() {
            self.finish_sweep_input();
        } else if p.seam_editor().is_some() {
            self.continue_sweep_seams(name);
        } else if p.value.is_none()
            && let (name, options) = name.split_once(char::is_whitespace).unwrap_or((name, ""))
            && super::rail_picks::is_chain_option(name)
        {
            self.start_sweep_one_rail_chain(options);
        } else if name.eq_ignore_ascii_case("SelAll") && p.selecting() {
            let ids = self
                .document
                .selectable_objects()
                .map(|o| o.id())
                .collect::<Vec<_>>();
            self.select_sweep_objects(ids, SelectionMode::Add);
        } else if name.eq_ignore_ascii_case("SelNone") && p.selecting() {
            let ids = p
                .state
                .sources
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            self.select_sweep_objects(ids, SelectionMode::Remove);
        } else if let Some(option) = p.value {
            self.update_sweep_options(&format!("{option}={input}"));
        } else if let Some(option) = Options::VALUE_OPTIONS
            .iter()
            .copied()
            .find(|n| name.eq_ignore_ascii_case(n))
        {
            let hint = if ["Style", "FrameStyle"].contains(&option) {
                if p.state.rail.as_ref().is_some_and(Rail::supports_alignment) {
                    "Sweep1 style options are Freeform, Roadlike, and AlignWithSurface".into()
                } else {
                    "Sweep1 style options are Freeform and Roadlike".into()
                }
            } else if option == "ShapeBlending" {
                "Sweep1 shape blending options are Local and Global".into()
            } else {
                format!("Sweep1: enter {option}")
            };
            self.sweep_session.as_mut().unwrap().value = Some(option);
            self.push_log(hint);
        } else if let Some((key, value)) = name.split_once('=')
            && key.eq_ignore_ascii_case("Edge")
            && p.awaiting_rail()
        {
            self.update_sweep_options(&format!("RailEdge={value}"));
        } else if input.contains('=') {
            self.update_sweep_options(input);
        } else if let Ok(id) = input.parse::<ObjectId>() {
            self.select_sweep_objects([id], SelectionMode::Add);
        } else if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_object_prompt(false);
            return false;
        } else {
            self.log_sweep_input();
        }
        self.command_input.clear();
        true
    }
    pub(super) fn update_sweep_options(&mut self, input: &str) {
        let p = self.sweep_session.as_ref().unwrap();
        match p
            .state
            .options
            .updated(&input.split_whitespace().collect::<Vec<_>>())
        {
            Ok(options) => {
                // Unrelated option edits must not refresh a stale rail snapshot.
                let replace_rail = input
                    .split_whitespace()
                    .filter_map(|w| w.split_once('='))
                    .any(|(name, _)| {
                        ["RailId", "RailName", "RailEdge"]
                            .iter()
                            .any(|n| name.trim_start_matches(['_', '-']).eq_ignore_ascii_case(n))
                    });
                let mut next = p.state.clone();
                next.options = options;
                if replace_rail {
                    match self.capture_sweep_reference(&next.options) {
                        Ok((reference, rail)) => {
                            next.rail = Some(rail);
                            next.reference = Some(reference);
                        }
                        Err(error) => {
                            self.push_log(format!("Error: {error}"));
                            return;
                        }
                    }
                }
                if next.stage.seams().is_some()
                    && (replace_rail
                        || (next.rail.is_none()
                            && next.options.parameters.is_some()
                                != p.state.options.parameters.is_some()))
                {
                    next.stage = ProfileStage::Selecting;
                }
                if next.options.surface_aligned
                    && (next
                        .rail
                        .as_ref()
                        .is_some_and(|rail| !rail.supports_alignment())
                        || (next.rail.is_none() && !next.sources.is_empty()))
                {
                    self.push_log(
                        "Sweep1 AlignWithSurface requires an edge with one supporting face".into(),
                    );
                    return;
                }
                let p = self.sweep_session.as_mut().unwrap();
                let old = p.state.clone();
                p.state = next;
                if p.profiles()
                    + usize::from(p.state.options.ends.start.is_some())
                    + usize::from(p.state.options.ends.end.is_some())
                    > viboceros_geometry::Sweep1::MAX_SECTIONS
                {
                    p.state = old;
                    self.push_log(
                        "Too many Sweep1 sections (limit 256 including point ends)".into(),
                    );
                    return;
                }
                p.undo.push(old);
                p.value = None;
                self.sync_sweep_selection();
                self.log_sweep_input();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}
