use super::*;

impl Brep {
    /// Builds a single face from exact UV curves on a NURBS support.
    ///
    /// Supply a simple counterclockwise outer loop followed by any simple,
    /// disjoint clockwise holes. Curved spatial edges are certified to the
    /// given tolerance. The supporting surface and the UV curves are retained.
    pub fn try_parameter_face(
        surface: NurbsSurface,
        boundaries: Vec<Vec<NurbsCurve2>>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut vertices = Vec::new();
        let mut edges = Vec::new();
        let mut loops = Vec::new();
        for (index, boundary) in boundaries.into_iter().enumerate() {
            let start = vertices.len();
            let count = boundary.len();
            let mut trims = Vec::with_capacity(count);
            for (offset, curve) in boundary.into_iter().enumerate() {
                let uv = curve.start_point()?;
                let end = curve.end_point()?;
                vertices.push(BrepVertex::try_new(surface.evaluate(uv.x(), uv.y())?, 0.)?);
                let (spatial, iso) = boundary_edge(&surface, &curve, uv, end, tolerance)?;
                let incidence = [start + offset, start + (offset + 1) % count];
                trims.push(BrepTrim::try_new(
                    incidence,
                    Some(edges.len()),
                    false,
                    curve,
                    BrepTrimType::Boundary,
                    iso,
                    [0.; 2],
                )?);
                edges.push(BrepEdge::try_new(incidence, spatial, tolerance.absolute())?);
            }
            let kind = if index == 0 {
                BrepLoopType::Outer
            } else {
                BrepLoopType::Inner
            };
            loops.push(BrepLoop::try_new(kind, trims)?);
        }
        let face = BrepFace::try_new(surface, false, loops)?;
        Self::try_new(vertices, edges, vec![face], tolerance)
    }
}

fn boundary_edge(
    surface: &NurbsSurface,
    curve: &NurbsCurve2,
    start: Point2,
    end: Point2,
    tolerance: Tolerance,
) -> Result<(NurbsCurve, SurfaceIso), GeometryError> {
    if curve.control_points().len() == 2 && !curve.is_rational() {
        if start.x() == end.x() && start.y() != end.y() {
            let u = start.x();
            let domain = surface.domain_u();
            let iso = if u == *domain.start() {
                SurfaceIso::West
            } else if u == *domain.end() {
                SurfaceIso::East
            } else {
                SurfaceIso::InteriorUConstant
            };
            return iso_edge(surface.isocurve_v(u)?, start.y(), end.y(), curve, iso);
        }
        if start.y() == end.y() && start.x() != end.x() {
            let v = start.y();
            let domain = surface.domain_v();
            let iso = if v == *domain.start() {
                SurfaceIso::South
            } else if v == *domain.end() {
                SurfaceIso::North
            } else {
                SurfaceIso::InteriorVConstant
            };
            return iso_edge(surface.isocurve_u(v)?, start.x(), end.x(), curve, iso);
        }
    }
    Ok((
        surface.try_pushup_curve_certified(curve, tolerance)?,
        SurfaceIso::NotIso,
    ))
}

fn iso_edge(
    line: NurbsCurve,
    start: Real,
    end: Real,
    curve: &NurbsCurve2,
    iso: SurfaceIso,
) -> Result<(NurbsCurve, SurfaceIso), GeometryError> {
    let mut spatial = line.try_trimmed(start.min(end)..=start.max(end))?;
    if start > end {
        spatial = spatial.reversed()?;
    }
    Ok((spatial.try_reparameterized(curve.domain())?, iso))
}
