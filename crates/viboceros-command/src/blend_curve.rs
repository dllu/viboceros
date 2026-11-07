//! Adjustable curve blends keep captured sources and previews outside history.
use crate::{
    Command, CommandError, curve_reference::CapturedCurve, option_name_eq, parse_finite_real,
    parse_point,
};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{
    CurveBlendContinuity as Continuity, CurveBlendJet, CurveBlendOptions, NurbsCurve,
    ParameterSide, Point3, Real, Tolerance, UnitVector3, Vector3,
};
mod arguments;
mod commit;
mod guide;
mod options;
use guide::Guide;
pub const USAGE: &str = "BlendCrv Curve=id point Curve=id point [Continuity_1=G0|G1|G2] [Continuity_2=G0|G1|G2] [BlendStart=CurveEnd|PickPoint] [Handle1=length] [Handle2=length] [Bulge1=factor] [Bulge2=factor] [Trim=Yes|No] [Join=Yes|No]";
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Preferences {
    pub pick_point: bool,
    pub trim: bool,
    pub join: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Handle {
    Start,
    StartShape,
    EndShape,
    End,
}
#[derive(Clone, Debug)]
pub struct Input {
    guides: Vec<Guide>,
    preferences: Preferences,
    continuity: [Continuity; 2],
    handles: [Option<Real>; 2],
    bulges: [Real; 2],
    tolerance: Tolerance,
    preview: Option<NurbsCurve>,
    active: Option<Handle>,
}
impl Input {
    pub fn new(tolerance: Tolerance) -> Self {
        Self::with_preferences(tolerance, Preferences::default())
    }
    pub fn with_preferences(tolerance: Tolerance, preferences: Preferences) -> Self {
        Self {
            guides: vec![],
            preferences,
            continuity: [Continuity::Curvature; 2],
            handles: [None; 2],
            bulges: [1.; 2],
            tolerance,
            preview: None,
            active: None,
        }
    }
    pub fn continuities(&self) -> [Continuity; 2] {
        self.continuity
    }
    pub fn preferences(&self) -> Preferences {
        self.preferences
    }
    pub fn ready(&self) -> bool {
        self.guides.len() == 2
    }
    pub fn reference_count(&self) -> usize {
        self.guides.len()
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance() && self.guides.iter().all(|g| g.source.current(doc))
    }
    pub fn reset(&mut self, tolerance: Tolerance) {
        *self = Self::with_preferences(tolerance, self.preferences);
    }
    pub fn preview(&self) -> Option<&NurbsCurve> {
        self.preview.as_ref()
    }
    pub fn prompt(&self) -> &'static str {
        match self.active {
            Some(Handle::Start | Handle::End) => "Select blend contact location",
            Some(_) => "Set blend shape handle length or location",
            None if self.guides.is_empty() => "Select first curve near blend end",
            None if !self.ready() => "Select second curve near blend end",
            None => "Select a blend control point or change continuity; Enter accepts",
        }
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
        self.guides.push(Guide::capture(
            doc,
            id,
            edge,
            pick,
            self.preferences.pick_point,
        )?);
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
        let jets = [
            self.guides[0].jet(0, self.continuity[0])?,
            self.guides[1].jet(1, self.continuity[1])?,
        ];
        let options = CurveBlendOptions {
            continuity: self.continuity,
            handles: self.handles,
            bulges: self.bulges,
            endpoint_specific: true,
        };
        #[cfg(feature = "native-smlib")]
        let curve = viboceros_smlib::blend_curve_from_jets(jets, options, self.tolerance)?;
        #[cfg(not(feature = "native-smlib"))]
        let curve = viboceros_geometry::try_blend_curve_from_jets(jets, options, self.tolerance)?;
        let length = curve.length(self.tolerance)?;
        self.preview = Some(curve.try_reparameterized(0. ..=length)?);
        Ok(())
    }
    pub fn flip_source(&mut self, id: ObjectId) -> Result<(), CommandError> {
        let mut next = self.clone();
        let g = next
            .guides
            .iter_mut()
            .find(|g| g.source.object_id() == id)
            .ok_or(CommandError::Usage("Select one of the blend sources"))?;
        g.flip = !g.flip;
        next.rebuild()?;
        *self = next;
        Ok(())
    }
    pub fn flip_both(&mut self) -> Result<(), CommandError> {
        let mut next = self.clone();
        for g in &mut next.guides {
            g.flip = !g.flip;
        }
        next.rebuild()?;
        *self = next;
        Ok(())
    }
    pub fn adjust_shape_length(&mut self, length: Real) -> Result<(), CommandError> {
        let i = match self.active {
            Some(Handle::StartShape) => 0,
            Some(Handle::EndShape) => 1,
            _ => return Err(CommandError::Usage(USAGE)),
        };
        if !length.is_finite() || length <= 0. {
            return Err(CommandError::Usage("Blend handle length must be positive"));
        }
        let mut next = self.clone();
        next.handles[i] = Some(length);
        next.rebuild()?;
        *self = next;
        Ok(())
    }
    pub fn active_handle(&self) -> Option<Handle> {
        self.active
    }
    pub fn control_points(&self) -> Vec<(Handle, Point3)> {
        let Some(c) = &self.preview else {
            return vec![];
        };
        let p = c.control_points();
        let mut out = vec![
            (Handle::Start, p[0].point()),
            (Handle::End, p[p.len() - 1].point()),
        ];
        if self.continuity[0] != Continuity::Position {
            out.push((Handle::StartShape, p[1].point()));
        }
        if self.continuity[1] != Continuity::Position {
            out.push((Handle::EndShape, p[p.len() - 2].point()));
        }
        out
    }
    pub fn select_handle(&mut self, handle: Handle) -> Result<(), CommandError> {
        if !self.control_points().iter().any(|(h, _)| *h == handle) {
            return Err(CommandError::Usage(USAGE));
        }
        self.active = Some(handle);
        Ok(())
    }
    pub fn finish_adjustment(&mut self) {
        self.active = None;
    }
    pub fn adjustment_curve(&self) -> Option<&NurbsCurve> {
        let i = match self.active? {
            Handle::Start => 0,
            Handle::End => 1,
            _ => return None,
        };
        Some(self.guides[i].source.curve())
    }
    pub fn shape_constraint(&self) -> Option<crate::translation::DestinationConstraint> {
        let i = match self.active? {
            Handle::StartShape => 0,
            Handle::EndShape => 1,
            _ => return None,
        };
        let guide = &self.guides[i];
        let mut direction = guide.direction(i).ok()?;
        if i == 1 {
            direction = direction.opposite();
        }
        Some(crate::translation::DestinationConstraint {
            anchor: guide.point().ok()?,
            direction: Some(direction),
            distance: None,
        })
    }
    pub fn adjust_constraint_parameter(&mut self, t: Real) -> Result<(), CommandError> {
        let i = match self.active {
            Some(Handle::Start) => 0,
            Some(Handle::End) => 1,
            _ => return Err(CommandError::Usage(USAGE)),
        };
        let parameter = self.guides[i].source.curve_ref().parameter_from_nurbs(t)?;
        self.option(&format!("Parameter{}={parameter}", i + 1))
            .map(|_| ())
    }
    pub fn adjust_point(&mut self, point: Point3) -> Result<(), CommandError> {
        if let Some(c) = self.shape_constraint() {
            let length = c
                .anchor
                .vector_to(point)?
                .dot(c.direction.unwrap().as_vector())?;
            let i = if self.active == Some(Handle::StartShape) {
                1
            } else {
                2
            };
            return self.option(&format!("Handle{i}={length}")).map(|_| ());
        }
        let i = match self.active {
            Some(Handle::Start) => 0,
            Some(Handle::End) => 1,
            _ => return Err(CommandError::Usage(USAGE)),
        };
        let t = self.guides[i]
            .source
            .curve_ref()
            .closest_parameter(point, self.tolerance)?;
        self.option(&format!("Parameter{}={t}", i + 1)).map(|_| ())
    }
    fn freeze_handles(&mut self) {
        let Some(c) = &self.preview else { return };
        let p = c.control_points();
        for (i, (a, b)) in [
            (p[0].point(), p[1].point()),
            (p[p.len() - 1].point(), p[p.len() - 2].point()),
        ]
        .into_iter()
        .enumerate()
        {
            if self.continuity[i] != Continuity::Position && self.handles[i].is_none() {
                self.handles[i] = a.distance_to(b).ok();
            }
        }
    }
    pub fn arguments(&self) -> Result<String, CommandError> {
        if !self.ready() {
            return Err(CommandError::Usage(USAGE));
        }
        let mut words = vec!["BlendCrv".to_owned(), "BlendStart=CurveEnd".to_owned()];
        for g in &self.guides {
            words.push(g.binding()?);
        }
        for i in 0..2 {
            words.push(format!(
                "Continuity_{}={}",
                i + 1,
                options::continuity_name(self.continuity[i])
            ));
            words.push(format!("Parameter{}={}", i + 1, self.guides[i].parameter));
            words.push(format!("Flip{}={}", i + 1, yes(self.guides[i].flip)));
            words.push(if let Some(h) = self.handles[i] {
                format!("Handle{}={h}", i + 1)
            } else {
                format!("Bulge{}={}", i + 1, self.bulges[i])
            });
        }
        words.push(format!(
            "BlendStart={}",
            if self.preferences.pick_point {
                "PickPoint"
            } else {
                "CurveEnd"
            }
        ));
        words.push(format!("Trim={}", yes(self.preferences.trim)));
        words.push(format!("Join={}", yes(self.preferences.join)));
        Ok(words.join(" "))
    }
}
fn yes(v: bool) -> &'static str {
    if v { "Yes" } else { "No" }
}
#[derive(Default)]
pub(super) struct BlendCurveCommand(crate::remembered::Remembered<Preferences>);
impl Command for BlendCurveCommand {
    fn name(&self) -> &'static str {
        "BlendCrv"
    }
    fn blend_curve_preferences(&self) -> Option<Preferences> {
        Some(self.0.get())
    }
    fn remember_blend_curve_preferences(&self, value: Preferences) -> bool {
        self.0.set(value);
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let input = arguments::parse(doc, args, self.0.get())?;
        input.commit(doc)?;
        self.0.set(input.preferences());
        Ok("Created an adjustable curve blend".into())
    }
}
