//! Adjustable curve blends keep captured sources and previews outside history.
use crate::{
    Command, CommandError, curve_reference::CapturedCurve, option_name_eq, parse_finite_real,
    parse_point,
};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{
    CurveBlendJet, CurveBlendJetOptions, NurbsCurve, ParameterSide, Point3, Real, Tolerance,
    UnitVector3, Vector3,
};
mod adjustment;
mod arguments;
mod commit;
mod endpoint;
mod guide;
use endpoint::Endpoint;
mod options;
use guide::Guide;
pub const USAGE: &str = "BlendCrv Curve=id point|Point=x,y,z Curve=id point|Point=x,y,z [Continuity_1=G0|G1|G2|G3|G4] [Continuity_2=G0|G1|G2|G3|G4] [BlendStart=CurveEnd|PickPoint] [Handle1=length] [Handle2=length] [Bulge1=factor] [Bulge2=factor] [Trim=Yes|No] [Join=Yes|No]";
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
    endpoints: Vec<Endpoint>,
    preferences: Preferences,
    continuity: [u8; 2],
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
            endpoints: vec![],
            preferences,
            continuity: [2; 2],
            handles: [None; 2],
            bulges: [1.; 2],
            tolerance,
            preview: None,
            active: None,
        }
    }
    pub fn continuities(&self) -> [u8; 2] {
        self.continuity
    }
    pub fn preferences(&self) -> Preferences {
        self.preferences
    }
    pub fn ready(&self) -> bool {
        self.endpoints.len() == 2
    }
    pub fn reference_count(&self) -> usize {
        self.endpoints.len()
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance() && self.endpoints.iter().all(|g| g.current(doc))
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
            None if self.endpoints.is_empty() => "Select first curve near blend end or Point",
            None if !self.ready() && !self.has_point_endpoint() => {
                "Select second curve near blend end or Point"
            }
            None if !self.ready() => "Select a curve near the blend end",
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
        self.endpoints.push(Endpoint::Curve(Guide::capture(
            doc,
            id,
            edge,
            pick,
            self.preferences.pick_point,
        )?));
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
            self.endpoints[0].jet(0, self.continuity[0])?,
            self.endpoints[1].jet(1, self.continuity[1])?,
        ];
        let options = CurveBlendJetOptions {
            orders: self.continuity,
            handles: self.handles,
            bulges: self.bulges,
            endpoint_specific: true,
        };
        #[cfg(feature = "native-smlib")]
        let curve = viboceros_smlib::blend_curve_from_endpoint_jets(jets, options, self.tolerance)?;
        #[cfg(not(feature = "native-smlib"))]
        let curve =
            viboceros_geometry::try_blend_curve_from_endpoint_jets(jets, options, self.tolerance)?;
        // The resulting parameter domain is a numerical length measurement,
        // not a modelling predicate. A coarse document tolerance must not
        // coarsen the script-visible knot interval of a high-degree blend.
        let measurement = Tolerance::try_new(
            self.tolerance.absolute().min(Tolerance::DEFAULT.absolute()),
            self.tolerance.relative().min(Tolerance::DEFAULT.relative()),
            self.tolerance.angular(),
        )?;
        let length = curve.length(measurement)?;
        self.preview = Some(curve.try_reparameterized(0. ..=length)?);
        Ok(())
    }
    pub fn flip_source(&mut self, id: ObjectId) -> Result<(), CommandError> {
        let mut next = self.clone();
        let g = next
            .endpoints
            .iter_mut()
            .filter_map(Endpoint::curve_mut)
            .find(|g| g.source.object_id() == id)
            .ok_or(CommandError::Usage("Select one of the blend sources"))?;
        g.flip = !g.flip;
        next.rebuild()?;
        *self = next;
        Ok(())
    }
    pub fn flip_both(&mut self) -> Result<(), CommandError> {
        let mut next = self.clone();
        for g in next.endpoints.iter_mut().filter_map(Endpoint::curve_mut) {
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
    pub fn arguments(&self) -> Result<String, CommandError> {
        if !self.ready() {
            return Err(CommandError::Usage(USAGE));
        }
        let mut words = vec!["BlendCrv".to_owned(), "BlendStart=CurveEnd".to_owned()];
        for g in &self.endpoints {
            words.push(g.binding()?);
        }
        for i in 0..2 {
            words.push(format!(
                "Continuity_{}={}",
                i + 1,
                options::continuity_name(self.continuity[i])
            ));
            let Some(guide) = self.endpoints[i].curve() else {
                continue;
            };
            words.push(format!("Parameter{}={}", i + 1, guide.parameter));
            words.push(format!("Flip{}={}", i + 1, yes(guide.flip)));
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
