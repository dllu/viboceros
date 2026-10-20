//! ArrayCrv getters keep instances read-only until one atomic command accepts.
use super::*;
use viboceros_command::curve_array::{
    CurveArrayOrientation as Orientation, Placement, Preferences, Reference,
};
use viboceros_geometry::{AffineTransform3, Curve3, NurbsCurve};
mod subcurve;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Guide,
    Base,
    SubcurveStart,
    SubcurveEnd,
    Ready,
}
#[derive(Clone, Copy, Debug)]
enum Edit {
    Items,
    Distance,
    Orientation,
}
struct Guide {
    captured: viboceros_command::curve_reference::CapturedCurve,
    original: Curve3,
    cursor: NurbsCurve,
    edge: Option<usize>,
    range: Option<[f64; 2]>,
    pick: Option<Point3>,
}
impl Guide {
    fn current(&self, doc: &Document) -> bool {
        self.captured.current(doc)
    }
}
pub(super) struct Session {
    sources: Vec<ObjectId>,
    source_geometry: Vec<viboceros_document::GeometrySnapshot>,
    postselected: bool,
    phase: Phase,
    edit: Option<Edit>,
    base: Option<Point3>,
    plane: Frame3,
    guide: Option<Guide>,
    subcurve: Option<super::uv_subcurve_input::PendingSubcurve>,
    placement: Option<Placement>,
    prefs: Preferences,
    transforms: Vec<AffineTransform3>,
    pub(super) pointer: Option<egui::Pos2>,
}
impl Session {
    pub(super) fn wants_point(&self) -> bool {
        matches!(
            self.phase,
            Phase::Base | Phase::SubcurveStart | Phase::SubcurveEnd
        )
    }
    pub(super) fn cursor_curve(&self) -> Option<&NurbsCurve> {
        matches!(self.phase, Phase::SubcurveStart | Phase::SubcurveEnd)
            .then(|| &self.guide.as_ref().unwrap().cursor)
    }
    pub(super) fn preview(&self) -> Option<crate::viewport::InstancePreview<'_>> {
        (self.phase == Phase::Ready).then_some(crate::viewport::InstancePreview {
            sources: &self.sources,
            transforms: &self.transforms,
        })
    }
}
impl VibocerosApp {
    pub(super) fn picking_curve_array(&self) -> bool {
        self.curve_array_session
            .as_ref()
            .is_some_and(|s| s.phase == Phase::Guide)
    }
    pub(super) fn start_curve_array(&mut self, input: &str, picked: Option<&[ObjectId]>) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty()
            || !["arraycrv", "arraycurve"].contains(
                &words[0]
                    .trim_start_matches(['_', '-'])
                    .to_ascii_lowercase()
                    .as_str(),
            )
        {
            return false;
        }
        let mut prefs = self.commands.curve_array_preferences();
        if !prefs.by_distance {
            prefs.items = prefs.items.max(2);
        }
        for word in &words[1..] {
            let Some((key, value)) = word.split_once('=') else {
                return false;
            };
            if !key
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Orientation")
            {
                return false;
            }
            let Some(o) = Orientation::parse(value) else {
                return false;
            };
            prefs.orientation = o;
        }
        self.cancel_interactive_command(false);
        let sources = picked.map_or_else(
            || self.document.selected_object_ids().collect::<Vec<_>>(),
            |p| p.to_vec(),
        );
        if sources.is_empty() {
            self.start_transform_source_prompt("ArrayCrv");
            self.object_prompt.as_mut().unwrap().command_override = Some(input.into());
            return true;
        }
        let source_geometry = sources
            .iter()
            .map(|id| {
                self.document
                    .object(*id)
                    .unwrap()
                    .geometry_snapshot()
                    .clone()
            })
            .collect();
        self.curve_array_session = Some(Session {
            sources,
            source_geometry,
            postselected: picked.is_some(),
            phase: Phase::Guide,
            edit: None,
            base: None,
            plane: self.viewports[self.active_viewport].construction_plane(),
            guide: None,
            subcurve: None,
            placement: None,
            prefs,
            transforms: Vec::new(),
            pointer: None,
        });
        self.active_command = Some(InteractiveCommand::CurveArray { base: None });
        self.curve_array_prompt();
        true
    }
    fn curve_array_prompt(&mut self) {
        let Some(s) = &self.curve_array_session else {
            return;
        };
        let prompt = if let Some(edit) = s.edit {
            match edit {
                Edit::Items => format!("Number of items <{}>", s.prefs.items),
                Edit::Distance => format!("Distance between items <{}>", s.prefs.distance),
                Edit::Orientation => "Orientation=Freeform|Roadlike|Stairlike|NoRotation".into(),
            }
        } else {
            match s.phase {
                Phase::Guide if s.subcurve.is_some() => {
                    "Select curve to temporarily shorten; Enter returns".into()
                }
                Phase::Guide => {
                    "Select path curve near its starting end (Basepoint, SubCrv)".into()
                }
                Phase::Base => "Base point for objects to array".into(),
                Phase::SubcurveStart => "Start of curve".into(),
                Phase::SubcurveEnd => {
                    "End point or curve length (Direction=Free|Locked); Enter returns".into()
                }
                Phase::Ready => format!(
                    "Array by {}; Enter accepts (Items={}, Distance={}, Orientation={})",
                    if s.prefs.by_distance {
                        "distance"
                    } else {
                        "number of items"
                    },
                    s.prefs.items,
                    s.prefs.distance,
                    s.prefs.orientation.name()
                ),
            }
        };
        self.push_log(format!("ArrayCrv: {prompt}; Esc cancels"));
    }
    pub(super) fn validate_curve_array(&mut self) -> bool {
        let Some(s) = self.curve_array_session.as_mut() else {
            return true;
        };
        let sources_current = s
            .sources
            .iter()
            .zip(&s.source_geometry)
            .all(|(id, snapshot)| {
                self.document.is_object_selectable(*id)
                    && self
                        .document
                        .object(*id)
                        .is_some_and(|o| snapshot.shares_storage_with(o.geometry_snapshot()))
            });
        if !sources_current {
            self.push_log("Array sources changed or became unavailable; restart ArrayCrv".into());
            self.cancel_interactive_command(false);
            return false;
        }
        if s.guide.as_ref().is_none_or(|g| g.current(&self.document)) {
            return true;
        }
        s.guide = None;
        s.placement = None;
        s.subcurve = None;
        s.transforms.clear();
        s.phase = Phase::Guide;
        s.edit = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.component_selection.clear_choices();
        self.push_log("Array guide changed or became unavailable; select it again".into());
        self.curve_array_prompt();
        false
    }
    pub(super) fn pick_curve_array(&mut self, id: Option<ObjectId>, edge: Option<usize>) -> bool {
        if !self.picking_curve_array() {
            return false;
        }
        let pick = id.and_then(|id| {
            self.viewports[self.active_viewport].curve_pick_point(
                self.document.object(id)?.geometry(),
                edge,
                self.curve_array_session.as_ref()?.pointer?,
                self.document.tolerance(),
            )
        });
        self.pick_curve_array_at(id, edge, pick)
    }
    pub(super) fn pick_curve_array_at(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
        pick: Option<Point3>,
    ) -> bool {
        if !self.picking_curve_array() {
            return false;
        }
        let result = (|| -> Result<_, String> {
            let id = id.ok_or("Select a curve or surface edge")?;
            if self
                .curve_array_session
                .as_ref()
                .unwrap()
                .sources
                .contains(&id)
            {
                return Err("Select a guide outside the source objects".into());
            }
            let captured = viboceros_command::curve_reference::CapturedCurve::capture(
                &self.document,
                id,
                edge,
            )
            .map_err(|e| e.to_string())?;
            let original = captured.curve_ref().to_owned();
            let cursor = original.as_ref().to_nurbs().map_err(|e| e.to_string())?;
            Ok(Guide {
                captured,
                original,
                cursor,
                edge,
                range: None,
                pick,
            })
        })();
        match result {
            Ok(g) => {
                let s = self.curve_array_session.as_mut().unwrap();
                s.guide = Some(g);
                s.plane = self.viewports[self.active_viewport].construction_plane();
                self.component_selection.clear();
                if let Some(p) = &mut s.subcurve {
                    p.object = id;
                    s.phase = Phase::SubcurveStart;
                } else {
                    self.prepare_curve_array();
                }
                self.curve_array_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        true
    }
    fn prepare_curve_array(&mut self) -> bool {
        let s = self.curve_array_session.as_mut().unwrap();
        let g = s.guide.as_ref().unwrap();
        let result = Placement::try_new(
            &self.document,
            &s.sources,
            Reference {
                curve: g.captured.object_id(),
                edge: g.edge,
                pick: g.pick,
            },
            g.range,
            s.base,
            s.plane,
            false,
        )
        .and_then(|p| {
            let prefs = p.spacing_preferences(s.prefs)?;
            let transforms = p.transforms(prefs.spacing(), prefs.orientation, s.sources.len())?;
            Ok((p, prefs, transforms))
        });
        match result {
            Ok((p, prefs, maps)) => {
                s.placement = Some(p);
                s.prefs = prefs;
                s.transforms = maps;
                s.phase = Phase::Ready;
                self.commands.remember_curve_array_preferences(prefs);
                true
            }
            Err(e) => {
                s.guide = None;
                s.placement = None;
                s.transforms.clear();
                s.subcurve = None;
                s.phase = Phase::Guide;
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn accept_curve_array_point(&mut self, p: Point3) -> Option<bool> {
        self.curve_array_session.as_ref()?;
        if !self.validate_curve_array() {
            return Some(false);
        }
        if let Some(result) = self.accept_curve_array_subcurve_point(p) {
            return Some(result);
        }
        let s = self.curve_array_session.as_mut().unwrap();
        if s.phase != Phase::Base {
            return Some(false);
        }
        s.base = Some(p);
        s.phase = Phase::Guide;
        self.active_command = Some(InteractiveCommand::CurveArray { base: Some(p) });
        self.curve_array_prompt();
        Some(true)
    }
    pub(super) fn accept_curve_array_parameter(&mut self, t: f64) -> bool {
        let p = self
            .curve_array_session
            .as_ref()
            .filter(|s| matches!(s.phase, Phase::SubcurveStart | Phase::SubcurveEnd))
            .and_then(|s| s.guide.as_ref()?.original.as_ref().evaluate(t).ok());
        p.is_some_and(|p| self.accept_curve_array_point(p).unwrap_or(false))
    }
    fn commit_curve_array(&mut self) -> bool {
        if !self.validate_curve_array() {
            return false;
        }
        let s = self.curve_array_session.as_ref().unwrap();
        let g = s.guide.as_ref().unwrap();
        let range = g
            .range
            .map_or_else(String::new, |[a, b]| format!(" SubCrv={a},{b}"));
        let base = s.base.map_or_else(String::new, |p| {
            format!(" BasePoint={}", format_model_point(p))
        });
        let spacing = if s.prefs.by_distance {
            format!("Distance={}", s.prefs.distance)
        } else {
            format!("Items={}", s.prefs.items)
        };
        let input = format!(
            "ArrayCrv {spacing} {}{range}{base} Orientation={} {}={}",
            Reference {
                curve: g.captured.object_id(),
                edge: g.edge,
                pick: g.pick
            }
            .arguments(),
            s.prefs.orientation.name(),
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
        match self.commands.execute_in_context(
            &mut self.document,
            &input,
            viboceros_command::CommandContext {
                construction_plane: s.plane,
            },
        ) {
            Ok(message) => {
                self.push_log(message);
                self.curve_array_session = None;
                self.cancel_interactive_command(false);
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    fn curve_array_option(&mut self, edit: Edit, value: &str) {
        let s = self.curve_array_session.as_mut().unwrap();
        let mut prefs = s.prefs;
        let valid = match edit {
            Edit::Items => value
                .parse::<usize>()
                .ok()
                .filter(|n| *n >= 2 && *n <= 1_000_001)
                .map(|n| {
                    prefs.items = n;
                    prefs.by_distance = false;
                })
                .is_some(),
            Edit::Distance => viboceros_drafting::PointInput::parse_length_with_units(
                value,
                self.document.units(),
            )
            .and_then(Result::ok)
            .filter(|d| {
                d.is_finite()
                    && *d > 0.
                    && s.placement
                        .as_ref()
                        .unwrap()
                        .curve()
                        .length(self.document.tolerance())
                        .is_ok_and(|length| *d < length)
            })
            .map(|d| {
                prefs.distance = d;
                prefs.by_distance = true;
            })
            .is_some(),
            Edit::Orientation => Orientation::parse(value)
                .map(|o| prefs.orientation = o)
                .is_some(),
        };
        if !valid {
            self.push_log("Error: enter an item count of at least two, a positive distance, or a valid orientation".into());
            return;
        }
        let p = s.placement.as_ref().unwrap();
        match p
            .transforms(prefs.spacing(), prefs.orientation, s.sources.len())
            .and_then(|maps| Ok((maps, p.spacing_preferences(prefs)?)))
        {
            Ok((maps, prefs)) => {
                s.prefs = prefs;
                s.transforms = maps;
                s.edit = None;
                self.commands.remember_curve_array_preferences(prefs);
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    pub(super) fn continue_curve_array(&mut self, input: &str) -> bool {
        let consumed = self.continue_curve_array_input(input);
        if consumed {
            self.command_input.clear();
        }
        consumed
    }
    fn continue_curve_array_input(&mut self, input: &str) -> bool {
        if self.curve_array_session.is_none() {
            return false;
        }
        if input
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Cancel")
        {
            self.cancel_interactive_command(true);
            return true;
        }
        if !self.validate_curve_array() {
            return true;
        }
        if self.continue_curve_array_subcurve(input) {
            return true;
        }
        let s = self.curve_array_session.as_ref().unwrap();
        let word = input.trim_start_matches(['_', '-']);
        let enter = input.is_empty() || word.eq_ignore_ascii_case("Enter");
        if s.phase == Phase::Guide && word.eq_ignore_ascii_case("Basepoint") {
            self.curve_array_session.as_mut().unwrap().phase = Phase::Base;
            self.curve_array_prompt();
            return true;
        }
        if s.phase != Phase::Ready {
            return false;
        }
        if let Some(edit) = s.edit {
            let default = match edit {
                Edit::Items => s.prefs.items.to_string(),
                Edit::Distance => s.prefs.distance.to_string(),
                Edit::Orientation => s.prefs.orientation.name().into(),
            };
            self.curve_array_option(edit, if enter { &default } else { input });
            self.curve_array_prompt();
            return true;
        }
        if enter {
            self.commit_curve_array();
            return true;
        }
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        let edit = if name.eq_ignore_ascii_case("Items") {
            Some(Edit::Items)
        } else if name.eq_ignore_ascii_case("Distance") {
            Some(Edit::Distance)
        } else if name.eq_ignore_ascii_case("Orientation") {
            Some(Edit::Orientation)
        } else {
            None
        };
        if let Some(edit) = edit {
            if let Some(value) = value {
                self.curve_array_option(edit, value);
            } else {
                self.curve_array_session.as_mut().unwrap().edit = Some(edit);
            }
        } else if input.parse::<usize>().is_ok() {
            self.curve_array_option(Edit::Items, input);
            if input.parse::<usize>().ok().is_some_and(|n| {
                n >= 2 && self.curve_array_session.as_ref().unwrap().prefs.items == n
            }) {
                self.commit_curve_array();
            }
        } else {
            self.push_log("Error: enter a count or choose Items, Distance or Orientation".into());
        }
        self.curve_array_prompt();
        true
    }
    pub(super) fn cancel_curve_array(&mut self) {
        if let Some(s) = self.curve_array_session.take()
            && (s.postselected || s.guide.is_some_and(|g| g.range.is_some()))
        {
            self.document.clear_selection();
        }
    }
}
