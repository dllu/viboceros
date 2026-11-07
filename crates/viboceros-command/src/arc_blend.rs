//! Ordered immutable curve-end references and adjustable tangent biarcs.
use crate::{
    Command, CommandError, curve_reference::CapturedCurve, option_name_eq, parse_finite_real,
    parse_point,
};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{ArcBlendOptions, NurbsCurve, Point3, PolyCurve3, Real, Tolerance};
mod arguments;
mod commit;
mod handles;
mod options;
mod preferences;
pub use handles::Handle;
pub use preferences::Preferences;
pub const USAGE: &str = "ArcBlend Curve=id point Curve=id point [Ratio=nonnegative|RadiusDifference=number] [AlternateSolution=Yes|No] [Trim=Yes|No] [Join=Yes|No] [Parameter1=t] [Parameter2=t]";
#[derive(Clone, Debug)]
struct Guide {
    source: CapturedCurve,
    parameter: Real,
    end: bool,
}
#[derive(Clone, Debug)]
pub struct Input {
    guides: Vec<Guide>,
    geometry: ArcBlendOptions,
    radius_difference: Option<Real>,
    last_radius_difference: Real,
    radius_target_reached: bool,
    radius_target_deferred: bool,
    trim: bool,
    join: bool,
    tolerance: Tolerance,
    curve: Option<PolyCurve3>,
    preview: Option<NurbsCurve>,
    active_handle: Option<Handle>,
    adjustment_curve: Option<handles::AdjustmentCurve>,
}
impl Input {
    pub fn new(tolerance: Tolerance) -> Self {
        Self {
            guides: vec![],
            geometry: Default::default(),
            radius_difference: None,
            last_radius_difference: 0.,
            radius_target_reached: false,
            radius_target_deferred: false,
            trim: false,
            join: false,
            tolerance,
            curve: None,
            preview: None,
            active_handle: None,
            adjustment_curve: None,
        }
    }
    pub fn reset(&mut self, tolerance: Tolerance) {
        self.guides.clear();
        self.curve = None;
        self.preview = None;
        self.active_handle = None;
        self.adjustment_curve = None;
        self.radius_target_reached = false;
        self.radius_target_deferred = false;
        self.tolerance = tolerance;
    }
    pub fn ready(&self) -> bool {
        self.guides.len() == 2
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance() && self.guides.iter().all(|g| g.source.current(doc))
    }
    pub fn preview(&self) -> Option<&NurbsCurve> {
        self.preview.as_ref()
    }
    pub fn prompt(&self) -> &'static str {
        if let Some(handle) = self.active_handle {
            return match handle {
                Handle::Start => "Select start point location",
                Handle::End => "Select end point location",
                Handle::Ratio => "Set arc control polygon length ratio",
            };
        }
        match self.guides.len() {
            0 => "Select first curve near end",
            1 => "Select second curve near end",
            _ => {
                "Pick a handle, Ratio, RadiusDifference, AlternateSolution or Trim; Enter accepts, Esc cancels"
            }
        }
    }
    pub fn reference_count(&self) -> usize {
        self.guides.len()
    }
    pub fn radius_difference(&self) -> Option<Real> {
        self.radius_difference
    }
    pub fn last_radius_difference(&self) -> Real {
        self.last_radius_difference
    }
    /// The ratio getter edits its target option without accepting a new ratio.
    pub fn remember_radius_difference(&mut self, target: Real) -> Result<(), CommandError> {
        if !target.is_finite() {
            return Err(CommandError::Usage(
                "ArcBlend radius difference must be finite",
            ));
        }
        self.last_radius_difference = target;
        if self.radius_difference.is_some_and(|old| old != target) {
            self.radius_difference = Some(target);
            self.radius_target_deferred = true;
            self.radius_target_reached = self.curve.as_ref().is_some_and(|curve| {
                let [
                    viboceros_geometry::CurveSegment3::Arc(a),
                    viboceros_geometry::CurveSegment3::Arc(b),
                ] = curve.segments()
                else {
                    return false;
                };
                (a.radius() - b.radius() - target).abs() <= 1. / 4_294_967_296.
            });
        }
        Ok(())
    }
    pub fn warning(&self) -> Option<&'static str> {
        (self.preview.is_some() && self.radius_difference.is_some() && !self.radius_target_reached)
            .then_some("Unable to achieve requested radius difference")
    }
    fn add(
        &mut self,
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        pick: Point3,
    ) -> Result<(), CommandError> {
        if self.ready() {
            return Err(CommandError::Usage(USAGE));
        }
        let source = CapturedCurve::capture(doc, id, edge)?;
        let c = source.curve_ref();
        if c.is_closed()? {
            return Err(CommandError::Usage("ArcBlend requires open curve ends"));
        }
        let a = c.evaluate(*c.domain().start())?;
        let b = c.evaluate(*c.domain().end())?;
        let end = pick.distance_to(b)? < pick.distance_to(a)?;
        let parameter = if end {
            *c.domain().end()
        } else {
            *c.domain().start()
        };
        self.guides.push(Guide {
            source,
            parameter,
            end,
        });
        Ok(())
    }
    pub fn pick_curve(
        &mut self,
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        pick: Point3,
    ) -> Result<(), CommandError> {
        let mut next = self.clone();
        next.add(doc, id, edge, pick)?;
        next.rebuild()?;
        *self = next;
        Ok(())
    }
    fn rebuild(&mut self) -> Result<(), CommandError> {
        if !self.ready() {
            return Ok(());
        }
        let jets = self
            .guides
            .iter()
            .map(|g| g.source.curve_ref().evaluate_with_tangent(g.parameter))
            .collect::<Result<Vec<_>, _>>()?;
        let a = if self.guides[0].end {
            jets[0].tangent()
        } else {
            jets[0].tangent().opposite()
        };
        let b = if self.guides[1].end {
            jets[1].tangent().opposite()
        } else {
            jets[1].tangent()
        };
        if let Some(difference) = self.radius_difference {
            let solution = viboceros_geometry::try_arc_blend_radius_ratio(
                jets[0].point(),
                a.as_vector(),
                jets[1].point(),
                b.as_vector(),
                difference,
                self.geometry.alternate,
            )?;
            self.geometry.ratio = solution.ratio;
            self.radius_target_reached = solution.reached_target;
        }
        let curve = viboceros_geometry::try_arc_blend(
            jets[0].point(),
            a.as_vector(),
            jets[1].point(),
            b.as_vector(),
            self.geometry,
            self.tolerance,
        )?;
        self.preview = Some(curve.to_nurbs()?);
        self.curve = Some(curve);
        self.radius_target_deferred = false;
        Ok(())
    }
    pub fn arguments(&self) -> Result<String, CommandError> {
        if !self.ready() {
            return Err(CommandError::Usage(USAGE));
        }
        let mut words = vec!["ArcBlend".to_owned()];
        for g in &self.guides {
            let edge = g.source.source().1;
            words.push(edge.map_or_else(
                || format!("Curve={}", g.source.object_id()),
                |e| format!("Edge={},{}", g.source.object_id(), e),
            ));
            let c = g.source.curve_ref();
            let p = c.evaluate(if g.end {
                *c.domain().end()
            } else {
                *c.domain().start()
            })?;
            words.push(p.to_array().map(|x| x.to_string()).join(","));
        }
        // The ratio getter can change a target without accepting another
        // curve. Preserve that preview when committing its command arguments.
        words.extend([
            self.radius_difference
                .filter(|_| !self.radius_target_deferred)
                .map_or_else(
                    || format!("Ratio={}", self.geometry.ratio),
                    |r| format!("RadiusDifference={r}"),
                ),
            format!("AlternateSolution={}", yes(self.geometry.alternate)),
            format!("Trim={}", yes(self.trim)),
        ]);
        if self.trim {
            words.push(format!("Join={}", yes(self.join)));
        }
        for (i, g) in self.guides.iter().enumerate() {
            words.push(format!("Parameter{}={}", i + 1, g.parameter));
        }
        Ok(words.join(" "))
    }
}
fn yes(v: bool) -> &'static str {
    if v { "Yes" } else { "No" }
}
fn boolean(v: &str) -> Result<bool, CommandError> {
    crate::parse_yes_no(v.trim_start_matches('_')).ok_or(CommandError::Usage(USAGE))
}
#[derive(Default)]
pub(super) struct ArcBlendCommand(crate::remembered::Remembered<Preferences>);
impl Command for ArcBlendCommand {
    fn name(&self) -> &'static str {
        "ArcBlend"
    }
    fn arc_blend_preferences(&self) -> Option<Preferences> {
        Some(self.0.get())
    }
    fn remember_arc_blend_preferences(&self, value: Preferences) -> bool {
        if !value.radius_difference.is_finite() {
            return false;
        }
        self.0.set(value);
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let input = arguments::parse(doc, args, self.0.get())?;
        input.commit(doc)?;
        self.0.set(input.preferences());
        Ok(input.warning().map_or_else(
            || "Created an arc blend".into(),
            |warning| format!("{warning}; created an arc blend"),
        ))
    }
}
