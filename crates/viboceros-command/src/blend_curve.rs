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
mod controls;
mod endpoint;
mod guide;
mod surface;
use endpoint::Endpoint;
mod options;
mod shape;
use guide::Guide;
pub const USAGE: &str = "BlendCrv source source (Curve=id point|Edges Edge=id,index point Curves|Point=x,y,z|SubCrv[Wrap]=id,start,end) [Continuity_1=G0|G1|G2|G3|G4] [Continuity_2=G0|G1|G2|G3|G4] [BlendStart=CurveEnd|PickPoint] [AtAngle_1=degrees] [AtAngle_2=degrees] [Handle1=length] [Handle2=length] [Bulge1=factor] [Bulge2=factor] [Spacing1=r2,r3,r4] [Spacing2=r2,r3,r4] [Trim=Yes|No] [Join=Yes|No]";
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
    StartControl(u8),
    EndShape,
    EndControl(u8),
    End,
}
#[derive(Clone, Debug)]
pub struct Input {
    endpoints: Vec<Endpoint>,
    preferences: Preferences,
    continuity: [u8; 2],
    handles: [Option<Real>; 2],
    bulges: [Real; 2],
    spacing: [[Real; 3]; 2],
    shape_origin: Option<Point3>,
    shape_distance: Option<(Real, Real)>,
    tolerance: Tolerance,
    preview: Option<NurbsCurve>,
    active: Option<Handle>,
    symmetric: bool,
    angle_editing: bool,
    surface_edges: bool,
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
            spacing: [[1.; 3]; 2],
            shape_origin: None,
            shape_distance: None,
            tolerance,
            preview: None,
            active: None,
            symmetric: false,
            angle_editing: false,
            surface_edges: false,
        }
    }
    pub fn continuities(&self) -> [u8; 2] {
        self.continuity
    }
    pub fn preferences(&self) -> Preferences {
        self.preferences
    }
    /// Surface-edge mode uses supporting-face constraints at the clicked point.
    pub fn surface_edges_only(&self) -> bool {
        self.surface_edges
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
            Some(_) if self.angle_editing => "Set the handle angle in degrees or pick a direction",
            Some(_) if self.secondary_control_adjustment() => {
                "Move the control along its tangent; a distance then a point chooses the direction"
            }
            Some(Handle::Start | Handle::End) => "Select blend contact location",
            Some(_) if self.paired_handle().is_some() => {
                "Set paired blend shape handle length or location"
            }
            Some(_) => "Set blend shape handle length or location",
            None if !self.ready() && self.surface_edges => "Select a surface edge; Curves or Point",
            None if self.endpoints.is_empty() => {
                "Select first curve near blend end; Point or SubCrv"
            }
            None if !self.ready() && !self.has_point_endpoint() => {
                "Select second curve near blend end; Point or SubCrv"
            }
            None if !self.ready() => "Select a curve near the blend end or SubCrv",
            None if self
                .endpoints
                .iter()
                .filter_map(Endpoint::curve)
                .any(|g| g.surface.is_some()) =>
            {
                "Select a blend control point or set AtAngle_1/2; Enter accepts"
            }
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
        if self.surface_edges && edge.is_none() {
            return Err(CommandError::Usage("Select a surface edge or enter Curves"));
        }
        self.endpoints.push(Endpoint::Curve(Guide::capture(
            doc,
            id,
            edge,
            Some(pick),
            self.preferences.pick_point,
            None,
            self.surface_edges,
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
            self.endpoints[0]
                .jet(0, self.continuity[0])?
                .with_control_spacing_ratios(self.spacing[0])?,
            self.endpoints[1]
                .jet(1, self.continuity[1])?
                .with_control_spacing_ratios(self.spacing[1])?,
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
        if g.surface.is_some() {
            return Err(CommandError::Usage(
                "Surface-edge direction is controlled by AtAngle",
            ));
        }
        g.flip = !g.flip;
        next.rebuild()?;
        *self = next;
        Ok(())
    }
    pub fn flip_both(&mut self) -> Result<(), CommandError> {
        let mut next = self.clone();
        for g in next.endpoints.iter_mut().filter_map(Endpoint::curve_mut) {
            if g.surface.is_none() {
                g.flip = !g.flip;
            }
        }
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
            if let Some(surface) = &guide.surface {
                words.push(format!("AtAngle_{}={}", i + 1, surface.angle));
            }
            words.push(if let Some(h) = self.handles[i] {
                format!("Handle{}={h}", i + 1)
            } else {
                format!("Bulge{}={}", i + 1, self.bulges[i])
            });
            if self.spacing[i] != [1.; 3] {
                words.push(format!(
                    "Spacing{}={}",
                    i + 1,
                    self.spacing[i].map(|v| v.to_string()).join(",")
                ));
            }
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
