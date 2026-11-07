//! A curve reference and a free point have distinct ownership and constraints.
use super::*;
#[derive(Clone, Debug)]
pub(super) enum Endpoint {
    Curve(Guide),
    Point(Point3),
}
impl Endpoint {
    pub fn curve(&self) -> Option<&Guide> {
        match self {
            Self::Curve(g) => Some(g),
            Self::Point(_) => None,
        }
    }
    pub fn curve_mut(&mut self) -> Option<&mut Guide> {
        match self {
            Self::Curve(g) => Some(g),
            Self::Point(_) => None,
        }
    }
    pub fn point(&self) -> Result<Point3, CommandError> {
        match self {
            Self::Curve(g) => g.point(),
            Self::Point(p) => Ok(*p),
        }
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.curve().is_none_or(|g| g.source.current(doc))
    }
    pub fn jet(&self, i: usize, order: u8) -> Result<CurveBlendJet, CommandError> {
        match self {
            Self::Curve(g) => g.jet(i, order),
            Self::Point(p) => Ok(CurveBlendJet::new(*p, None, Vector3::try_new(0., 0., 0.)?)),
        }
    }
    pub fn binding(&self) -> Result<String, CommandError> {
        match self {
            Self::Curve(g) => g.binding(),
            Self::Point(p) => Ok(format!(
                "Point={}",
                p.to_array().map(|v| v.to_string()).join(",")
            )),
        }
    }
}
impl Input {
    pub(super) fn add_point(&mut self, point: Point3) -> Result<(), CommandError> {
        if self.ready()
            || self
                .endpoints
                .iter()
                .any(|p| matches!(p, Endpoint::Point(_)))
        {
            return Err(CommandError::Usage(
                "BlendCrv accepts at most one point endpoint",
            ));
        }
        self.continuity[self.endpoints.len()] = 0;
        self.endpoints.push(Endpoint::Point(point));
        Ok(())
    }
    /// Point endpoints are positional constraints and do not own a document object.
    pub fn pick_point(&mut self, point: Point3) -> Result<(), CommandError> {
        let mut next = self.clone();
        next.add_point(point)?;
        next.rebuild()?;
        *self = next;
        Ok(())
    }
    /// A free contact uses ordinary drafting instead of projection onto a source.
    pub fn point_adjustment_anchor(&self) -> Option<Point3> {
        let i = match self.active? {
            Handle::Start => 0,
            Handle::End => 1,
            _ => return None,
        };
        match self.endpoints.get(i)? {
            Endpoint::Point(p) => Some(*p),
            _ => None,
        }
    }
    pub fn has_point_endpoint(&self) -> bool {
        self.endpoints
            .iter()
            .any(|e| matches!(e, Endpoint::Point(_)))
    }
    pub fn point_input_anchor(&self) -> Option<Point3> {
        self.point_adjustment_anchor()
            .or_else(|| self.endpoints.last()?.point().ok())
    }
}

impl Input {
    pub(super) fn add_subcurve(
        &mut self,
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        parameters: [Real; 2],
        wrap: bool,
    ) -> Result<(), CommandError> {
        if self.ready() {
            return Err(CommandError::Usage(USAGE));
        }
        if self.surface_edges {
            return Err(CommandError::Usage("Enter Curves before selecting SubCrv"));
        }
        self.endpoints.push(Endpoint::Curve(Guide::capture(
            doc,
            id,
            edge,
            None,
            self.preferences.pick_point,
            Some((parameters, wrap)),
            false,
        )?));
        Ok(())
    }
    /// A directed private portion retains its original document parent.
    pub fn pick_subcurve(
        &mut self,
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        parameters: [Real; 2],
        wrap: bool,
    ) -> Result<(), CommandError> {
        let mut next = self.clone();
        next.add_subcurve(doc, id, edge, parameters, wrap)?;
        next.rebuild()?;
        *self = next;
        Ok(())
    }
}
