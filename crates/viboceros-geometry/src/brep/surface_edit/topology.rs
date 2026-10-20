//! Explicitly bounded component uncertainty for topology-preserving deformation.
use super::*;
use crate::exact_scalar::rational;

impl Brep {
    /// Deform a natural rectangular face while retaining its seam and pole
    /// topology. Spatial edges use the first incident exact isocurve. Every
    /// other incident image receives an outward control-hull deviation bound;
    /// singular images receive the analogous point bound. These measured
    /// component tolerances include one model tolerance for evaluation roundoff;
    /// the document model tolerance is unchanged.
    ///
    /// This is distinct from `try_with_edited_single_surface`, whose boundaries
    /// must fit at model tolerance. Nonrectangular faces use that strict path.
    /// Unsupported rational gauges or boundary bases fail without mutation.
    pub fn try_with_topology_preserving_surface_edit(
        &self,
        surface: NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let [face] = self.faces.as_slice() else {
            return invalid("surface deformation requires one face");
        };
        if surface.domain_u() != face.surface.domain_u()
            || surface.domain_v() != face.surface.domain_v()
        {
            return invalid("surface deformation must retain UV domains");
        }
        if surface == face.surface {
            return Ok(self.clone());
        }
        if face.rectangular_trim_bounds(tolerance)?.is_none()
            || face.loops.len() != 1
            || face.loops[0].trims.len() != 4
            || face.loops[0]
                .trims
                .iter()
                .any(|t| t.curve.degree() != 1 || t.curve.control_points().len() != 2)
        {
            return self.try_with_edited_single_surface(surface, tolerance);
        }
        let trims = &face.loops[0].trims;
        let images = trims
            .iter()
            .map(|trim| isocurve(&surface, trim))
            .collect::<Result<Vec<_>, _>>()?;
        let mut vertices = self.vertices.clone();
        let mut assigned = vec![false; vertices.len()];
        for (trim, image) in trims.iter().zip(&images) {
            for (end, p) in [
                image.evaluate(*image.domain().start())?,
                image.evaluate(*image.domain().end())?,
            ]
            .into_iter()
            .enumerate()
            {
                let i = trim.vertices[end];
                if !assigned[i] {
                    vertices[i].point = p;
                    assigned[i] = true;
                }
                vertices[i].tolerance = vertices[i].tolerance.max(with_model_precision(
                    distance_bound(vertices[i].point, p)?,
                    tolerance,
                )?);
            }
            if trim.edge.is_none() {
                let i = trim.vertices[0];
                vertices[i].tolerance = vertices[i].tolerance.max(with_model_precision(
                    point_hull_bound(image, vertices[i].point)?,
                    tolerance,
                )?);
            }
        }
        let mut edges = self.edges.clone();
        let mut assigned = vec![false; edges.len()];
        for (trim, image) in trims.iter().zip(&images) {
            let Some(i) = trim.edge else { continue };
            if !assigned[i] {
                edges[i].curve = if trim.reversed_3d {
                    image.reversed()?
                } else {
                    image.clone()
                };
                assigned[i] = true;
            }
            let oriented = if trim.reversed_3d {
                edges[i].curve.reversed()?
            } else {
                edges[i].curve.clone()
            };
            edges[i].tolerance = edges[i].tolerance.max(with_model_precision(
                correspondence_hull_bound(image, &oriented)?,
                tolerance,
            )?);
        }
        Self::try_new(
            vertices,
            edges,
            vec![BrepFace::try_new(
                surface,
                face.reversed,
                face.loops.clone(),
            )?],
            tolerance,
        )
    }
}

fn isocurve(surface: &NurbsSurface, trim: &BrepTrim) -> Result<NurbsCurve, GeometryError> {
    let a = trim.curve.start_point()?;
    let b = trim.curve.end_point()?;
    let (curve, start, end) = if a.x() == b.x() {
        (surface.isocurve_v(a.x())?, a.y(), b.y())
    } else if a.y() == b.y() {
        (surface.isocurve_u(a.y())?, a.x(), b.x())
    } else {
        return invalid("a rectangular deformation trim must be isoparametric");
    };
    let curve = curve.try_trimmed(start.min(end)..=start.max(end))?;
    if start > end {
        curve.reversed()
    } else {
        Ok(curve)
    }
}
// The coefficient certificate bounds the stored isocurves. Surface and curve
// evaluators take different floating-point paths, so retain one model tolerance
// of numerical budget instead of testing a saturated hull bound without slack.
fn with_model_precision(bound: Real, tolerance: Tolerance) -> Result<Real, GeometryError> {
    let allowed = (bound + tolerance.absolute()).next_up();
    require_finite([allowed], "deformed boundary tolerance")?;
    Ok(allowed)
}
fn distance_bound(a: Point3, b: Point3) -> Result<Real, GeometryError> {
    a.distance_bound_to(b, Real::MAX)?
        .ok_or(GeometryError::Degenerate {
            context: "deformed boundary distance exceeds the finite range",
        })
}
fn signed_hull(curve: &NurbsCurve) -> Result<(), GeometryError> {
    let sign = curve.control_points()[0].weight().is_sign_positive();
    if curve
        .control_points()
        .iter()
        .any(|p| p.weight().is_sign_positive() != sign)
    {
        return invalid("deformation bounds require uniformly signed boundary weights");
    }
    Ok(())
}
fn point_hull_bound(curve: &NurbsCurve, point: Point3) -> Result<Real, GeometryError> {
    signed_hull(curve)?;
    curve
        .control_points()
        .iter()
        .map(|p| distance_bound(p.point(), point))
        .try_fold(0_f64, |a, b| Ok(a.max(b?)))
}
fn correspondence_hull_bound(a: &NurbsCurve, b: &NurbsCurve) -> Result<Real, GeometryError> {
    signed_hull(a)?;
    signed_hull(b)?;
    if a.degree() != b.degree()
        || a.control_points().len() != b.control_points().len()
        || a.knots().len() != b.knots().len()
    {
        return invalid("deformation boundary correspondence changed basis");
    }
    let ad = a.domain();
    let bd = b.domain();
    let a0 = rational(*ad.start());
    let b0 = rational(*bd.start());
    let al = rational(*ad.end()) - &a0;
    let bl = rational(*bd.end()) - &b0;
    for (&x, &y) in a.knots().iter().zip(b.knots()) {
        if (rational(x) - &a0) * &bl != (rational(y) - &b0) * &al {
            return invalid("deformation boundary correspondence changed knots");
        }
    }
    let wa = rational(a.control_points()[0].weight());
    let wb = rational(b.control_points()[0].weight());
    let mut bound = 0_f64;
    for (x, y) in a.control_points().iter().zip(b.control_points()) {
        if rational(x.weight()) * &wb != rational(y.weight()) * &wa {
            return invalid("deformation boundary correspondence changed rational gauges");
        }
        bound = bound.max(distance_bound(x.point(), y.point())?);
    }
    Ok(bound)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn correspondence_bound_is_exact_under_signed_gauges_and_changed_domains() {
        let c = NurbsCurve::try_new_rational(
            2,
            [[0., 0., 0.], [1., 1., 0.], [2., 0., 0.]]
                .into_iter()
                .zip([1., 2., 1.])
                .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let d = NurbsCurve::try_new_rational(
            2,
            c.control_points()
                .iter()
                .map(|p| {
                    WeightedPoint3::try_new(
                        p.point()
                            .translated(Vector3::try_new(0., 3., 4.).unwrap())
                            .unwrap(),
                        p.weight() * -1e100,
                    )
                    .unwrap()
                })
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap()
        .try_reparameterized(-1e120..=3e120)
        .unwrap();
        assert_eq!(correspondence_hull_bound(&c, &d).unwrap(), 5.);
        let mixed = NurbsCurve::try_new_rational(
            2,
            c.control_points()
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    WeightedPoint3::try_new(
                        p.point(),
                        if i == 1 { -p.weight() } else { p.weight() },
                    )
                    .unwrap()
                })
                .collect(),
            c.knots().to_vec(),
        )
        .unwrap();
        assert!(correspondence_hull_bound(&c, &mixed).is_err());
    }
    #[test]
    fn seam_topology_retains_exact_surface_and_uv_with_measured_component_bounds() {
        let points = [
            [0., 0., 0.],
            [1., 3., 0.],
            [3., -1., 0.],
            [5., 0., 0.],
            [0., 0., 0.],
            [1., 3., 0.],
            [3., -1., 0.],
        ];
        let s = NurbsSurface::try_new(
            1,
            3,
            2,
            7,
            points
                .into_iter()
                .flat_map(|p| {
                    [
                        Point3::try_from(p).unwrap(),
                        Point3::try_new(p[0], p[1], 6.).unwrap(),
                    ]
                })
                .collect(),
            vec![0., 0., 6., 6.],
            (-3..=7).map(|x| x as Real).collect(),
        )
        .unwrap();
        let tol = Tolerance::DEFAULT;
        let source = Brep::try_surface_face(s.clone(), tol).unwrap();
        let before = source.clone();
        let target = s
            .prepare_soft_edit([0., 2.], [4., 4.], false, tol)
            .unwrap()
            .apply(Vector3::try_new(-1., 0.2, 2.2).unwrap())
            .unwrap();
        assert!(!target.is_periodic_v());
        let result = source
            .try_with_topology_preserving_surface_edit(target.clone(), tol)
            .unwrap();
        assert_eq!(&source, &before);
        assert_eq!(result.faces()[0].surface(), &target);
        assert_eq!(result.faces()[0].loops(), source.faces()[0].loops());
        assert_eq!(result.edges().len(), source.edges().len());
        assert_eq!(result.vertices().len(), source.vertices().len());
        assert!(
            result
                .edges()
                .iter()
                .any(|e| e.tolerance() > tol.absolute())
        );
        for trim in result.faces()[0].loops()[0].trims() {
            let Some(edge) = trim.edge() else { continue };
            let edge = &result.edges()[edge];
            for i in 0..=64 {
                let f = i as Real / 64.;
                let uv = trim
                    .curve()
                    .evaluate(trim.curve().parameter_at(f).unwrap())
                    .unwrap();
                let on_surface = target.evaluate(uv.x(), uv.y()).unwrap();
                let g = if trim.is_reversed_3d() { 1. - f } else { f };
                let p = edge
                    .curve()
                    .evaluate(edge.curve().parameter_at(g).unwrap())
                    .unwrap();
                assert!(
                    on_surface
                        .distance_bound_to(p, edge.tolerance().max(tol.absolute()))
                        .unwrap()
                        .is_some()
                );
            }
        }
    }
}
