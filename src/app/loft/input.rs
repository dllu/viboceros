use super::*;
use viboceros_command::curve_reference::parse_edge;
use viboceros_drafting::PointInput;

impl VibocerosApp {
    /// Selection's Point option takes precedence over the Point snap override.
    pub(in crate::app) fn continue_loft_point_option(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some()
            || !self.loft_prompt.as_ref().is_some_and(Prompt::selecting)
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
        if matches!(end, End::Finish) && !p.uses_script() {
            self.finish_loft();
        } else {
            self.log_loft();
        }
        Some(true)
    }

    pub(in crate::app) fn continue_loft(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some() || self.loft_prompt.is_none() {
            return false;
        }
        if !self.validate_loft() {
            self.command_input.clear();
            return true;
        }
        let input = input.trim();
        let name = input.trim_start_matches('_');
        let p = self.loft_prompt.as_ref().unwrap();
        if input == "!" || name.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
        } else if name.eq_ignore_ascii_case("Undo") {
            self.undo_loft_input();
        } else if p.picking.is_some() {
            return false; // Typed coordinates, constraints, snaps, transparent commands.
        } else if p.review.as_ref().is_some_and(|review| review.aligning) {
            if input.is_empty() || name.eq_ignore_ascii_case("Enter") {
                self.finish_loft();
            } else if let Some(key) = typed_section(name) {
                self.flip_loft_source(key);
            } else {
                self.log_loft();
            }
        } else if p.review.is_none() && p.seams.is_some() {
            self.continue_loft_seams(name);
        } else if p.selecting() && name.eq_ignore_ascii_case("SelAll") {
            let ids = self
                .document
                .selectable_objects()
                .map(|o| o.id())
                .collect::<Vec<_>>();
            self.select_loft_curves(ids, SelectionMode::Add);
        } else if p.selecting() && name.eq_ignore_ascii_case("SelNone") {
            let keys = p
                .state
                .sources
                .iter()
                .map(|section| (section_key(&section.source), None))
                .collect::<Vec<_>>();
            self.apply_loft_picks(keys, SelectionMode::Remove);
        } else if p.selecting() && edge_value(name).is_some() {
            if let Some(key) = typed_section(name) {
                self.apply_loft_picks([(key, None)], SelectionMode::Add);
            } else {
                self.push_log("Enter Edge=object,index to select a Loft section".into());
            }
        } else if p.selecting() && (p.profile_id_pending || sel_id(name).is_some()) {
            let value = sel_id(name).unwrap_or(name);
            let waiting =
                !name.is_empty() && !name.eq_ignore_ascii_case("Enter") && value.is_empty();
            self.loft_prompt.as_mut().unwrap().profile_id_pending = waiting;
            if let Ok(id) = value.parse::<ObjectId>() {
                self.select_loft_curves([id], SelectionMode::Add);
            } else {
                self.log_loft();
            }
        } else if input.is_empty() || name.eq_ignore_ascii_case("Enter") {
            if p.value.is_some() {
                self.loft_prompt.as_mut().unwrap().value = None;
                self.log_loft();
            } else {
                self.finish_loft();
            }
        } else if p.review.is_some() && name.eq_ignore_ascii_case("AlignCurves") {
            self.align_loft_curves();
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
        } else if let Ok(id) = input.parse::<ObjectId>() {
            self.select_loft_curves([id], SelectionMode::Add);
        } else if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_loft(false);
            return false;
        } else {
            self.log_loft();
        }
        self.command_input.clear();
        true
    }

    fn continue_loft_seams(&mut self, name: &str) {
        let p = self.loft_prompt.as_mut().unwrap();
        if name.is_empty() || name.eq_ignore_ascii_case("Enter") {
            if p.seam_id_pending {
                p.seam_id_pending = false;
                self.log_loft();
            } else {
                self.finish_loft();
            }
            return;
        }
        let editor = &mut p.seams.as_mut().unwrap().editor;
        if edge_value(name).is_some() {
            if let Some(key) = typed_section(name) {
                editor.select_source(self.active_viewport, key);
            }
        } else if p.seam_id_pending || sel_id(name).is_some() {
            let value = sel_id(name).unwrap_or(name);
            p.seam_id_pending = value.is_empty();
            if let Ok(id) = value.parse::<ObjectId>() {
                editor.select_id(self.active_viewport, id);
            }
        } else if let Some(result) = editor.option(name, self.document.tolerance()) {
            if let Err(error) = result {
                self.push_log(error.into());
                return;
            }
            self.loft_snap_to_knots = editor.snap_to_knots();
        } else {
            let parameter = name
                .split_once('=')
                .filter(|(key, _)| key.eq_ignore_ascii_case("Parameter"))
                .and_then(|(_, value)| value.trim().parse::<f64>().ok())
                .filter(|value| value.is_finite());
            let point = PointInput::parse_with_units(name, self.document.units())
                .and_then(Result::ok)
                .and_then(|point| {
                    point
                        .resolve(
                            self.viewports[self.active_viewport].construction_plane(),
                            self.last_point,
                        )
                        .ok()
                });
            if !editor.relocate_target(parameter, point, self.document.tolerance()) {
                self.log_loft();
                return;
            }
        }
        self.log_loft();
    }

    fn undo_loft_input(&mut self) {
        let p = self.loft_prompt.as_mut().unwrap();
        if p.picking.take().is_none() && p.value.take().is_none() {
            if let Some(review) = &mut p.review {
                if review.aligning {
                    review.aligning = false;
                } else if let Some((previous, simplification)) = review.undo.pop() {
                    p.state.options = previous;
                    review.simplification = simplification;
                    self.remember_loft_options();
                    self.refresh_loft_preview();
                    return;
                } else {
                    p.review = None;
                    if p.seams.is_none() {
                        p.state.finished_profiles = false;
                    }
                }
            } else if p.seams.take().is_some() {
                p.state.finished_profiles = false;
            } else if let Some(state) = p.undo.pop() {
                p.state = state;
            }
        }
        p.seam_id_pending = false;
        p.profile_id_pending = false;
        self.active_command = None;
        self.drafting_plane = None;
        self.snaps.model_override = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.sync_loft_selection();
        self.log_loft();
    }

    fn update_loft_options(&mut self, input: &str) {
        let p = self.loft_prompt.as_mut().unwrap();
        let arguments = input.split_whitespace().collect::<Vec<_>>();
        let options = if let Some(review) = &p.review {
            review
                .simplification
                .update(p.state.options, &arguments, self.document.tolerance())
                .map(|(options, values)| (options, Some(values)))
        } else {
            p.state
                .options
                .updated(&arguments)
                .map(|options| (options, None))
        };
        match options {
            Ok((options, _))
                if p.state.sources.len()
                    + usize::from(options.ends.start.is_some())
                    + usize::from(options.ends.end.is_some())
                    > viboceros_geometry::MAX_LOFT_SECTIONS =>
            {
                self.push_log("Too many Loft sections (limit 256 including point ends)".into());
            }
            Ok((options, values))
                if options.ends.is_empty() || options.style != LoftStyle::Uniform =>
            {
                let geometry_changed = options != p.state.options;
                if let Some(review) = &mut p.review {
                    if options.closed && p.state.sources.len() < 3 {
                        self.push_log("Closed loft needs at least three curve sections".into());
                        return;
                    }
                    let values = values.expect("Loft preview options");
                    if geometry_changed || values != review.simplification {
                        review.undo.push((p.state.options, review.simplification));
                        review.simplification = values;
                    }
                } else {
                    p.undo.push(p.state.clone());
                }
                p.state.options = options;
                p.value = None;
                self.remember_loft_options();
                if geometry_changed {
                    self.refresh_loft_preview();
                } else {
                    self.log_loft();
                }
            }
            Ok(_) => self.push_log("Uniform lofts do not accept point ends".into()),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}

fn sel_id(input: &str) -> Option<&str> {
    named_value(input, "SelID")
}

fn edge_value(input: &str) -> Option<&str> {
    named_value(input, "Edge")
}

fn named_value<'a>(input: &'a str, option: &str) -> Option<&'a str> {
    let (name, value) = input
        .split_once(|c: char| c == '=' || c.is_whitespace())
        .unwrap_or((input, ""));
    name.eq_ignore_ascii_case(option).then(|| value.trim())
}

fn typed_section(input: &str) -> Option<SectionKey> {
    if let Some(value) = edge_value(input) {
        let (object, edge) = parse_edge(value)?;
        Some((object, Some(edge)))
    } else {
        Some((sel_id(input)?.parse().ok()?, None))
    }
}
