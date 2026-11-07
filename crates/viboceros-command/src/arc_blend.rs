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
pub use handles::Handle;
pub const USAGE: &str = "ArcBlend Curve=id point Curve=id point [Ratio=positive] [AlternateSolution=Yes|No] [Trim=Yes|No] [Join=Yes|No] [Parameter1=t] [Parameter2=t]";
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
                "Pick a handle, Ratio, AlternateSolution, Trim or Join; Enter accepts, Esc cancels"
            }
        }
    }
    pub fn reference_count(&self) -> usize {
        self.guides.len()
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
        words.extend([
            format!("Ratio={}", self.geometry.ratio),
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
pub(super) struct ArcBlendCommand;
impl Command for ArcBlendCommand {
    fn name(&self) -> &'static str {
        "ArcBlend"
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let input = arguments::parse(doc, args)?;
        input.commit(doc)?;
        Ok("Created an arc blend".into())
    }
}
