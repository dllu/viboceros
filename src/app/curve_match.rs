//! Ordered Match contacts and a read-only preview during option editing.
use super::*;
use crate::viewport::EdgePick;
use viboceros_command::{CommandError, curve_reference::CapturedCurve, match_curve};
use viboceros_drafting::PointInput;
use viboceros_geometry::NurbsCurve;

mod options;
use options::Menu;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MatchTo {
    CurveEnd,
    PickPoint,
    SurfaceEdge,
}

pub(super) struct Session {
    picks: Vec<match_curve::Endpoint>,
    match_to: MatchTo,
    guide: Option<CapturedCurve>,
    object_id_pending: bool,
    menu: Option<Menu>,
    options: match_curve::Options,
    prepared: Option<match_curve::Prepared>,
    preview: Vec<NurbsCurve>,
}

impl Session {
    fn ready(&self) -> bool {
        self.picks.len() == 2
    }

    pub(super) fn hint(&self) -> &'static str {
        if self.ready() {
            self.menu
                .map_or("Press Enter to match or change options", Menu::hint)
        } else if self.object_id_pending {
            "Enter the curve object ID"
        } else if self.picks.is_empty() {
            "Select open curve near the end to change"
        } else if self.guide.is_some() {
            if self.match_to == MatchTo::SurfaceEdge {
                "Point on surface edge"
            } else {
                "Point on target curve"
            }
        } else if self.menu == Some(Menu::MatchTo) {
            "Match to CurveEnd or PickPoint"
        } else {
            match self.match_to {
                MatchTo::CurveEnd => {
                    "Select open curve near the end to match (SurfaceEdge, MatchTo)"
                }
                MatchTo::PickPoint => "Select point on curve to match (SurfaceEdge, MatchTo)",
                MatchTo::SurfaceEdge => "Select surface edge to match",
            }
        }
    }

    pub(super) fn preview(&self) -> Option<&[NurbsCurve]> {
        self.prepared.as_ref().map(|_| self.preview.as_slice())
    }

    pub(super) fn guide(&self) -> Option<&CapturedCurve> {
        self.guide.as_ref()
    }
}

impl VibocerosApp {
    pub(super) fn start_curve_match(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let Some(command) = words.next() else {
            return false;
        };
        if !command
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Match")
        {
            return false;
        }
        let arguments = words.collect::<Vec<_>>();
        if !arguments.is_empty()
            && self.document.selected_object_count() == 2
            && !command.contains('-')
        {
            return false;
        }
        let options = match match_curve::parse(&arguments, self.curve_match_options) {
            Ok(parsed) if parsed.picks.iter().any(Option::is_some) => return false,
            Ok(parsed) => parsed.options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.curve_match_session = Some(Session {
            picks: Vec::with_capacity(2),
            match_to: MatchTo::CurveEnd,
            guide: None,
            object_id_pending: false,
            menu: None,
            options,
            prepared: None,
            preview: Vec::new(),
        });
        self.curve_match_options = options;
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_curve_match();
        true
    }

    fn log_curve_match(&mut self) {
        if let Some(session) = &self.curve_match_session {
            let hint = session.hint();
            if session.ready() && session.menu.is_none() {
                self.push_log(format!("{hint} ({})", session.option_summary()));
            } else {
                self.push_log(hint.into());
            }
        }
    }

    pub(super) fn validate_curve_match(&mut self) {
        if self.curve_match_session.as_ref().is_some_and(|s| {
            s.picks.iter().any(|pick| !pick.current(&self.document))
                || s.guide
                    .as_ref()
                    .is_some_and(|guide| !guide.current(&self.document))
                || s.prepared
                    .as_ref()
                    .is_some_and(|prepared| !prepared.is_current(&self.document))
        }) {
            self.cancel_interactive_command(false);
            self.push_log("Match sources or document settings changed".into());
        }
    }

    pub(super) fn curve_match_first_id(&self) -> Option<ObjectId> {
        self.curve_match_session
            .as_ref()?
            .picks
            .first()
            .map(match_curve::Endpoint::id)
    }

    pub(super) fn picking_curve_match(&self) -> bool {
        self.plane_prompt.is_none()
            && self.curve_match_session.as_ref().is_some_and(|s| {
                !s.ready()
                    && s.guide.is_none()
                    && !s.object_id_pending
                    && s.menu.is_none()
                    && s.match_to != MatchTo::SurfaceEdge
            })
    }

    pub(super) fn picking_curve_match_edge(&self) -> bool {
        self.plane_prompt.is_none()
            && self.curve_match_session.as_ref().is_some_and(|s| {
                !s.ready() && s.guide.is_none() && s.match_to == MatchTo::SurfaceEdge
            })
    }

    pub(super) fn curve_match_guide(&self) -> Option<&CapturedCurve> {
        self.plane_prompt.is_none().then_some(())?;
        self.curve_match_session.as_ref()?.guide()
    }

    pub(super) fn pick_curve_match_viewport(
        &mut self,
        object: Option<ObjectId>,
        pick: Option<(usize, egui::Pos2)>,
    ) -> bool {
        if self.curve_match_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        if self.picking_curve_match()
            && let Some(id) = object
        {
            let point = pick.and_then(|(viewport, cursor)| {
                self.viewports[viewport].curve_pick_point(
                    self.document.object(id)?.geometry(),
                    None,
                    cursor,
                    self.document.tolerance(),
                )
            });
            if pick.is_none() || point.is_some() {
                self.pick_curve_match(id, point);
            }
        }
        true
    }

    fn pick_curve_match(&mut self, id: ObjectId, point: Option<Point3>) {
        if self.curve_match_first_id() == Some(id) {
            self.push_log("Select a different target".into());
            return;
        }
        let Some(session) = &self.curve_match_session else {
            return;
        };
        if session.match_to == MatchTo::PickPoint {
            if let Some(point) = point {
                self.take_match_pick(match_curve::Endpoint::at_point(
                    &self.document,
                    id,
                    None,
                    point,
                ));
            } else {
                self.capture_match_guide(id, None);
            }
        } else {
            self.take_match_pick(match_curve::Endpoint::capture(&self.document, id, point));
        }
    }

    pub(super) fn pick_curve_match_edge(&mut self, picks: &[EdgePick]) -> bool {
        if !self.picking_curve_match_edge() {
            return false;
        }
        if let Some(edge) = picks.first() {
            self.capture_match_guide(edge.object, Some(edge.edge));
        }
        true
    }

    fn capture_match_guide(&mut self, id: ObjectId, edge: Option<usize>) {
        match CapturedCurve::capture(&self.document, id, edge) {
            Ok(guide) => {
                if let Some(session) = &mut self.curve_match_session {
                    session.guide = Some(guide);
                }
                self.log_curve_match();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn accept_curve_match_parameter(&mut self, parameter: f64) -> bool {
        let Some(guide) = self.curve_match_guide() else {
            return false;
        };
        match guide.curve().evaluate(parameter) {
            Ok(point) => self.accept_curve_match_point(point),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    fn accept_curve_match_point(&mut self, point: Point3) {
        if let Some(guide) = self.curve_match_guide() {
            self.take_match_pick(match_curve::Endpoint::at_point(
                &self.document,
                guide.object_id(),
                guide.source().1,
                point,
            ));
        }
    }

    fn take_match_pick(&mut self, pick: Result<match_curve::Endpoint, CommandError>) {
        self.validate_curve_match();
        let Some(session) = &mut self.curve_match_session else {
            return;
        };
        match pick {
            Ok(pick) => {
                session.guide = None;
                let id = pick.id();
                if session.picks.is_empty() {
                    self.document
                        .set_transient_selection([id])
                        .expect("captured Match source");
                    self.document.mark_object_selection_temporary([id]);
                }
                session.picks.push(pick);
                if session.ready() {
                    self.refresh_curve_match();
                } else {
                    self.log_curve_match();
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn refresh_curve_match(&mut self) {
        let Some(session) = &mut self.curve_match_session else {
            return;
        };
        let [first, second] = session.picks.as_slice() else {
            return;
        };
        session.preview.clear();
        session.prepared = None;
        match match_curve::prepare(&self.document, [first, second], session.options) {
            Ok(prepared) => {
                session.preview.extend(
                    prepared.geometries().iter().filter_map(|geometry| {
                        geometry.nurbs_curve_representation().ok().flatten()
                    }),
                );
                session.prepared = Some(prepared);
            }
            Err(error) => self.push_log(format!("Preview error: {error}")),
        }
        self.log_curve_match();
    }

    pub(super) fn continue_curve_match(&mut self, input: &str) -> bool {
        if self.curve_match_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        self.command_input.clear();
        self.validate_curve_match();
        let Some(session) = &mut self.curve_match_session else {
            return true;
        };
        let input = input.trim().trim_start_matches(['_', '-']);
        if input == "!" || input.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
        } else if session.ready() {
            self.continue_curve_match_options(input);
        } else if session.guide.is_some() {
            if let Some(parsed) = PointInput::parse_with_units(input, self.document.units()) {
                match parsed.and_then(|point| {
                    point.resolve(
                        self.viewports[self.active_viewport].construction_plane(),
                        self.last_point,
                    )
                }) {
                    Ok(point) => self.accept_curve_match_point(point),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            } else {
                self.log_curve_match();
            }
        } else if input.is_empty() {
            if session.menu.take().is_none() && !std::mem::take(&mut session.object_id_pending) {
                self.cancel_interactive_command(true);
            } else {
                self.log_curve_match();
            }
        } else {
            self.continue_curve_match_selection(input);
        }
        true
    }

    fn continue_curve_match_selection(&mut self, input: &str) {
        let session = self.curve_match_session.as_mut().expect("Match selection");
        let (name, value) = input.split_once(['=', ' ']).unwrap_or((input, ""));
        let value = value.trim().trim_start_matches('_');
        if (name.eq_ignore_ascii_case("SelID") || session.object_id_pending)
            && session.match_to != MatchTo::SurfaceEdge
        {
            let id = if session.object_id_pending {
                input
            } else {
                value
            };
            if id.is_empty() {
                session.object_id_pending = true;
            } else if let Ok(id) = id.parse() {
                session.object_id_pending = false;
                self.pick_curve_match(id, None);
                return;
            } else {
                self.push_log("Enter an existing curve object ID".into());
                return;
            }
        } else if session.picks.len() == 1 {
            if name.eq_ignore_ascii_case("SurfaceEdge") {
                session.match_to = MatchTo::SurfaceEdge;
                session.menu = None;
            } else if name.eq_ignore_ascii_case("Edge") && session.match_to == MatchTo::SurfaceEdge
            {
                if let Some((id, index)) = viboceros_command::curve_reference::parse_edge(value) {
                    self.capture_match_guide(id, Some(index));
                } else {
                    self.push_log("Enter Edge=object-id,index".into());
                }
                return;
            } else if name.eq_ignore_ascii_case("MatchTo") || session.menu == Some(Menu::MatchTo) {
                let choice = if name.eq_ignore_ascii_case("MatchTo") {
                    value
                } else {
                    input
                };
                if choice.is_empty() {
                    session.menu = Some(Menu::MatchTo);
                } else if choice.eq_ignore_ascii_case("CurveEnd")
                    || choice.eq_ignore_ascii_case("PickPoint")
                {
                    session.match_to = if choice.eq_ignore_ascii_case("CurveEnd") {
                        MatchTo::CurveEnd
                    } else {
                        MatchTo::PickPoint
                    };
                    session.menu = None;
                } else {
                    self.push_log("MatchTo expects CurveEnd or PickPoint".into());
                    return;
                }
            }
        }
        self.log_curve_match();
    }
}
