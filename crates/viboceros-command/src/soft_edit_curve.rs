//! Local curve editing with reusable, immutable falloff previews.
use super::*;
use crate::remembered::Remembered;
use viboceros_geometry::CurveSoftEdit;

pub const USAGE: &str =
    "SoftEditCrv reference target [Distance=value] [FixEnds=Yes|No] [Copy=Yes|No]";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub distance: Real,
    pub fix_ends: bool,
    pub copy: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            distance: 1.,
            fix_ends: false,
            copy: false,
        }
    }
}
impl Options {
    pub fn accept(&mut self, word: &str) -> Result<bool, CommandError> {
        let Some((key, value)) = word.split_once('=') else {
            return Ok(false);
        };
        let mut next = *self;
        if option_name_eq(key, "Distance") {
            next.distance = parse_finite_real(value)?;
            if next.distance <= 0. {
                return Err(CommandError::Usage(USAGE));
            }
        } else if option_name_eq(key, "FixEnds") {
            next.fix_ends = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        } else if option_name_eq(key, "Copy") {
            next.copy = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        } else {
            return Ok(false);
        }
        *self = next;
        Ok(true)
    }
    pub fn arguments(self) -> String {
        format!(
            "Distance={} FixEnds={} Copy={}",
            self.distance,
            if self.fix_ends { "Yes" } else { "No" },
            if self.copy { "Yes" } else { "No" }
        )
    }
}

#[derive(Default)]
pub(super) struct SoftEditCurveCommand(Remembered<Options>);
impl Command for SoftEditCurveCommand {
    fn name(&self) -> &'static str {
        "SoftEditCrv"
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn soft_edit_curve_options(&self) -> Option<Options> {
        Some(self.0.get())
    }
    fn remember_soft_edit_curve_options(&self, value: Options) -> bool {
        if !value.distance.is_finite() || value.distance <= 0. {
            return false;
        }
        self.0.set(value);
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_selected(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.run_selected(doc, args, true)
    }
}
impl SoftEditCurveCommand {
    fn run_selected(
        &self,
        doc: &mut Document,
        args: &[&str],
        post: bool,
    ) -> Result<String, CommandError> {
        let ids = doc
            .selected_objects()
            .filter(|o| doc.is_object_selectable(o.id()) && o.geometry().curve_ref().is_some())
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let [id] = ids.as_slice() else {
            return Err(CommandError::Usage(USAGE));
        };
        let mut draft = Draft::new(doc, *id)?;
        let mut options = self.0.get();
        let mut points = Vec::new();
        let mut i = 0;
        while i < args.len() {
            if options.accept(args[i])? {
                i += 1;
            } else {
                let (point, n) = parse_point(&args[i..])?;
                points.push(point);
                i += n;
            }
        }
        draft.set_options(options)?;
        for point in points {
            draft.point(point)?;
        }
        if draft.anchor.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let result = draft.commit(doc, post)?;
        self.0.set(draft.options());
        Ok(result)
    }
}

#[derive(Clone, Debug)]
pub struct Draft {
    state: crate::soft_edit_state::SoftEditState,
    options: Options,
    curve: NurbsCurve,
    anchor: Option<Point3>,
    parameter: Option<Real>,
    prepared: Option<CurveSoftEdit>,
}
impl std::ops::Deref for Draft {
    type Target = crate::refinement::Draft;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
impl Draft {
    pub fn new(doc: &Document, id: ObjectId) -> Result<Self, CommandError> {
        let state = crate::refinement::Draft::new(doc, id, None, "SoftEditCrv")?;
        let curve = state
            .result
            .curve_ref()
            .ok_or(CommandError::Usage(USAGE))?
            .to_nurbs()?;
        Ok(Self {
            state: crate::soft_edit_state::SoftEditState::new(state),
            curve,
            options: Options::default(),
            anchor: None,
            parameter: None,
            prepared: None,
        })
    }
    pub fn options(&self) -> Options {
        self.options
    }
    pub fn anchor(&self) -> Option<Point3> {
        self.anchor
    }
    pub fn cancel_offset(&mut self) {
        self.anchor = None;
        self.parameter = None;
        self.prepared = None;
    }
    pub fn distance_to_pick(&self, point: Point3) -> Result<Real, CommandError> {
        let anchor = self.anchor.ok_or(CommandError::Usage(USAGE))?;
        let b = self.curve.closest_parameter(point, self.state.tolerance)?;
        Ok(anchor.distance_to(self.curve.evaluate(b)?)?)
    }
    pub fn set_options(&mut self, options: Options) -> Result<(), CommandError> {
        if !options.distance.is_finite() || options.distance <= 0. {
            return Err(CommandError::Usage(USAGE));
        }
        let prepared = self
            .parameter
            .map(|t| {
                self.curve.prepare_soft_edit(
                    t,
                    options.distance,
                    options.fix_ends,
                    self.state.tolerance,
                )
            })
            .transpose()?;
        self.options = options;
        self.prepared = prepared;
        Ok(())
    }
    pub fn marker_points(&self) -> Result<Vec<Point3>, CommandError> {
        Ok(self.curve.greville_points_in_active_domain()?)
    }
    pub fn point(&mut self, point: Point3) -> Result<(), CommandError> {
        if let Some(anchor) = self.anchor {
            if viboceros_geometry::curve_points_coincident(anchor, point) {
                return Err(CommandError::Usage(
                    "Offset point must differ from the base point",
                ));
            }
            let delta = anchor.vector_to(point)?;
            let curve = self.prepared.as_ref().unwrap().apply(delta)?;
            if !self.options.copy {
                self.curve = curve.clone();
            }
            self.state
                .accept(Geometry::NurbsCurve(curve), self.options.copy);
            self.anchor = None;
            self.parameter = None;
            self.prepared = None;
        } else {
            let mut t = self.curve.closest_parameter(point, self.state.tolerance)?;
            let parameters = self.curve.greville_parameters_in_active_domain()?;
            // A point getter accepts coincidence at the middle of a knot
            // span before polishing the projection. Retain that exact site
            // when an ulp of projection error changes the selected control.
            for pair in self
                .curve
                .knots()
                .windows(2)
                .filter(|p| p[0] < p[1] && p[0] <= t && t <= p[1])
            {
                let middle = pair[0].midpoint(pair[1]);
                let scale = pair[0].abs().max(pair[1].abs()).max(1.);
                if (middle - t).abs() <= 64. * Real::EPSILON * scale
                    && viboceros_geometry::curve_points_coincident(
                        self.curve.evaluate(middle)?,
                        point,
                    )
                {
                    t = middle;
                    break;
                }
            }
            let t = parameters
                .into_iter()
                .map(|parameter| (parameter, (parameter - t).abs()))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap()
                .0;
            let prepared = self.curve.prepare_soft_edit(
                t,
                self.options.distance,
                self.options.fix_ends,
                self.state.tolerance,
            )?;
            self.anchor = Some(prepared.anchor());
            self.parameter = Some(t);
            self.prepared = Some(prepared);
        }
        Ok(())
    }
    pub fn preview(&self, point: Point3) -> Result<Option<Geometry>, CommandError> {
        let Some(anchor) = self.anchor else {
            return Ok(None);
        };
        Ok(Some(Geometry::NurbsCurve(
            self.prepared
                .as_ref()
                .unwrap()
                .apply(anchor.vector_to(point)?)?,
        )))
    }
    pub fn source_curve(&self) -> &NurbsCurve {
        &self.curve
    }
    pub fn scene(&self, doc: &Document, hover: Option<Point3>) -> Result<Document, CommandError> {
        let hover = hover.map(|p| self.preview(p)).transpose()?.flatten();
        self.state.scene(doc, hover, self.options.copy)
    }
    pub fn commit(&self, doc: &mut Document, _post: bool) -> Result<String, CommandError> {
        self.state.commit(doc, true, false)
    }
}
