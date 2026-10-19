//! Repeated point getter: geometry remains immutable until Enter.
use super::*;
use viboceros_command::refinement::{self as edits, Options};
use viboceros_geometry::NurbsCurve;
mod subcurve;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Kink,
    Knot,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Kink => "InsertKink",
            Self::Knot => "InsertKnot",
        }
    }
    fn noun(self) -> &'static str {
        match self {
            Self::Kink => "a kink",
            Self::Knot => "a knot",
        }
    }
}
#[derive(Debug)]
enum Draft {
    Kink(viboceros_command::insert_kink::Draft),
    Knot(viboceros_command::insert_knot::Draft),
}
impl std::ops::Deref for Draft {
    type Target = viboceros_command::refinement::Draft;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Kink(d) => d,
            Self::Knot(d) => d,
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
            Kind::Kink => Self::Kink(viboceros_command::insert_kink::Draft::new(doc, id, range)?),
            Kind::Knot => Self::Knot(viboceros_command::insert_knot::Draft::new(doc, id, range)?),
        })
    }
    fn point(
        &mut self,
        point: Point3,
        options: Options,
    ) -> Result<(), viboceros_command::CommandError> {
        match self {
            Self::Kink(d) => d.point(point, options),
            Self::Knot(d) => d.point(point, options),
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
    guides: Vec<NurbsCurve>,
    guide_points: Vec<Point3>,
}
impl Prompt {
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
        self.cursor.as_ref()
    }
    pub(super) fn hint(&self) -> &str {
        &self.hint
    }
    pub(super) fn guides(&self) -> &[NurbsCurve] {
        &self.guides
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
        if self.kind == Kind::Knot
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
            if self.subcurve {
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
        } else if self.cursor.is_some() {
            format!(
                "Point on curve to add {}; {} accepted; {}Enter finishes; Esc discards all picks",
                self.kind.noun(),
                self.draft.as_ref().unwrap().picks(),
                if self.kind == Kind::Knot {
                    "type a parameter, Automatic or Undo; "
                } else {
                    ""
                }
            )
        } else {
            format!(
                "Point on surface to add {} ({} Toggle); {}Enter finishes; Esc discards all picks",
                self.kind.noun(),
                self.options.arguments(),
                if self.kind == Kind::Knot {
                    "Automatic or Undo; "
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
            .filter(|p| p.kind == Kind::Knot && !p.shortening)
        else {
            return false;
        };
        let Some(Draft::Knot(d)) = p.draft.as_mut() else {
            return false;
        };
        let key = text.trim_start_matches(['_', '-']);
        let result = if key.eq_ignore_ascii_case("Automatic") {
            d.automatic(p.options)
        } else if key.eq_ignore_ascii_case("Undo") {
            if !d.undo() {
                self.push_log("No staged knot edits to undo".into());
            } else {
                self.log_refinement();
            }
            return true;
        } else if !text.contains(',')
            && let Ok(parameter) = text.parse::<f64>()
        {
            d.parameter(parameter, p.options)
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
        } else {
            return false;
        };
        let mut options = match kind {
            Kind::Kink => self.commands.insert_kink_options(),
            Kind::Knot => self.commands.insert_knot_options(),
        };
        let mut subcurve = false;
        for w in words {
            if w.trim_start_matches('_').eq_ignore_ascii_case("SubCrv") {
                subcurve = true;
            } else {
                match options.accept(w) {
                    Ok(true) => {}
                    _ => return false,
                }
            }
        }
        let ids = self
            .document
            .selected_objects()
            .filter(|o| self.document.is_object_selectable(o.id()) && edits::accepts(o.geometry()))
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
            guides: Vec::new(),
            guide_points: Vec::new(),
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
        }
    }
    fn log_refinement(&mut self) {
        if let Some(p) = &mut self.refinement_prompt {
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
        match Draft::new(&self.document, id, None, p.kind) {
            Ok(draft) => {
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
        match d.point(point, p.options) {
            Ok(()) => {
                self.log_refinement();
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
            && self
                .refinement_prompt
                .as_ref()
                .is_some_and(|p| p.selecting() && !p.shortening)
        {
            let p = self.refinement_prompt.as_mut().unwrap();
            p.subcurve = true;
            p.shortening = true;
            p.input = Default::default();
            self.log_refinement();
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
        match p.options.accept(text) {
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
