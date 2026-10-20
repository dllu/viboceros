//! Curve endpoint picking and repeated placements use the scripted command solver.
use super::*;
use viboceros_command::curve_reference::CapturedCurve;
use viboceros_command::orient_curve_edge::{Edge, Options, Placement, Source};
use viboceros_geometry::{AffineTransform3, NurbsCurve};
mod subcurve;

pub(super) struct Session {
    group: viboceros_document::HistoryGroup,
    sources: Vec<ObjectId>,
    source: Option<Source>,
    edge: Option<(ObjectId, usize)>,
    placement: Option<Placement>,
    cursor: Option<NurbsCurve>,
    pick: Option<Point3>,
    face: Option<usize>,
    options: Options,
    plane: Frame3,
    preview: Option<AffineTransform3>,
    applied: bool,
    postselected: bool,
    subcurve: Option<super::uv_subcurve_input::PendingSubcurve>,
    subcurve_source: Option<CapturedCurve>,
    range: Option<[f64; 2]>,
    pub(super) pointer: Option<egui::Pos2>,
}
impl Session {
    pub(super) fn has_subcurve(&self) -> bool {
        self.subcurve.is_some()
    }
    pub(super) fn wants_point(&self) -> bool {
        self.placement.is_some() || self.subcurve_source.is_some()
    }
    pub(super) fn cursor_curve(&self) -> Option<&NurbsCurve> {
        self.cursor.as_ref()
    }
    pub(super) fn update_preview(&mut self, value: Option<AffineTransform3>) -> bool {
        if self.preview == value {
            false
        } else {
            self.preview = value;
            true
        }
    }
    pub(super) fn preview(&self) -> Option<crate::viewport::AffinePreview<'_>> {
        Some(crate::viewport::AffinePreview {
            sources: &self.sources,
            grips: &[],
            definition: crate::viewport::affine_preview::Mapping::CurveEdge(
                self.placement.as_ref()?,
            ),
            frame: Some(self.plane),
            copy: self.options.copy,
            last_transform: self.preview,
            rigid_layout: None,
        })
    }
}
impl VibocerosApp {
    pub(super) fn picking_orient_edge(&self) -> bool {
        self.orient_edge_session
            .as_ref()
            .is_some_and(|s| !s.wants_point())
    }
    pub(super) fn start_orient_edge(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|s| {
            !s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("OrientCrvToEdge")
        }) {
            return false;
        }
        let Ok((rest, mut options, reference)) =
            viboceros_command::orient_curve_edge::options(&words[1..], Options::default())
        else {
            return false;
        };
        if !rest.is_empty() || reference.edge.is_some() || reference.pick.is_some() {
            return false;
        }
        self.cancel_interactive_command(false);
        let remembered = self.commands.begin_copy_options("OrientCrvToEdge").unwrap();
        if !words[1..].iter().any(|s| {
            s.split('=')
                .next()
                .unwrap()
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Copy")
        }) {
            options.copy = remembered;
        }
        match self.document.begin_history_group("OrientCrvToEdge") {
            Ok(group) => {
                self.orient_edge_session = Some(Session {
                    group,
                    sources: Vec::new(),
                    source: None,
                    edge: None,
                    placement: None,
                    cursor: None,
                    pick: None,
                    face: reference.face,
                    options,
                    plane: self.viewports[self.active_viewport].construction_plane(),
                    preview: None,
                    applied: false,
                    postselected: true,
                    subcurve: None,
                    subcurve_source: None,
                    range: None,
                    pointer: None,
                });
                self.active_command = Some(InteractiveCommand::OrientCurveToEdge);
                self.orient_edge_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    fn orient_edge_prompt(&mut self) {
        let Some(s) = &self.orient_edge_session else {
            return;
        };
        let prompt = if let Some(p) = &s.subcurve {
            match (p.object, p.start, p.length) {
                (None, _, _) => "Select curve to temporarily shorten; Enter returns",
                (Some(_), None, _) => "Start of curve: pick on the source",
                (Some(_), Some(_), None) => {
                    "Pick directed end or type a curve length; Direction=Locked/Free"
                }
                (Some(_), Some(_), Some(_)) => {
                    "Confirm length direction on the curve; Enter returns"
                }
            }
        } else if s.source.is_none() {
            "select a curve near an end (SubCrv)"
        } else if s.edge.is_none() {
            "select a target surface edge"
        } else {
            "pick a target edge point; Enter finishes"
        };
        self.push_log(format!(
            "OrientCrvToEdge: {prompt} ({})",
            s.options.arguments()
        ));
    }
    pub(super) fn validate_orient_edge(&mut self) -> bool {
        let Some(s) = &mut self.orient_edge_session else {
            return true;
        };
        if s.subcurve_source
            .as_ref()
            .is_some_and(|c| !c.current(&self.document))
            || s.source
                .as_ref()
                .is_some_and(|c| !c.curve().current(&self.document))
        {
            self.push_log(
                "Source curve changed or became unavailable; restart OrientCrvToEdge".into(),
            );
            self.cancel_interactive_command(false);
            return false;
        }
        if s.placement
            .as_ref()
            .is_some_and(|p| !p.edge.current(&self.document))
        {
            s.edge = None;
            s.placement = None;
            s.cursor = None;
            s.preview = None;
            self.component_selection.clear_choices();
            self.point_filter = None;
            self.point_constraint = None;
            self.push_log("Target edge changed or became unavailable; select it again".into());
            self.orient_edge_prompt();
            return false;
        }
        true
    }
    pub(super) fn pick_curve_to_edge(&mut self, id: Option<ObjectId>, edge: Option<usize>) -> bool {
        if !self.picking_orient_edge() {
            return false;
        }
        let pick = id.and_then(|id| {
            self.viewports[self.active_viewport].curve_pick_point(
                self.document.object(id)?.geometry(),
                edge,
                self.orient_edge_session.as_ref()?.pointer?,
                self.document.tolerance(),
            )
        });
        self.pick_curve_to_edge_at(id, edge, pick)
    }
    pub(super) fn pick_curve_to_edge_at(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
        pick: Option<Point3>,
    ) -> bool {
        if !self.picking_orient_edge() {
            return false;
        }
        if !self.validate_orient_edge() {
            return true;
        }
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            let id = id.ok_or(viboceros_command::CommandError::Usage(
                "Select a curve or surface edge",
            ))?;
            let s = self.orient_edge_session.as_mut().unwrap();
            if let Some(p) = &mut s.subcurve {
                if edge.is_some() {
                    return Err(viboceros_command::CommandError::Usage(
                        "Select a curve object",
                    ));
                }
                let captured = CapturedCurve::capture(&self.document, id, None)?;
                s.cursor = Some(captured.curve().clone());
                s.postselected = !self.document.is_selected(id);
                s.sources = vec![id];
                s.pick = pick;
                p.object = Some(id);
                s.subcurve_source = Some(captured);
            } else if let Some(source) = &s.source {
                let index = edge.ok_or(viboceros_command::CommandError::Usage(
                    "Select a surface edge",
                ))?;
                let captured = Edge::capture(&self.document, id, index, s.face)?;
                let placement = Placement::new(
                    source.clone(),
                    captured,
                    s.options,
                    s.plane,
                    self.document.tolerance(),
                );
                s.cursor = Some(placement.curve().to_nurbs()?);
                s.placement = Some(placement);
                s.edge = Some((id, index));
            } else {
                if edge.is_some() {
                    return Err(viboceros_command::CommandError::Usage(
                        "Select an open curve object near an end",
                    ));
                }
                let source = Source::capture(&self.document, id, pick)?;
                s.postselected = !self.document.is_selected(id);
                s.sources = vec![id];
                s.source = Some(source);
                s.pick = pick;
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
        } else {
            self.component_selection.clear_choices();
            self.orient_edge_prompt();
        }
        true
    }
    pub(super) fn resolve_orient_edge_point(
        &self,
        point: Point3,
    ) -> Result<Point3, viboceros_command::CommandError> {
        self.orient_edge_session
            .as_ref()
            .and_then(|s| s.placement.as_ref())
            .map_or(Ok(point), |p| Ok(p.locate(point)?.point()))
    }
    pub(super) fn accept_orient_edge_point(&mut self, point: Point3) -> Option<bool> {
        self.orient_edge_session.as_ref()?;
        if !self.validate_orient_edge() {
            return Some(false);
        }
        if let Some(result) = self.accept_edge_subcurve_point(point) {
            return Some(result);
        }
        let s = self.orient_edge_session.as_mut().unwrap();
        let Some(placement) = &s.placement else {
            self.push_log("Select the curve and target surface edge first".into());
            return Some(false);
        };
        if !self.document.history_group_is_current(&s.group) {
            match self.document.begin_history_group("OrientCrvToEdge") {
                Ok(g) => s.group = g,
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return Some(false);
                }
            }
        }
        let (id, index) = s.edge.unwrap();
        let pick = s.pick.map_or_else(String::new, |p| {
            format!(" PickPoint={}", format_model_point(p))
        });
        let range = s
            .range
            .map_or_else(String::new, |[start, end]| format!(" SubCrv={start},{end}"));
        let input = format!(
            "OrientCrvToEdge {} Edge={id},{index} Face={} {} {}={}{}{}",
            format_model_point(point),
            placement.edge.target.face,
            s.options.arguments(),
            if s.postselected {
                "PickedSources"
            } else {
                "Sources"
            },
            s.sources[0],
            pick,
            range
        );
        match self.commands.execute_in_history_group(
            &mut self.document,
            &input,
            viboceros_command::CommandContext {
                construction_plane: s.plane,
            },
            &mut s.group,
        ) {
            Ok(message) => {
                s.applied = true;
                s.preview = None;
                let copy = s.options.copy;
                self.push_log(message);
                if !copy {
                    self.cancel_interactive_command(false);
                } else {
                    self.orient_edge_prompt();
                }
                Some(true)
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                Some(false)
            }
        }
    }
    pub(super) fn continue_orient_edge(&mut self, input: &str) -> bool {
        if self.continue_edge_subcurve(input) {
            return true;
        }
        let Some(s) = self.orient_edge_session.as_mut() else {
            return false;
        };
        let word = input.trim_start_matches(['_', '-']);
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            self.cancel_interactive_command(true);
            return true;
        }
        if word.eq_ignore_ascii_case("Cancel") {
            return false;
        }
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        if key.eq_ignore_ascii_case("Face") {
            if s.placement.is_some() {
                self.push_log("Choose the surface face before selecting the target edge".into());
            } else if let Some(index) = value.and_then(|v| v.parse().ok()) {
                s.face = Some(index);
                self.orient_edge_prompt();
            } else {
                self.push_log("Error: expected a face index".into());
            }
            return true;
        }
        let slot = match key.to_ascii_lowercase().as_str() {
            "copy" => &mut s.options.copy,
            "flipsurface" => &mut s.options.flip_surface,
            "reversecurve" => &mut s.options.reverse_curve,
            _ => return false,
        };
        match value
            .map(|v| v.trim_start_matches('_').to_ascii_lowercase())
            .as_deref()
        {
            None => *slot = !*slot,
            Some("yes") => *slot = true,
            Some("no") => *slot = false,
            _ => {
                self.push_log("Error: expected Yes or No".into());
                return true;
            }
        }
        if let Some(p) = &mut s.placement {
            p.options = s.options;
        }
        s.preview = None;
        self.orient_edge_prompt();
        true
    }
    pub(super) fn finish_orient_edge(&mut self) -> bool {
        let Some(s) = self.orient_edge_session.take() else {
            return false;
        };
        if s.applied && s.postselected {
            self.document.clear_selection();
        }
        s.applied
    }
}
