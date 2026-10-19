//! Repeated point getter: geometry remains immutable until Enter.
use super::*;
use viboceros_command::refinement::{self as edits, Options};
use viboceros_geometry::NurbsCurve;
mod subcurve;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Kink,
    Knot,
    Remove,
    Control,
    InsertControl,
    InsertEdit,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Kink => "InsertKink",
            Self::Knot => "InsertKnot",
            Self::Remove => "RemoveKnot",
            Self::Control => "RemoveControlPoint",
            Self::InsertControl => "InsertControlPoint",
            Self::InsertEdit => "InsertEditPoint",
        }
    }
    fn noun(self) -> &'static str {
        match self {
            Self::Kink => "a kink",
            Self::Knot | Self::Remove => "a knot",
            Self::Control | Self::InsertControl => "a control point",
            Self::InsertEdit => "an edit point",
        }
    }
    fn accept(
        self,
        options: &mut Options,
        word: &str,
    ) -> Result<bool, viboceros_command::CommandError> {
        if self == Self::InsertEdit {
            Ok(false)
        } else if self == Self::InsertControl {
            viboceros_command::insert_control_point::accept_option(options, word)
        } else if self == Self::Control {
            viboceros_command::remove_control_point::accept_option(options, word)
        } else if self == Self::Remove {
            viboceros_command::remove_knot::accept_option(options, word)
        } else {
            options.accept(word)
        }
    }
    fn options(self, options: Options) -> String {
        if self == Self::InsertControl {
            return format!(
                "{} Midpoint={}",
                options.arguments().split_whitespace().next().unwrap(),
                if options.midpoints { "Yes" } else { "No" }
            );
        }
        if self.removes() {
            options
                .arguments()
                .split_whitespace()
                .next()
                .unwrap()
                .into()
        } else {
            options.arguments()
        }
    }
    fn removes(self) -> bool {
        matches!(self, Self::Remove | Self::Control)
    }
}
#[derive(Debug)]
enum Draft {
    Kink(viboceros_command::insert_kink::Draft),
    Knot(viboceros_command::insert_knot::Draft),
    Remove(viboceros_command::remove_knot::Draft),
    Control(viboceros_command::remove_control_point::Draft),
    InsertControl(viboceros_command::insert_control_point::Draft),
    InsertEdit(viboceros_command::insert_edit_point::Draft),
}
impl std::ops::Deref for Draft {
    type Target = viboceros_command::refinement::Draft;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Kink(d) => d,
            Self::Knot(d) => d,
            Self::Remove(d) => d,
            Self::Control(d) => d,
            Self::InsertControl(d) => d,
            Self::InsertEdit(d) => d,
        }
    }
}
impl Draft {
    fn new(
        doc: &Document,
        id: ObjectId,
        range: Option<[f64; 2]>,
        kind: Kind,
    ) -> Result<Self, viboceros_command::CommandError> {
        Ok(match kind {
            Kind::InsertEdit => Self::InsertEdit(viboceros_command::insert_edit_point::Draft::new(
                doc, id, range,
            )?),
            Kind::InsertControl => {
                if range.is_some() {
                    return Err(viboceros_command::CommandError::Usage(
                        "InsertControlPoint does not support SubCrv",
                    ));
                }
                Self::InsertControl(viboceros_command::insert_control_point::Draft::new(
                    doc, id,
                )?)
            }
            Kind::Kink => Self::Kink(viboceros_command::insert_kink::Draft::new(doc, id, range)?),
            Kind::Knot => Self::Knot(viboceros_command::insert_knot::Draft::new(doc, id, range)?),
            Kind::Remove => {
                Self::Remove(viboceros_command::remove_knot::Draft::new(doc, id, range)?)
            }
            Kind::Control => {
                if range.is_some() {
                    return Err(viboceros_command::CommandError::Usage(
                        "RemoveControlPoint does not support SubCrv",
                    ));
                }
                Self::Control(viboceros_command::remove_control_point::Draft::new(
                    doc, id,
                )?)
            }
        })
    }
    fn commit(
        &self,
        doc: &mut Document,
        post: bool,
    ) -> Result<String, viboceros_command::CommandError> {
        match self {
            Self::Remove(d) => d.commit(doc, post),
            Self::Control(d) => d.commit(doc, post),
            Self::InsertControl(d) => d.commit(doc, post),
            Self::InsertEdit(d) => d.commit(doc, post),
            _ => std::ops::Deref::deref(self).commit(doc, post),
        }
    }
    fn point(
        &mut self,
        point: Point3,
        options: Options,
        projection: Option<viboceros_command::picking::Projection>,
    ) -> Result<(), viboceros_command::CommandError> {
        match self {
            Self::Kink(d) => d.point(point, options),
            Self::Knot(d) => d.point(point, options),
            Self::Remove(d) => d.point_in_view(point, options, projection),
            Self::Control(d) => d.point_in_view(point, options, projection),
            Self::InsertControl(d) => d.point(point, options),
            Self::InsertEdit(d) => d.point(point),
        }
    }
}
#[derive(Debug)]
pub(super) struct Prompt {
    kind: Kind,
    draft: Option<Draft>,
    options: Options,
    postselected: bool,
    subcurve: bool,
    input: super::uv_subcurve_input::PendingSubcurve,
    locked_side: Option<viboceros_document::GeometrySnapshot>,
    full_closed_length: bool,
    shortening: bool,
    cursor: Option<NurbsCurve>,
    hint: String,
    scene: Option<Document>,
    background: Option<super::surface_rebuild::Background>,
    guides: Vec<NurbsCurve>,
    guide_points: Vec<Point3>,
    extend_on_select: bool,
}
impl Prompt {
    pub(super) fn scene(&self) -> Option<&Document> {
        self.scene.as_ref()
    }
    fn refresh_scene(
        &mut self,
        document: &Document,
    ) -> Result<(), viboceros_command::CommandError> {
        if !matches!(
            self.kind,
            Kind::Remove | Kind::InsertControl | Kind::InsertEdit
        ) {
            return Ok(());
        }
        self.scene = None;
        self.background = Some(super::surface_rebuild::Background::capture(document));
        let Some(draft) = &self.draft else {
            return Ok(());
        };
        if self.shortening {
            return Ok(());
        }
        self.cursor = draft
            .geometry()
            .curve_ref()
            .map(|c| c.to_nurbs())
            .transpose()?;
        let mut scene = document.clone();
        scene.clear_history()?;
        if self.subcurve {
            scene.add_geometry(draft.geometry().clone())?;
        } else {
            scene.replace_object_geometries([(draft.object(), draft.geometry().clone())])?;
        }
        self.scene = Some(scene);
        Ok(())
    }
    pub(super) fn command_name(&self) -> &'static str {
        self.kind.name()
    }
    pub(super) fn selecting(&self) -> bool {
        self.draft.is_none()
    }
    pub(super) fn wants_point(&self) -> bool {
        self.draft.is_some()
    }
    pub(super) fn cursor_curve(&self) -> Option<&NurbsCurve> {
        self.cursor.as_ref().filter(|_| self.kind != Kind::Control)
    }
    pub(super) fn hint(&self) -> &str {
        &self.hint
    }
    pub(super) fn guides(&self) -> &[NurbsCurve] {
        &self.guides
    }
    pub(super) fn control_removal(
        &self,
    ) -> Option<(&viboceros_command::remove_control_point::Draft, Options)> {
        match self.draft.as_ref()? {
            Draft::Control(d) => Some((d, self.options)),
            _ => None,
        }
    }
    pub(super) fn markers(&self) -> Option<&[Point3]> {
        (!self.shortening && !self.guide_points.is_empty()).then_some(self.guide_points.as_slice())
    }
    fn update_guides(&mut self) {
        self.guides.clear();
        self.guide_points.clear();
        let Some(draft) = &self.draft else {
            return;
        };
        if let Draft::InsertEdit(d) = draft {
            self.guide_points = d.marker_points().unwrap_or_default();
            return;
        }
        if let Draft::Control(d) = draft {
            self.guide_points = d.marker_points().to_vec();
            return;
        }
        if let Draft::InsertControl(d) = draft {
            self.guide_points = d.marker_points().unwrap_or_default();
            return;
        }
        if let Draft::Remove(d) = draft
            && let Ok(Some(points)) = d.marker_points()
        {
            self.guide_points = points.into_iter().take(4096).collect();
            return;
        }
        if matches!(self.kind, Kind::Knot | Kind::Remove)
            && let Some(c) = draft.geometry().curve_ref().and_then(|c| c.to_nurbs().ok())
        {
            let domain = c.domain();
            let mut knots = c
                .knots()
                .iter()
                .copied()
                .filter(|x| domain.contains(x))
                .collect::<Vec<_>>();
            knots.dedup();
            let values = if self.options.midpoints {
                knots.windows(2).map(|x| x[0].midpoint(x[1])).collect()
            } else {
                knots
                    .into_iter()
                    .filter(|x| *x > *domain.start() && *x < *domain.end())
                    .collect::<Vec<_>>()
            };
            self.guide_points = values
                .into_iter()
                .take(4096)
                .filter_map(|t| c.evaluate(t).ok())
                .collect();
            return;
        }
        let Some((surface, _, _)) =
            viboceros_command::surface_reference::surface(draft.geometry(), None)
        else {
            return;
        };
        for (knots, domain, constant_u) in [
            (surface.knots_u(), surface.domain_u(), true),
            (surface.knots_v(), surface.domain_v(), false),
        ] {
            if constant_u && self.options.direction == viboceros_geometry::SurfaceKnotDirection::U
                || !constant_u
                    && self.options.direction == viboceros_geometry::SurfaceKnotDirection::V
            {
                continue;
            }
            let mut unique = knots
                .iter()
                .copied()
                .filter(|t| domain.contains(t))
                .collect::<Vec<_>>();
            unique.dedup();
            let values = if self.options.midpoints {
                unique
                    .windows(2)
                    .map(|p| p[0] / 2. + p[1] / 2.)
                    .collect::<Vec<_>>()
            } else {
                unique
                    .into_iter()
                    .filter(|t| *t > *domain.start() && *t < *domain.end())
                    .collect()
            };
            for t in values.into_iter().take(1024) {
                if let Ok(c) = if constant_u {
                    surface.isocurve_v(t)
                } else {
                    surface.isocurve_u(t)
                } {
                    self.guides.push(c);
                }
            }
        }
    }
    pub(super) fn surface(&self) -> Option<(ObjectId, Option<usize>, bool)> {
        if self.kind == Kind::Control {
            return None;
        }
        let d = self.draft.as_ref()?;
        d.geometry()
            .curve_ref()
            .is_none()
            .then_some((d.object(), None, true))
    }
    pub(super) fn object(&self) -> Option<ObjectId> {
        self.draft.as_ref().map(|d| d.object())
    }
    fn update_hint(&mut self) {
        self.hint = if self.selecting() {
            if self.kind == Kind::InsertEdit {
                "Select one curve (SubCrv temporarily shortens a curve); Esc cancels".into()
            } else if self.extend_on_select {
                "Select one surface to extend its control net; Esc cancels".into()
            } else if matches!(self.kind, Kind::Control | Kind::InsertControl) {
                "Select one curve or surface; Esc cancels".into()
            } else if self.subcurve {
                "Select curve to temporarily shorten; Enter or Esc returns".into()
            } else {
                "Select one curve or surface (SubCrv temporarily shortens a curve); Esc cancels"
                    .into()
            }
        } else if self.shortening {
            if self.input.start.is_none() {
                "Pick temporary subcurve start; Enter or Esc returns".into()
            } else {
                format!(
                    "{}; Direction={}; Esc returns",
                    if self.input.length.is_some() {
                        "Confirm length direction or enter another length"
                    } else {
                        "Pick temporary subcurve end or enter a length"
                    },
                    if self.input.locked_forward.is_some() {
                        "Locked"
                    } else {
                        "Free"
                    }
                )
            }
        } else if self.kind == Kind::InsertControl {
            let extending =
                matches!(self.draft.as_ref(), Some(Draft::InsertControl(d)) if d.extending());
            format!(
                "{} ({}); {} accepted; Enter finishes; Esc discards all picks",
                if extending {
                    "Pick control-net end to extend"
                } else {
                    "Pick a location for a control point or row; Extend selects a surface end"
                },
                self.kind.options(self.options),
                self.draft.as_ref().unwrap().picks()
            )
        } else if self.kind == Kind::Control {
            format!(
                "Pick one control point to remove ({} Toggle for surfaces); {} accepted; Enter finishes; Esc cancels",
                self.kind.options(self.options),
                self.draft.as_ref().unwrap().picks()
            )
        } else if self.cursor.is_some() {
            format!(
                "Point on curve to {} {}; {} accepted; {}Enter finishes; Esc discards all picks",
                if self.kind == Kind::Remove {
                    "remove"
                } else {
                    "add"
                },
                self.kind.noun(),
                self.draft.as_ref().unwrap().picks(),
                if self.kind == Kind::Knot {
                    "type a parameter, Automatic or Undo; "
                } else if self.kind == Kind::Remove {
                    "Undo; "
                } else {
                    ""
                }
            )
        } else {
            format!(
                "Point on surface to {} {} ({} Toggle); {}Enter finishes; Esc discards all picks",
                if self.kind == Kind::Remove {
                    "remove"
                } else {
                    "add"
                },
                self.kind.noun(),
                self.kind.options(self.options),
                if self.kind == Kind::Knot {
                    "Automatic or Undo; "
                } else if self.kind == Kind::Remove {
                    "Undo; "
                } else {
                    ""
                }
            )
        };
    }
}
impl VibocerosApp {
    fn continue_knot_action(&mut self, text: &str) -> bool {
        let Some(p) = self
            .refinement_prompt
            .as_mut()
            .filter(|p| matches!(p.kind, Kind::Knot | Kind::Remove) && !p.shortening)
        else {
            return false;
        };
        let Some(d) = p.draft.as_mut() else {
            return false;
        };
        let key = text.trim_start_matches(['_', '-']);
        let result = if key.eq_ignore_ascii_case("Undo") {
            let changed = match d {
                Draft::Knot(d) => d.undo(),
                Draft::Remove(d) => d.undo(),
                _ => unreachable!(),
            };
            if !changed {
                self.push_log("No staged edits to undo".into());
            }
            self.log_refinement();
            self.command_input.clear();
            return true;
        } else if key.eq_ignore_ascii_case("Automatic") {
            match d {
                Draft::Knot(d) => d.automatic(p.options),
                _ => return false,
            }
        } else if !text.contains(',')
            && let Ok(parameter) = text.parse::<f64>()
        {
            match d {
                Draft::Knot(d) => d.parameter(parameter, p.options),
                Draft::Remove(d) => d.parameter(parameter),
                Draft::Control(_) => return false,
                _ => unreachable!(),
            }
        } else {
            return false;
        };
        match result {
            Ok(()) => self.log_refinement(),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn cancel_refinement_input(&mut self) {
        if self
            .refinement_prompt
            .as_ref()
            .is_some_and(|p| p.shortening)
        {
            self.discard_refinement_subcurve();
        } else {
            self.cancel_refinement();
        }
    }
    pub(super) fn cancel_refinement(&mut self) {
        if self
            .refinement_prompt
            .as_ref()
            .is_some_and(|p| p.kind.removes())
        {
            self.document.clear_selection();
        }
        self.refinement_prompt = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.snaps.model_override = None;
        self.command_input.clear();
    }
    pub(super) fn validate_refinement(&mut self) {
        if self
            .refinement_prompt
            .as_ref()
            .and_then(|p| p.draft.as_ref())
            .is_some_and(|d| !d.current(&self.document))
        {
            self.cancel_refinement();
            self.push_log("Refinement source changed; start again".into());
        } else if let Some(p) = &mut self.refinement_prompt
            && matches!(
                p.kind,
                Kind::Remove | Kind::InsertControl | Kind::InsertEdit
            )
            && p.background
                .as_ref()
                .is_none_or(|b| !b.is_current(&self.document))
            && let Err(e) = p.refresh_scene(&self.document)
        {
            self.push_log(format!("Preview error: {e}"));
        }
    }
    pub(super) fn start_refinement(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let Some(command) = words.next().map(|w| w.trim_start_matches(['_', '-'])) else {
            return false;
        };
        let kind = if command.eq_ignore_ascii_case("InsertKink") {
            Kind::Kink
        } else if command.eq_ignore_ascii_case("InsertKnot") {
            Kind::Knot
        } else if command.eq_ignore_ascii_case("RemoveKnot") {
            Kind::Remove
        } else if command.eq_ignore_ascii_case("RemoveControlPoint") {
            Kind::Control
        } else if command.eq_ignore_ascii_case("InsertEditPoint") {
            Kind::InsertEdit
        } else if command.eq_ignore_ascii_case("InsertControlPoint") {
            Kind::InsertControl
        } else {
            return false;
        };
        let mut options = match kind {
            Kind::Kink => self.commands.insert_kink_options(),
            Kind::Knot => self.commands.insert_knot_options(),
            Kind::Remove => self.commands.remove_knot_options(),
            Kind::Control => self.commands.remove_control_point_options(),
            Kind::InsertControl => self.commands.insert_control_point_options(),
            Kind::InsertEdit => Options::default(),
        };
        let mut subcurve = false;
        let mut extend_on_select = false;
        for w in words {
            if kind == Kind::InsertControl
                && w.trim_start_matches('_').eq_ignore_ascii_case("Extend")
            {
                extend_on_select = true;
            } else if w.trim_start_matches('_').eq_ignore_ascii_case("SubCrv") {
                if matches!(kind, Kind::Control | Kind::InsertControl) {
                    return false;
                }
                subcurve = true;
            } else {
                match kind.accept(&mut options, w) {
                    Ok(true) => {}
                    _ => return false,
                }
            }
        }
        let ids = self
            .document
            .selected_objects()
            .filter(|o| {
                self.document.is_object_selectable(o.id())
                    && edits::accepts(o.geometry())
                    && (kind != Kind::InsertEdit || o.geometry().curve_ref().is_some())
            })
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        let mut prompt = Prompt {
            kind,
            draft: None,
            options,
            postselected: ids.len() != 1 || subcurve,
            subcurve,
            input: Default::default(),
            locked_side: None,
            full_closed_length: false,
            shortening: subcurve,
            cursor: None,
            hint: String::new(),
            scene: None,
            background: None,
            guides: Vec::new(),
            guide_points: Vec::new(),
            extend_on_select,
        };
        prompt.update_hint();
        self.refinement_prompt = Some(prompt);
        self.remember_refinement_options(kind, options);
        self.push_log(format!("> {input}"));
        if ids.len() == 1 && !subcurve {
            self.pick_refinement_object(Some(ids[0]));
        } else {
            self.log_refinement();
        }
        true
    }
    fn remember_refinement_options(&self, kind: Kind, options: Options) {
        match kind {
            Kind::Kink => {
                self.commands.remember_insert_kink_options(options);
            }
            Kind::Knot => {
                self.commands.remember_insert_knot_options(options);
            }
            Kind::Remove => {
                self.commands.remember_remove_knot_options(options);
            }
            Kind::Control => {
                self.commands.remember_remove_control_point_options(options);
            }
            Kind::InsertControl => {
                self.commands.remember_insert_control_point_options(options);
            }
            Kind::InsertEdit => {}
        }
    }
    fn log_refinement(&mut self) {
        if let Some(p) = &mut self.refinement_prompt {
            if let Err(e) = p.refresh_scene(&self.document) {
                self.push_log(format!("Preview error: {e}"));
                return;
            }
            p.update_hint();
            p.update_guides();
            let h = p.hint.clone();
            self.push_log(h);
        }
    }
    pub(super) fn pick_refinement_object(&mut self, object: Option<ObjectId>) -> bool {
        let Some(p) = &self.refinement_prompt else {
            return false;
        };
        if !p.selecting() {
            return true;
        }
        let Some(id) = object else {
            return true;
        };
        if p.subcurve
            && self
                .document
                .object(id)
                .is_none_or(|o| o.geometry().curve_ref().is_none())
        {
            return true;
        }
        let prepared = if p.kind == Kind::InsertEdit && p.shortening {
            viboceros_command::insert_edit_point::Draft::for_subcurve_selection(&self.document, id)
                .map(Draft::InsertEdit)
        } else {
            Draft::new(&self.document, id, None, p.kind)
        };
        match prepared {
            Ok(mut draft) => {
                if p.extend_on_select
                    && let Draft::InsertControl(d) = &mut draft
                    && let Err(e) = d.extend()
                {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
                let cursor = draft
                    .geometry()
                    .curve_ref()
                    .map(|c| c.to_nurbs())
                    .transpose();
                match cursor {
                    Ok(cursor) => {
                        let p = self.refinement_prompt.as_mut().unwrap();
                        p.cursor = cursor;
                        p.input.object = Some(draft.object());
                        p.draft = Some(draft);
                        p.extend_on_select = false;
                        self.log_refinement();
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    fn refinement_point(&mut self, point: Point3) -> bool {
        let projection = self.viewports[self.active_viewport].pick_projection();
        self.validate_refinement();
        let Some(p) = &mut self.refinement_prompt else {
            return false;
        };
        let Some(d) = &mut p.draft else {
            return false;
        };
        if p.shortening {
            return self.refinement_subcurve_point(point);
        }
        match d.point(point, p.options, projection) {
            Ok(()) => {
                if matches!(d, Draft::InsertControl(d) if d.cancelled()) {
                    self.cancel_refinement();
                } else if p.kind == Kind::Control {
                    self.continue_refinement("");
                } else {
                    self.log_refinement();
                }
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn accept_refinement_drafting_point(&mut self, point: Point3) -> Option<bool> {
        if !self
            .refinement_prompt
            .as_ref()
            .is_some_and(Prompt::wants_point)
        {
            return None;
        }
        Some(self.refinement_point(point))
    }
    pub(super) fn continue_refinement(&mut self, input: &str) -> bool {
        if self.refinement_prompt.is_none() {
            return false;
        }
        let text = input.trim();
        let key = text.trim_start_matches(['_', '-']);
        if key.eq_ignore_ascii_case("Cancel") || text == "!" {
            self.cancel_refinement();
            return true;
        }
        self.validate_refinement();
        if self.continue_refinement_subcurve(text) {
            return true;
        }
        if key.eq_ignore_ascii_case("SubCrv")
            && self.refinement_prompt.as_ref().is_some_and(|p| {
                !matches!(p.kind, Kind::Control | Kind::InsertControl)
                    && p.selecting()
                    && !p.shortening
            })
        {
            let p = self.refinement_prompt.as_mut().unwrap();
            p.subcurve = true;
            p.shortening = true;
            p.input = Default::default();
            self.log_refinement();
            return true;
        }
        if key.eq_ignore_ascii_case("Extend")
            && let Some(p) = self.refinement_prompt.as_mut()
            && let Some(Draft::InsertControl(d)) = p.draft.as_mut()
        {
            match d.extend() {
                Ok(()) => self.log_refinement(),
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.command_input.clear();
            return true;
        }
        if self.continue_knot_action(text) {
            return true;
        }
        if self
            .commands
            .recognizes(text.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_refinement();
            return false;
        }
        self.validate_refinement();
        self.command_input.clear();
        let Some(p) = &mut self.refinement_prompt else {
            return true;
        };
        if text.is_empty() {
            if p.selecting() {
                self.cancel_refinement();
                return true;
            }
            let p = self.refinement_prompt.take().unwrap();
            let d = p.draft.as_ref().unwrap();
            if let Err(e) = self.document.begin_transaction(p.kind.name()) {
                self.push_log(format!("Error: {e}"));
                self.refinement_prompt = Some(p);
                return true;
            }
            match d.commit(&mut self.document, p.postselected) {
                Ok(message) => match self.document.commit_transaction() {
                    Ok(_) => self.push_log(message),
                    Err(e) => self.push_log(format!("Error: {e}")),
                },
                Err(e) => {
                    let _ = self.document.rollback_transaction();
                    self.push_log(format!("Error: {e}"));
                    self.refinement_prompt = Some(p);
                }
            }
            return true;
        }
        match p.kind.accept(&mut p.options, text) {
            Ok(true) => {
                let (kind, options) = (p.kind, p.options);
                self.remember_refinement_options(kind, options);
                self.log_refinement();
                return true;
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
            _ => {}
        }
        if self.try_continue_point_constraint(text)
            || self.try_continue_point_filter(text)
            || self.try_continue_point_input(text)
        {
            return true;
        }
        self.push_log("Expected a point or surface option; Enter finishes, Esc cancels".into());
        true
    }
    pub(super) fn refinement_viewport_hit(&mut self, output: &ViewportOutput) -> bool {
        let Some(p) = &self.refinement_prompt else {
            return false;
        };
        if output.enter_pressed {
            self.continue_refinement("");
            return true;
        }
        if p.selecting() {
            if let Some(picks) = &output.parametric_click {
                let ids = picks
                    .iter()
                    .filter(|p| p.member.is_none())
                    .map(|p| p.object)
                    .filter(|id| {
                        self.document
                            .object(*id)
                            .is_some_and(|o| edits::accepts(o.geometry()))
                    })
                    .collect::<std::collections::BTreeSet<_>>();
                if ids.len() == 1 {
                    self.pick_refinement_object(ids.into_iter().next());
                } else if ids.len() > 1 {
                    self.selection_menu = Some(SelectionMenu {
                        choice: SelectionChoice {
                            mode: SelectionMode::Replace,
                            viewport: self.active_viewport,
                            object_ids: ids.into_iter().collect(),
                            pointer: output.selection_pointer.unwrap_or(egui::Pos2::ZERO),
                        },
                        highlighted: 0,
                    });
                }
                return true;
            }
        } else if let Some(t) = output.edge_parameter {
            if let Some(point) = p.cursor.as_ref().and_then(|c| c.evaluate(t).ok()) {
                self.accept_filtered_drafting_snap(point, None);
            }
            return true;
        } else if let Some(point) = output.picked_point {
            self.accept_filtered_drafting_snap(point, output.picked_snap);
            return true;
        }
        false
    }
}
