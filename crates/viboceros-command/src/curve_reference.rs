//! Resolve ordinary curves or immutable surface edges without temporary objects.
use viboceros_document::{Geometry, ObjectId};
use viboceros_geometry::{Brep, CurveRef, Frame3, GeometryError, NurbsCurve, Point3, Tolerance};

/// The shared normal plane at the closest location on a construction guide.
pub fn perpendicular_frame(curve: CurveRef<'_>, pick: Point3) -> Result<Frame3, GeometryError> {
    let parameter = curve.closest_parameter(pick, Tolerance::NUMERICAL_VALIDATION)?;
    let sample = curve.evaluate_with_tangent(parameter)?;
    Frame3::try_from_normal(
        sample.point(),
        sample.tangent().as_vector(),
        Tolerance::NUMERICAL_VALIDATION,
    )
}

pub type EdgeReference = (ObjectId, usize);

/// A component index belongs to the current topology of its named parent.
pub fn parse_edge(value: &str) -> Option<EdgeReference> {
    let (object, index) = value.split_once(',')?;
    Some((object.parse().ok()?, index.parse().ok()?))
}

pub enum ResolvedCurve<'a> {
    Borrowed(CurveRef<'a>),
    SurfaceEdge(NurbsCurve),
}
impl ResolvedCurve<'_> {
    pub fn curve(&self) -> CurveRef<'_> {
        match self {
            Self::Borrowed(curve) => *curve,
            Self::SurfaceEdge(curve) => CurveRef::NurbsCurve(curve),
        }
    }
}

pub fn resolve(
    geometry: &Geometry,
    edge: Option<usize>,
    tolerance: Tolerance,
) -> Option<ResolvedCurve<'_>> {
    let Some(index) = edge else {
        return geometry.curve_ref().map(ResolvedCurve::Borrowed);
    };
    match geometry {
        Geometry::Brep(brep) => brep
            .edges()
            .get(index)
            .map(|e| ResolvedCurve::Borrowed(CurveRef::NurbsCurve(e.curve()))),
        Geometry::NurbsSurface(surface) => {
            let brep = Brep::try_surface_face(surface.clone(), tolerance).ok()?;
            brep.edges()
                .get(index)
                .map(|e| ResolvedCurve::SurfaceEdge(e.curve().clone()))
        }
        _ => None,
    }
}

/// A selectable curve/edge and its immutable parent during a point getter.
#[derive(Clone, Debug)]
pub struct CapturedCurve {
    id: ObjectId,
    edge: Option<usize>,
    curve: std::sync::Arc<NurbsCurve>,
    parent: viboceros_document::GeometrySnapshot,
    tolerance: Tolerance,
}
impl CapturedCurve {
    pub fn capture(
        doc: &viboceros_document::Document,
        id: ObjectId,
        edge: Option<usize>,
    ) -> Result<Self, crate::CommandError> {
        let object = doc
            .object(id)
            .filter(|_| doc.is_object_selectable(id))
            .ok_or(viboceros_document::DocumentError::ObjectNotFound(id))?;
        let curve = resolve(object.geometry(), edge, doc.tolerance())
            .ok_or(crate::CommandError::Usage("Select a curve or surface edge"))?
            .curve()
            .to_nurbs()?;
        Ok(Self {
            id,
            edge,
            curve: std::sync::Arc::new(curve),
            parent: object.geometry_snapshot().clone(),
            tolerance: doc.tolerance(),
        })
    }
    pub fn source(&self) -> (&viboceros_document::GeometrySnapshot, Option<usize>) {
        (&self.parent, self.edge)
    }
    pub fn object_id(&self) -> ObjectId {
        self.id
    }
    pub fn curve(&self) -> &NurbsCurve {
        &self.curve
    }
    /// Preserve analytic evaluation for frame construction. Converting an arc
    /// to NURBS can perturb tangent components at a normal-frame axis tie.
    pub fn curve_ref(&self) -> CurveRef<'_> {
        if self.edge.is_none() {
            self.parent.curve_ref().expect("captured ordinary curve")
        } else {
            CurveRef::NurbsCurve(&self.curve)
        }
    }
    pub fn current(&self, doc: &viboceros_document::Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc.is_object_selectable(self.id)
            && doc
                .object(self.id)
                .is_some_and(|o| self.parent.shares_storage_with(o.geometry_snapshot()))
    }
}
