//! Resolve ordinary curves or immutable surface edges without temporary objects.
use viboceros_document::Geometry;
use viboceros_geometry::{Brep, CurveRef, NurbsCurve, Tolerance};

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
