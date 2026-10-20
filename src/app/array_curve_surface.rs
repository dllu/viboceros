//! Repeated surface-normal placements share the command's captured guide solver.
use super::*;
use viboceros_command::array_curve_surface::{Layout, Placement, References};
use viboceros_geometry::{AffineTransform3, Curve3, NurbsCurve};
mod subcurve;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Base,
    Guide,
    SubcurveStart,
    SubcurveEnd,
    Surface,
    Ready,
    Count(bool),
    Spacing(usize),
}
struct Guide {
    id: ObjectId,
    edge: Option<usize>,
    parent: viboceros_document::GeometrySnapshot,
    tolerance: Tolerance,
    original: Curve3,
    working: Curve3,
    cursor: NurbsCurve,
    range: Option<[f64; 2]>,
    pick: Option<Point3>,
}
impl Guide {
    fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc.is_object_selectable(self.id)
            && doc
                .object(self.id)
                .is_some_and(|o| self.parent.shares_storage_with(o.geometry_snapshot()))
    }
}
pub(super) struct Session {
    group: viboceros_document::HistoryGroup,
    sources: Vec<ObjectId>,
    postselected: bool,
    phase: Phase,
    base: Option<Point3>,
    plane: Frame3,
    guide: Option<Guide>,
    subcurve: Option<super::uv_subcurve_input::PendingSubcurve>,
    placement: Option<Placement>,
    surface: Option<(ObjectId, Option<usize>)>,
    preview: Option<AffineTransform3>,
    applied: bool,
    last_distance: f64,
    pub(super) pointer: Option<egui::Pos2>,
}
impl Session {
    pub(super) fn wants_point(&self) -> bool {
        matches!(
            self.phase,
            Phase::Base | Phase::SubcurveStart | Phase::SubcurveEnd | Phase::Ready
        )
    }
    pub(super) fn update_preview(&mut self, value: Option<AffineTransform3>) -> bool {
        if self.preview == value {
            false
        } else {
            self.preview = value;
            true
        }
    }
    pub(super) fn cursor_curve(&self) -> Option<&NurbsCurve> {
        if matches!(
            self.phase,
            Phase::SubcurveStart | Phase::SubcurveEnd | Phase::Ready
        ) {
            Some(&self.guide.as_ref()?.cursor)
        } else {
            None
        }
    }
    pub(super) fn preview(&self) -> Option<crate::viewport::AffinePreview<'_>> {
        if self.phase != Phase::Ready {
            return None;
        }
        Some(crate::viewport::AffinePreview {
            sources: &self.sources,
            grips: &[],
            definition: crate::viewport::affine_preview::Mapping::SurfaceCurveArray(
                self.placement.as_ref()?,
            ),
            frame: Some(self.plane),
            copy: true,
            last_transform: self.preview,
            rigid_layout: None,
        })
    }
}
impl VibocerosApp {
    pub(super) fn picking_array_curve(&self) -> bool {
        self.array_curve_surface_session
            .as_ref()
            .is_some_and(|s| s.phase == Phase::Guide)
    }
    pub(super) fn picking_array_surface(&self) -> bool {
        self.array_curve_surface_session
            .as_ref()
            .is_some_and(|s| s.phase == Phase::Surface)
    }
    pub(super) fn try_start_array_curve_surface(
        &mut self,
        input: &str,
        picked: Option<&[ObjectId]>,
    ) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.len() != 1
            || !["arraycrvonsrf", "arraycurveonsurface"].contains(
                &words[0]
                    .trim_start_matches(['_', '-'])
                    .to_ascii_lowercase()
                    .as_str(),
            )
        {
            return false;
        }
        self.cancel_interactive_command(false);
        let sources = picked.map_or_else(
            || self.document.selected_object_ids().collect::<Vec<_>>(),
            |p| p.to_vec(),
        );
        if sources.is_empty() {
            self.start_transform_source_prompt("ArrayCrvOnSrf");
            return true;
        }
        match self.document.begin_history_group("ArrayCrvOnSrf") {
            Ok(group) => {
                self.array_curve_surface_session = Some(Session {
                    group,
                    sources,
                    postselected: picked.is_some(),
                    phase: Phase::Base,
                    base: None,
                    plane: self.viewports[self.active_viewport].construction_plane(),
                    guide: None,
                    subcurve: None,
                    placement: None,
                    surface: None,
                    preview: None,
                    applied: false,
                    last_distance: 0.,
                    pointer: None,
                });
                self.active_command = Some(InteractiveCommand::ArrayCurveOnSurface);
                self.array_curve_surface_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    fn array_curve_surface_prompt(&mut self) {
        let Some(s) = &self.array_curve_surface_session else {
            return;
        };
        let prefs = self.commands.array_curve_surface_preferences();
        let text = match s.phase {
            Phase::Base => "base point".into(),
            Phase::Guide if s.subcurve.is_some() => {
                "select a curve to temporarily shorten; Enter returns".into()
            }
            Phase::Guide => "select a curve near its starting end (SubCrv)".into(),
            Phase::SubcurveStart => "start of temporary guide".into(),
            Phase::SubcurveEnd => {
                "directed end or curve length; Direction=Free|Locked; Enter returns".into()
            }
            Phase::Surface => "select the surface".into(),
            Phase::Ready => "position objects or distance from last (Divide, Multiple)".into(),
            Phase::Count(multiple) => format!(
                "number of {} <{}>",
                if multiple { "copies" } else { "objects" },
                prefs.count
            ),
            Phase::Spacing(_) => format!("distance between copies <{}>", prefs.distance),
        };
        self.push_log(format!("ArrayCrvOnSrf: {text}; Esc finishes"));
    }
    pub(super) fn validate_array_curve_surface(&mut self) -> bool {
        let Some(s) = self.array_curve_surface_session.as_mut() else {
            return true;
        };
        if s.guide.as_ref().is_none_or(|g| g.current(&self.document))
            && s.placement
                .as_ref()
                .is_none_or(|p| p.current(&self.document))
        {
            return true;
        }
        s.guide = None;
        s.placement = None;
        s.surface = None;
        s.subcurve = None;
        s.preview = None;
        s.phase = Phase::Guide;
        s.last_distance = 0.;
        if !self.document.history_group_is_current(&s.group) {
            match self.document.begin_history_group("ArrayCrvOnSrf") {
                Ok(group) => s.group = group,
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    self.cancel_interactive_command(false);
                    return false;
                }
            }
        }
        self.component_selection.clear_choices();
        self.point_filter = None;
        self.point_constraint = None;
        self.push_log("Array guide changed or became unavailable; select it again".into());
        self.array_curve_surface_prompt();
        false
    }
    pub(super) fn pick_array_curve(&mut self, id: Option<ObjectId>, edge: Option<usize>) -> bool {
        if !self.picking_array_curve() {
            return false;
        }
        let point = id.and_then(|id| {
            let o = self.document.object(id)?;
            let pointer = self.array_curve_surface_session.as_ref()?.pointer?;
            self.viewports[self.active_viewport].curve_pick_point(
                o.geometry(),
                edge,
                pointer,
                self.document.tolerance(),
            )
        });
        self.pick_array_curve_at(id, edge, point)
    }
    pub(super) fn pick_array_curve_at(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
        pick: Option<Point3>,
    ) -> bool {
        if !self.picking_array_curve() {
            return false;
        }
        let result = (|| -> Result<_, String> {
            let id = id.ok_or("Select a curve or surface edge")?;
            let o = self
                .document
                .object(id)
                .filter(|_| self.document.is_object_selectable(id))
                .ok_or("Curve is unavailable")?;
            if self
                .array_curve_surface_session
                .as_ref()
                .unwrap()
                .sources
                .contains(&id)
            {
                return Err("Select a guide outside the source objects".into());
            }
            let c = viboceros_command::curve_reference::CapturedCurve::capture(
                &self.document,
                id,
                edge,
            )
            .map_err(|e| e.to_string())?;
            let original = c.curve_ref().to_owned();
            let cursor = original.as_ref().to_nurbs().map_err(|e| e.to_string())?;
            Ok(Guide {
                id,
                edge,
                parent: o.geometry_snapshot().clone(),
                tolerance: self.document.tolerance(),
                original: original.clone(),
                working: original,
                cursor,
                range: None,
                pick,
            })
        })();
        match result {
            Ok(g) => {
                let s = self.array_curve_surface_session.as_mut().unwrap();
                s.guide = Some(g);
                self.component_selection.clear();
                if let Some(p) = &mut s.subcurve {
                    p.object = id;
                    s.phase = Phase::SubcurveStart;
                } else {
                    s.phase = Phase::Surface;
                }
                self.array_curve_surface_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn accept_array_surface(&mut self, id: ObjectId, face: Option<usize>) -> bool {
        if !self.picking_array_surface() {
            return false;
        }
        let s = self.array_curve_surface_session.as_ref().unwrap();
        let g = s.guide.as_ref().unwrap();
        let refs = References {
            curve: g.id,
            edge: g.edge,
            surface: id,
            face,
            pick: g.pick,
        };
        let result = if s.sources.contains(&id) {
            Err(viboceros_command::CommandError::Usage(
                "Select a surface outside the source objects",
            ))
        } else {
            Placement::try_new(
                &self.document,
                s.base.unwrap(),
                s.plane,
                refs,
                g.range,
                false,
            )
        };
        match result {
            Ok(p) => {
                let cursor = p.cursor_curve();
                match cursor {
                    Ok(c) => {
                        let s = self.array_curve_surface_session.as_mut().unwrap();
                        s.guide.as_mut().unwrap().cursor = c;
                        s.placement = Some(p);
                        s.surface = Some((id, face));
                        s.phase = Phase::Ready;
                        self.array_curve_surface_prompt();
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn resolve_array_curve_surface_point(
        &self,
        point: Point3,
    ) -> Result<Point3, viboceros_command::CommandError> {
        let Some(s) = &self.array_curve_surface_session else {
            return Ok(point);
        };
        let c = match s.phase {
            Phase::SubcurveStart | Phase::SubcurveEnd => {
                s.guide.as_ref().map(|g| g.original.as_ref())
            }
            Phase::Ready => s.placement.as_ref().map(Placement::curve),
            _ => None,
        };
        let Some(c) = c else {
            return Ok(point);
        };
        Ok(c.evaluate(c.closest_parameter(point, self.document.tolerance())?)?)
    }
    pub(super) fn accept_array_curve_surface_parameter(&mut self, t: f64) -> bool {
        let point = self
            .array_curve_surface_session
            .as_ref()
            .and_then(Session::cursor_curve)
            .and_then(|c| c.evaluate(t).ok());
        if let Some(p) = point {
            self.accept_filtered_drafting_point(p, false);
            true
        } else {
            false
        }
    }
    pub(super) fn accept_array_curve_surface_point(&mut self, point: Point3) -> Option<bool> {
        self.array_curve_surface_session.as_ref()?;
        if !self.validate_array_curve_surface() {
            return Some(false);
        }
        if let Some(result) = self.accept_array_subcurve_point(point) {
            return Some(result);
        }
        let s = self.array_curve_surface_session.as_mut().unwrap();
        let accepted = match s.phase {
            Phase::Base => {
                s.base = Some(point);
                s.plane = self.viewports[self.active_viewport].construction_plane();
                s.phase = Phase::Guide;
                true
            }
            Phase::Ready => self.commit_array_curve_surface(Layout::Positions(vec![point])),
            _ => false,
        };
        if accepted {
            self.array_curve_surface_prompt();
        }
        Some(accepted)
    }
    fn commit_array_curve_surface(&mut self, layout: Layout) -> bool {
        let s = self.array_curve_surface_session.as_mut().unwrap();
        let g = s.guide.as_ref().unwrap();
        let (surface, face) = s.surface.unwrap();
        let placement = s.placement.as_ref().unwrap();
        let maps = match placement.transforms(&layout, s.sources.len()) {
            Ok(m) => m,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let next_distance = match &layout {
            Layout::Positions(p) => placement.distance_at(*p.last().unwrap()).ok(),
            Layout::Distances { start, deltas } => Some(start + deltas.iter().sum::<f64>()),
            Layout::Multiple {
                distance, start, ..
            } => Some(start + maps.len() as f64 * distance),
            Layout::Divide(_) => None,
        };
        let creates_copies = !maps.is_empty();
        drop(maps);
        let args = match layout {
            Layout::Divide(n) => format!("Divide={n}"),
            Layout::Multiple {
                count,
                distance,
                start,
            } => format!("Multiple={count},{distance} Offset={start}"),
            Layout::Positions(points) => format!(
                "Positions={}",
                points
                    .into_iter()
                    .map(format_model_point)
                    .collect::<Vec<_>>()
                    .join(";")
            ),
            Layout::Distances { start, deltas } => format!(
                "Distances={} Offset={start}",
                deltas
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        };
        let range = g
            .range
            .map_or_else(String::new, |[a, b]| format!(" SubCrv={a},{b}"));
        let refs = References {
            curve: g.id,
            edge: g.edge,
            surface,
            face,
            pick: g.pick,
        };
        let input = format!(
            "ArrayCrvOnSrf {} {} {args}{range} Finish=No {}={}",
            format_model_point(s.base.unwrap()),
            refs.arguments(),
            if s.postselected {
                "PickedSources"
            } else {
                "Sources"
            },
            s.sources
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
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
                s.applied |= creates_copies;
                s.preview = None;
                if let Some(d) = next_distance {
                    s.last_distance = d;
                }
                s.phase = Phase::Ready;
                self.push_log(message);
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn try_continue_array_curve_surface(&mut self, input: &str) -> bool {
        let consumed = self.continue_array_curve_surface_input(input);
        if consumed {
            self.command_input.clear();
        }
        consumed
    }
    fn continue_array_curve_surface_input(&mut self, input: &str) -> bool {
        if self.array_curve_surface_session.is_none() {
            return false;
        }
        if !self.validate_array_curve_surface() {
            return true;
        }
        if self.continue_array_subcurve(input) {
            return true;
        }
        let word = input.trim_start_matches(['_', '-']);
        let phase = self.array_curve_surface_session.as_ref().unwrap().phase;
        let mut prefs = self.commands.array_curve_surface_preferences();
        let enter = input.is_empty() || word.eq_ignore_ascii_case("Enter");
        if phase == Phase::Ready
            && (word.eq_ignore_ascii_case("Divide") || word.eq_ignore_ascii_case("Multiple"))
        {
            self.array_curve_surface_session.as_mut().unwrap().phase =
                Phase::Count(word.eq_ignore_ascii_case("Multiple"));
            self.array_curve_surface_prompt();
            return true;
        }
        match phase {
            Phase::Count(multiple) => {
                let count = if enter {
                    Some(prefs.count)
                } else {
                    input.parse::<usize>().ok()
                };
                let Some(n) = count else {
                    return false;
                };
                if !(2..=1_000_000).contains(&n) {
                    self.push_log("Error: count must be between 2 and 1000000".into());
                    return true;
                }
                prefs.count = n;
                self.commands
                    .remember_array_curve_surface_preferences(prefs);
                if multiple {
                    self.array_curve_surface_session.as_mut().unwrap().phase = Phase::Spacing(n);
                } else {
                    self.commit_array_curve_surface(Layout::Divide(n));
                }
                self.array_curve_surface_prompt();
                true
            }
            Phase::Spacing(count) => {
                let value = if enter {
                    Some(Ok(prefs.distance))
                } else {
                    viboceros_drafting::PointInput::parse_length_with_units(
                        input,
                        self.document.units(),
                    )
                };
                let Some(value) = value else {
                    return false;
                };
                match value {
                    Ok(d) if d.is_finite() && d >= 0. => {
                        prefs.distance = d;
                        self.commands
                            .remember_array_curve_surface_preferences(prefs);
                        let start = self
                            .array_curve_surface_session
                            .as_ref()
                            .unwrap()
                            .last_distance;
                        self.commit_array_curve_surface(Layout::Multiple {
                            count,
                            distance: d,
                            start,
                        });
                        self.array_curve_surface_prompt();
                    }
                    _ => self.push_log("Error: distance must be finite and nonnegative".into()),
                }
                true
            }
            Phase::Ready if enter => {
                self.array_curve_surface_prompt();
                true
            }
            Phase::Ready => {
                let Some(value) = viboceros_drafting::PointInput::parse_length_with_units(
                    input,
                    self.document.units(),
                ) else {
                    return false;
                };
                match value {
                    Ok(delta) => {
                        let start = self
                            .array_curve_surface_session
                            .as_ref()
                            .unwrap()
                            .last_distance;
                        self.commit_array_curve_surface(Layout::Distances {
                            start,
                            deltas: vec![delta],
                        });
                        self.array_curve_surface_prompt();
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
                true
            }
            _ => false,
        }
    }
    pub(super) fn finish_array_curve_surface_session(&mut self) -> bool {
        let Some(s) = self.array_curve_surface_session.take() else {
            return false;
        };
        if s.postselected || s.guide.as_ref().is_some_and(|g| g.range.is_some()) {
            self.document.clear_selection();
        }
        s.applied
    }
}
