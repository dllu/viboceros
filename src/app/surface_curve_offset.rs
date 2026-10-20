//! Source/face getters and cached, nonmutating surface-offset previews.
use super::*;
use viboceros_command::{
    curve_reference::CapturedCurve,
    orient_surface::Target,
    surface_curve_offset::{self as solver, Options, Plan, PointSetPlan, Source},
};
use viboceros_geometry::NurbsCurve;

pub(super) struct Session {
    group: viboceros_document::HistoryGroup,
    source: Option<Source>,
    target: Option<Target>,
    side: Option<Point3>,
    options: Options,
    plan: Option<Plan>,
    number_prompt: bool,
    subcurve: Option<super::uv_subcurve_input::PendingSubcurve>,
    subcurve_source: Option<CapturedCurve>,
    draft: Vec<NurbsCurve>,
    through: bool,
    through_cursor: Option<Point3>,
    selected_points: Option<Vec<ObjectId>>,
}
impl Session {
    pub(super) fn cursor_surface(&self) -> Option<(ObjectId, Option<usize>, bool)> {
        let target = self
            .target
            .as_ref()
            .filter(|_| self.through && self.selected_points.is_none())?;
        Some((target.id, Some(target.face), false))
    }
    pub(super) fn preview_curves(&self) -> &[NurbsCurve] {
        self.plan
            .as_ref()
            .map_or(&self.draft, |p| p.curves.as_slice())
    }
    pub(super) fn cursor_curve(&self) -> Option<&NurbsCurve> {
        self.subcurve_source.as_ref().map(CapturedCurve::curve)
    }
    fn current(&self, doc: &Document) -> bool {
        doc.history_group_is_current(&self.group)
            && self.source.as_ref().is_none_or(|s| s.current(doc))
            && self.target.as_ref().is_none_or(|t| t.current(doc))
            && self.subcurve_source.as_ref().is_none_or(|s| s.current(doc))
    }
}
impl VibocerosApp {
    pub(super) fn picking_surface_offset_points(&self) -> bool {
        self.surface_offset_session
            .as_ref()
            .is_some_and(|s| s.selected_points.is_some())
    }
    pub(super) fn surface_offset_selected_points(&self) -> &[ObjectId] {
        self.surface_offset_session
            .as_ref()
            .and_then(|s| s.selected_points.as_deref())
            .unwrap_or_default()
    }
    pub(super) fn pick_surface_offset_points(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) -> bool {
        if !self.picking_surface_offset_points() {
            return false;
        }
        if !self.validate_surface_offset() {
            return true;
        }
        let points = self
            .surface_offset_session
            .as_mut()
            .unwrap()
            .selected_points
            .as_mut()
            .unwrap();
        let mut next = points.clone();
        for id in ids {
            if !self.document.is_object_selectable(id)
                || !self
                    .document
                    .object(id)
                    .is_some_and(|o| matches!(o.geometry(), Geometry::Point(_)))
            {
                continue;
            }
            let index = next.iter().position(|p| *p == id);
            match (mode, index) {
                (SelectionMode::Remove | SelectionMode::Toggle, Some(index)) => {
                    next.remove(index);
                }
                (SelectionMode::Remove, None) => {}
                (_, None) if next.len() < solver::MAX_POINTS => next.push(id),
                (_, None) => {
                    self.push_log(format!(
                        "Select at most {} point objects",
                        solver::MAX_POINTS
                    ));
                    return true;
                }
                _ => {}
            }
        }
        *points = next;
        self.surface_offset_prompt();
        true
    }
    pub(super) fn picking_surface_offset_curve(&self) -> bool {
        self.surface_offset_session
            .as_ref()
            .is_some_and(|s| s.source.is_none() && s.subcurve_source.is_none())
    }
    pub(super) fn picking_surface_offset_face(&self) -> bool {
        self.surface_offset_session
            .as_ref()
            .is_some_and(|s| s.source.is_some() && s.target.is_none())
    }
    pub(super) fn start_surface_offset(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("OffsetCrvOnSrf")
        }) {
            return false;
        }
        let Ok((options, refs, distance)) = solver::options(
            &words[1..],
            Options {
                flip: false,
                ..self.surface_offset_defaults
            },
        ) else {
            return false;
        };
        if refs.through.is_some() || refs.points.is_some() || distance && refs.surface.is_some() {
            return false;
        }
        let selected = refs.source.or_else(|| {
            let ids = self
                .document
                .selected_objects()
                .filter(|o| o.geometry().curve_ref().is_some())
                .map(|o| o.id())
                .collect::<Vec<_>>();
            if let [id] = ids.as_slice() {
                Some(*id)
            } else {
                None
            }
        });
        self.cancel_interactive_command(false);
        let source = selected
            .map(|id| Source::capture(&self.document, id, refs.edge, refs.range))
            .transpose();
        let source = match source {
            Ok(s) => s,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let target = refs
            .surface
            .map(|id| Target::capture(&self.document, id, refs.face))
            .transpose();
        let target = match target {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        match self.document.begin_history_group("OffsetCrvOnSrf") {
            Ok(group) => {
                self.surface_offset_session = Some(Session {
                    group,
                    source,
                    target,
                    side: refs.side,
                    options,
                    plan: None,
                    number_prompt: false,
                    subcurve: None,
                    subcurve_source: None,
                    draft: Vec::new(),
                    through: refs.through_mode,
                    through_cursor: refs.through,
                    selected_points: None,
                });
                self.active_command = Some(InteractiveCommand::OffsetCurveOnSurface);
                self.prepare_surface_offset();
                self.surface_offset_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    fn surface_offset_prompt(&mut self) {
        let Some(s) = &self.surface_offset_session else {
            return;
        };
        let message = if let Some(p) = &s.subcurve {
            if s.subcurve_source.is_none() {
                "Select curve to temporarily shorten".into()
            } else if p.start.is_none() {
                "Start of temporary curve".into()
            } else if p.length.is_some() {
                "Confirm length direction on the curve; Direction=Locked|Free".into()
            } else {
                "End of temporary curve or curve length; Direction=Locked|Free".into()
            }
        } else if s.source.is_none() {
            "Select one curve or surface edge (SubCrv)".into()
        } else if s.target.is_none() {
            "Select the base surface on the offset side".into()
        } else if let Some(points) = &s.selected_points {
            format!(
                "Select point objects ({} selected); Enter creates offsets",
                points.len()
            )
        } else if s.through {
            let select = if s.group.can_undo() { "" } else { " (Select)" };
            format!("Through point on the surface; Enter finishes{select}")
        } else if s.number_prompt {
            format!("Number of curves <{}>", s.options.number)
        } else {
            format!(
                "Offset distance <{}> (ThroughPoint Flip={} Number={})",
                s.options.distance,
                if s.options.flip { "Yes" } else { "No" },
                s.options.number
            )
        };
        self.push_log(format!("OffsetCrvOnSrf: {message}"));
    }
    fn prepare_surface_offset(&mut self) {
        self.prepare_surface_offset_with_log(true);
    }
    fn prepare_surface_offset_with_log(&mut self, announce: bool) {
        let Some(s) = self.surface_offset_session.as_mut() else {
            return;
        };
        s.plan = None;
        if s.selected_points.is_some() {
            return;
        }
        let (Some(source), Some(target)) = (&s.source, &s.target) else {
            return;
        };
        let result = if s.through {
            let Some(point) = s.through_cursor else {
                return;
            };
            Plan::prepare_through_point(
                &self.document,
                source.clone(),
                target.clone(),
                point,
                s.options,
            )
        } else {
            Plan::prepare(
                &self.document,
                source.clone(),
                target.clone(),
                s.side,
                s.options,
            )
        };
        match result {
            Ok(plan) => s.plan = Some(plan),
            Err(e) if announce => self.push_log(format!("Error: {e}")),
            Err(_) => {}
        }
    }
    pub(super) fn validate_surface_offset(&mut self) -> bool {
        let Some(s) = &self.surface_offset_session else {
            return true;
        };
        if s.current(&self.document) {
            return true;
        }
        self.cancel_interactive_command(false);
        self.push_log(
            "Source, surface, or command history changed; start OffsetCrvOnSrf again".into(),
        );
        false
    }
    pub(super) fn pick_surface_offset_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if !self.picking_surface_offset_curve() {
            return false;
        }
        if !self.validate_surface_offset() {
            return true;
        }
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            let id = id.ok_or(viboceros_command::CommandError::Usage(
                "Select a curve or surface edge",
            ))?;
            let s = self.surface_offset_session.as_mut().unwrap();
            if s.subcurve.is_some() {
                s.subcurve_source = Some(CapturedCurve::capture(&self.document, id, edge)?);
            } else {
                s.source = Some(Source::capture(&self.document, id, edge, None)?);
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
        } else {
            self.component_selection.clear_choices();
            self.prepare_surface_offset();
            self.surface_offset_prompt();
        }
        true
    }
    pub(super) fn pick_surface_offset_face(
        &mut self,
        id: Option<ObjectId>,
        face: Option<usize>,
        point: Option<Point3>,
    ) -> bool {
        if !self.picking_surface_offset_face() {
            return false;
        }
        if !self.validate_surface_offset() {
            return true;
        }
        let result = id
            .ok_or(viboceros_command::CommandError::Usage(
                "Select a surface or face",
            ))
            .and_then(|id| Target::capture(&self.document, id, face));
        match result {
            Ok(target) => {
                let s = self.surface_offset_session.as_mut().unwrap();
                s.target = Some(target);
                s.side = point;
                self.component_selection.clear_choices();
                self.prepare_surface_offset();
                self.surface_offset_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    fn commit_surface_offset(&mut self) -> bool {
        if !self.validate_surface_offset() {
            return false;
        }
        let Some(s) = self.surface_offset_session.as_mut() else {
            return false;
        };
        let Some(plan) = &s.plan else {
            self.surface_offset_prompt();
            return false;
        };
        match plan.commit(&mut self.document, &mut s.group) {
            Ok(message) => {
                self.surface_offset_defaults = s.options;
                let through = s.through;
                s.plan = None;
                self.push_log(message);
                if through {
                    self.surface_offset_prompt();
                } else {
                    self.cancel_interactive_command(false);
                }
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn finish_surface_offset(&mut self) -> bool {
        self.surface_offset_session
            .take()
            .is_some_and(|s| s.group.can_undo())
    }
    pub(super) fn discard_surface_offset_subcurve(&mut self) -> bool {
        let Some(s) = self
            .surface_offset_session
            .as_mut()
            .filter(|s| s.subcurve.is_some())
        else {
            return false;
        };
        s.subcurve = None;
        s.subcurve_source = None;
        s.draft.clear();
        self.point_filter = None;
        self.point_constraint = None;
        self.surface_offset_prompt();
        true
    }
    pub(super) fn continue_surface_offset(&mut self, input: &str) -> bool {
        if self.surface_offset_session.is_none() {
            return false;
        }
        if !self.validate_surface_offset() {
            return true;
        }
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            if !self.discard_surface_offset_subcurve() {
                self.cancel_interactive_command(true);
            }
            return true;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if self.discard_surface_offset_subcurve() {
                return true;
            }
            let s = self.surface_offset_session.as_mut().unwrap();
            if let Some(ids) = &s.selected_points {
                if ids.is_empty() {
                    self.cancel_interactive_command(false);
                    return true;
                }
                let result = PointSetPlan::prepare(
                    &self.document,
                    s.source.as_ref().unwrap().clone(),
                    s.target.as_ref().unwrap().clone(),
                    ids,
                    s.options,
                )
                .and_then(|plan| plan.commit(&mut self.document, &mut s.group));
                match result {
                    Ok(message) => {
                        self.surface_offset_defaults = s.options;
                        self.push_log(message);
                        self.cancel_interactive_command(false);
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            } else if s.number_prompt {
                s.number_prompt = false;
                self.surface_offset_prompt();
            } else if s.target.is_some() {
                if s.through {
                    self.cancel_interactive_command(false);
                } else {
                    self.commit_surface_offset();
                }
            } else {
                self.cancel_interactive_command(false);
            }
            return true;
        }
        let s = self.surface_offset_session.as_mut().unwrap();
        if s.selected_points.is_some() {
            let ids = input
                .split_whitespace()
                .map(str::parse::<ObjectId>)
                .collect::<Result<Vec<_>, _>>();
            if let Ok(ids) = ids {
                self.pick_surface_offset_points(ids, SelectionMode::Add);
            } else {
                self.surface_offset_prompt();
            }
            return true;
        }
        if word.eq_ignore_ascii_case("Select")
            && s.through
            && s.target.is_some()
            && !s.group.can_undo()
        {
            s.selected_points = Some(Vec::new());
            s.through_cursor = None;
            s.plan = None;
            self.document.clear_selection();
            self.point_filter = None;
            self.point_constraint = None;
            self.surface_offset_prompt();
            return true;
        }
        if word.eq_ignore_ascii_case("ThroughPoint") && s.subcurve.is_none() {
            s.through = true;
            s.through_cursor = None;
            s.plan = None;
            s.number_prompt = false;
            self.surface_offset_prompt();
            return true;
        }
        if word.eq_ignore_ascii_case("SubCrv") && s.target.is_none() {
            s.source = None;
            s.subcurve = Some(Default::default());
            s.subcurve_source = None;
            s.draft.clear();
            self.surface_offset_prompt();
            return true;
        }
        if let Some(p) = s.subcurve.as_mut().filter(|p| p.start.is_some()) {
            if let Some(direction) = viboceros_command::subcurve_input::direction_option(input) {
                let mut message = None;
                match direction {
                    Ok(false) => p.locked_forward = None,
                    Ok(true) => {
                        if let Some(hover) = p.hover_parameter {
                            p.locked_forward = viboceros_command::subcurve_input::cursor_forward(
                                s.subcurve_source.as_ref().unwrap().curve_ref(),
                                p.start.unwrap(),
                                hover,
                            )
                            .ok();
                        } else {
                            message =
                                Some("Move along the curve before locking its direction".into());
                        }
                    }
                    Err(e) => message = Some(e.into()),
                }
                p.hover_parameter = None;
                s.draft.clear();
                if let Some(message) = message {
                    self.push_log(message);
                }
                self.surface_offset_prompt();
                return true;
            }
            if let Some(length) = viboceros_drafting::PointInput::parse_length_with_units(
                input,
                self.document.units(),
            ) {
                match length {
                    Ok(v) if v != 0. => {
                        let c = s.subcurve_source.as_ref().unwrap().curve_ref();
                        match c.length(self.document.tolerance()) {
                            Ok(total) if v.abs() <= total => {}
                            Ok(_) => {
                                self.push_log("Curve length exceeds the whole source curve".into());
                                return true;
                            }
                            Err(e) => {
                                self.push_log(format!("Error: {e}"));
                                return true;
                            }
                        }
                        p.length = Some(v.abs());
                        p.hover_parameter = None;
                        s.draft.clear();
                        if let Some(forward) = p.locked_forward {
                            match viboceros_command::subcurve_input::locked_numeric_requires_confirmation(
                                c, p.start.unwrap(), v.abs(), forward, self.document.tolerance(),
                            ) {
                                Ok(false) => {
                                    if let Ok(point) = c.evaluate(p.start.unwrap()) {
                                        self.accept_surface_offset_point(point);
                                    }
                                    return true;
                                }
                                Ok(true) => p.locked_forward = None,
                                Err(e) => {
                                    self.push_log(format!("Error: {e}"));
                                    return true;
                                }
                            }
                        }
                        self.surface_offset_prompt();
                    }
                    Ok(_) => {
                        self.discard_surface_offset_subcurve();
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
                return true;
            }
        }
        if s.number_prompt {
            match input.parse::<usize>() {
                Ok(n) if (1..=solver::MAX_NUMBER).contains(&n) => {
                    let changed = s.options.number != n || s.plan.is_none();
                    s.options.number = n;
                    s.number_prompt = false;
                    if changed {
                        self.prepare_surface_offset();
                    }
                    self.surface_offset_prompt();
                }
                _ => self.push_log(format!("Enter a number from 1 to {}", solver::MAX_NUMBER)),
            }
            return true;
        }
        if word.eq_ignore_ascii_case("Number") && s.target.is_some() && !s.through {
            s.number_prompt = true;
            self.surface_offset_prompt();
            return true;
        }
        if s.source.is_none()
            && let Ok(id) = input.parse::<ObjectId>()
        {
            self.pick_surface_offset_curve(Some(id), None);
            return true;
        }
        if s.source.is_some()
            && s.target.is_none()
            && let Ok(id) = input.parse::<ObjectId>()
        {
            self.pick_surface_offset_face(Some(id), None, None);
            return true;
        }
        if s.target.is_some()
            && !s.through
            && let Some(length) = viboceros_drafting::PointInput::parse_length_with_units(
                input,
                self.document.units(),
            )
        {
            match length {
                Ok(v) if v != 0. => {
                    let changed = s.options.distance != v || s.plan.is_none();
                    s.options.distance = v;
                    if changed {
                        self.prepare_surface_offset();
                    }
                    self.commit_surface_offset();
                }
                Ok(_) => self.push_log("Enter a nonzero offset distance".into()),
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            return true;
        }
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !s.through
            && let Ok((options, refs, _)) = solver::options(&words, s.options)
            && refs.source.is_none()
            && refs.surface.is_none()
            && refs.range.is_none()
            && refs.face.is_none()
            && refs.side.is_none()
            && refs.edge.is_none()
            && !refs.through_mode
            && refs.points.is_none()
        {
            let changed = s.options != options || s.plan.is_none();
            s.options = options;
            if changed {
                self.prepare_surface_offset();
            }
            self.surface_offset_prompt();
            return true;
        }
        if self.try_continue_point_input(input) {
            return true;
        }
        if !self
            .commands
            .recognizes(words.first().copied().unwrap_or(""))
        {
            self.surface_offset_prompt();
            return true;
        }
        false
    }
    pub(super) fn accept_surface_offset_point(&mut self, point: Point3) -> Option<bool> {
        self.surface_offset_session.as_ref()?;
        if !self.validate_surface_offset() {
            return Some(false);
        }
        if self.picking_surface_offset_points() {
            return Some(false);
        }
        if self
            .surface_offset_session
            .as_ref()
            .is_some_and(|s| s.through && s.target.is_some())
        {
            let s = self.surface_offset_session.as_mut().unwrap();
            if s.through_cursor != Some(point) || s.plan.is_none() {
                s.through_cursor = Some(point);
                self.prepare_surface_offset();
            }
            return Some(self.commit_surface_offset());
        }
        let s = self.surface_offset_session.as_mut()?;
        if let Some(c) = &s.subcurve_source {
            let parameter = match c
                .curve_ref()
                .closest_parameter(point, self.document.tolerance())
            {
                Ok(t) => t,
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return Some(false);
                }
            };
            let p = s.subcurve.as_mut().unwrap();
            let Some(start) = p.start else {
                p.start = Some(parameter);
                self.surface_offset_prompt();
                return Some(true);
            };
            let interval = if let Some(length) = p.length {
                if let Some(forward) = p.locked_forward {
                    viboceros_command::subcurve_input::interval_in_direction(
                        c.curve_ref(),
                        start,
                        length,
                        forward,
                        self.document.tolerance(),
                    )
                } else {
                    viboceros_command::subcurve_input::interval(
                        c.curve_ref(),
                        start,
                        parameter,
                        length,
                        self.document.tolerance(),
                    )
                }
            } else {
                viboceros_command::subcurve_input::point_parameters(
                    c.curve_ref(),
                    start,
                    parameter,
                    p.locked_forward,
                    false,
                )
                .map_err(Into::into)
            };
            match interval {
                Ok(Some(range)) => {
                    match Source::capture(&self.document, c.object_id(), c.source().1, Some(range))
                    {
                        Ok(source) => {
                            s.source = Some(source);
                            s.subcurve = None;
                            s.subcurve_source = None;
                            s.draft.clear();
                            self.point_filter = None;
                            self.point_constraint = None;
                            self.surface_offset_prompt();
                            Some(true)
                        }
                        Err(e) => {
                            self.push_log(format!("Error: {e}"));
                            Some(false)
                        }
                    }
                }
                Ok(None) => {
                    self.discard_surface_offset_subcurve();
                    Some(true)
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    Some(false)
                }
            }
        } else if s.target.is_some() {
            s.options.flip = !s.options.flip;
            self.prepare_surface_offset();
            self.surface_offset_prompt();
            Some(true)
        } else {
            self.surface_offset_prompt();
            Some(false)
        }
    }
    pub(super) fn accept_surface_offset_parameter(&mut self, t: f64) -> bool {
        let Some(c) = self
            .surface_offset_session
            .as_ref()
            .and_then(|s| s.subcurve_source.as_ref())
        else {
            return false;
        };
        if let Ok(point) = c.curve().evaluate(t) {
            self.accept_surface_offset_point(point);
        }
        true
    }
    pub(super) fn update_surface_offset_hover(&mut self, point: Point3) {
        if let Some(s) = self
            .surface_offset_session
            .as_mut()
            .filter(|s| s.through && s.target.is_some() && s.selected_points.is_none())
        {
            if s.through_cursor != Some(point) {
                s.through_cursor = Some(point);
                self.prepare_surface_offset_with_log(false);
            }
            return;
        }
        let Some(s) = self.surface_offset_session.as_mut() else {
            return;
        };
        let Some(p) = s.subcurve.as_mut().filter(|p| p.start.is_some()) else {
            return;
        };
        let Some(c) = s.subcurve_source.as_ref() else {
            return;
        };
        let (source, edge) = c.source();
        let Some(t) = p
            .preview
            .parameter(Some(source), edge, point, self.document.tolerance())
        else {
            return;
        };
        if p.hover_parameter == Some(t) {
            return;
        }
        p.hover_parameter = Some(t);
        s.draft.clear();
        if let Some(preview) = p.preview.temporary(
            Some(source),
            edge,
            [p.start.unwrap(), t],
            p.length,
            p.locked_forward,
            self.document.tolerance(),
        ) {
            s.draft.push(preview.geometry.curve.clone());
        }
    }
}
