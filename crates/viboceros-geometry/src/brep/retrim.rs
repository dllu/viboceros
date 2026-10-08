//! Transfer physical face boundaries to a changed surface parameterization.
use super::*;
use crate::PointMorph;

const MAX_PROJECTED_CONTROLS: usize = 4096;

struct Projection<'a> {
    surface: &'a NurbsSurface,
    tolerance: Tolerance,
}
impl PointMorph for Projection<'_> {
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        let (u, v) = self.surface.closest_parameters(point, self.tolerance)?;
        Point3::try_new(u, v, 0.)
    }
}

fn canonical_projection(
    mut uv: NurbsCurve2,
    parameter_limit: Real,
) -> Result<NurbsCurve2, GeometryError> {
    // Closest-point and interpolation roundoff can perturb a constant
    // coordinate. Same-sign weights bound the complete coordinate change by
    // the maximum control displacement; keep it inside the UV fitting budget.
    let a = uv.start_point()?.to_array();
    let b = uv.end_point()?.to_array();
    let sign = uv.control_points()[0].weight().is_sign_positive();
    if uv
        .control_points()
        .iter()
        .all(|p| p.weight().is_sign_positive() == sign)
    {
        for axis in 0..2 {
            if a[axis] != b[axis]
                || uv
                    .control_points()
                    .iter()
                    .any(|p| (p.point().to_array()[axis] - a[axis]).abs() > parameter_limit)
            {
                continue;
            }
            let controls = uv
                .control_points()
                .iter()
                .map(|p| {
                    let mut point = p.point().to_array();
                    point[axis] = a[axis];
                    WeightedPoint2::try_new(Point2::try_new(point[0], point[1])?, p.weight())
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            uv = NurbsCurve2::try_new_rational(uv.degree(), controls, uv.knots().to_vec())?;
        }
    }
    // This predicate proves the complete curve image is its ordered segment.
    // Retain that exact locus and orientation with linear parameter speed so
    // tensor-knot crossings can be composed directly instead of subdivided.
    if uv.is_straight_segment() {
        let domain = uv.domain();
        NurbsCurve2::try_new(
            1,
            vec![uv.start_point()?, uv.end_point()?],
            vec![
                *domain.start(),
                *domain.start(),
                *domain.end(),
                *domain.end(),
            ],
        )
    } else {
        Ok(uv)
    }
}

fn projection_curve(
    surface: &NurbsSurface,
    spatial: &NurbsCurve,
    ends: [Point2; 2],
    tolerance: Tolerance,
) -> Result<NurbsCurve2, GeometryError> {
    if let Ok(uv) = surface.try_pullback_exact_curve(spatial, tolerance)
        && let Some(uv) = crate::surface_pullback::constrain_curve_endpoints(uv, ends)?
        && surface
            .parameter_curve_deviation_bound(&uv, spatial, tolerance.absolute())?
            .is_some()
    {
        return canonical_projection(uv, 0.);
    }
    // In normalized UV, polynomial derivative control nets bound the amount
    // of model-space motion caused by an interpolation error in each axis.
    let mut speed = [0_f64; 2];
    let count = [
        surface.control_point_count_u(),
        surface.control_point_count_v(),
    ];
    for axis in 0..2 {
        let (degree, knots) = if axis == 0 {
            (surface.degree_u(), surface.knots_u())
        } else {
            (surface.degree_v(), surface.knots_v())
        };
        for j in 0..count[1] {
            for i in 0..count[0] {
                let index = [i, j][axis];
                if index + 1 == count[axis] {
                    continue;
                }
                let denominator = knots[index + degree + 1] - knots[index + 1];
                if denominator <= 0. {
                    continue;
                }
                let a = surface.control_points()[j * count[0] + i].point();
                let b = surface.control_points()
                    [j * count[0] + i + if axis == 0 { 1 } else { count[0] }]
                .point();
                speed[axis] = speed[axis].max(a.distance_to(b)? * degree as Real / denominator);
            }
        }
    }
    let limit = tolerance.absolute() / (4. * (speed[0] + speed[1]).max(1.));
    if !limit.is_finite() || limit <= 0. {
        return Err(GeometryError::SingularSystem);
    }
    let fitting = Tolerance::try_new(limit, tolerance.relative(), tolerance.angular())?;
    let numerical = Tolerance::try_new(
        (tolerance.absolute() * 1e-4).max(Real::MIN_POSITIVE),
        (tolerance.relative() * 1e-4).max(Real::MIN_POSITIVE),
        tolerance.angular(),
    )?;
    let fitted = crate::morph::fit_curve_with_control_limit(
        &Projection {
            surface,
            tolerance: numerical,
        },
        spatial,
        fitting,
        MAX_PROJECTED_CONTROLS,
    )?;
    let uv = NurbsCurve2::try_new_rational(
        fitted.degree(),
        fitted
            .control_points()
            .iter()
            .map(|p| {
                WeightedPoint2::try_new(Point2::try_new(p.point().x(), p.point().y())?, p.weight())
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        fitted.knots().to_vec(),
    )?;
    let uv = crate::surface_pullback::constrain_curve_endpoints(uv, ends)?.ok_or(
        GeometryError::InvalidBrepTopology {
            context: "projected trim endpoints cannot be constrained",
        },
    )?;
    canonical_projection(uv, limit)
}

impl Brep {
    /// Project a single face's spatial edges onto a changed polynomial surface,
    /// retaining loop order, shared vertices, face orientation and source UV domains.
    /// Exact pullbacks are certified when available. Otherwise the bounded
    /// closest-point projection fit has sampled accuracy checks; its resulting
    /// spatial edges have continuous certificates against the new UV trims.
    /// Projection is not a certified global nearest-point or topology guarantee.
    pub fn try_retrimmed_single_surface(
        &self,
        surface: NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let [face] = self.faces.as_slice() else {
            return invalid("retrimming requires one face");
        };
        if surface.is_rational() {
            return invalid("retrimming requires a polynomial target surface");
        }
        // Native natural faces retain their complete target boundary. Construct
        // exact isocurves directly, preserving the source's numeric topology,
        // instead of fitting an inverse parameter map along the same boundary.
        if face.loops.len() == 1
            && face.loops[0].trims.len() == 4
            && face.is_untrimmed(tolerance)?
        {
            let surface =
                surface.try_reparameterized(face.surface.domain_u(), face.surface.domain_v())?;
            let natural = Self::try_surface_face(surface.clone(), tolerance)?;
            let exact_sides = face.loops[0].trims.iter().all(|old| {
                old.curve.is_straight_segment()
                    && natural.faces[0].loops[0]
                        .trims
                        .iter()
                        .find(|t| t.iso == old.iso)
                        .is_some_and(|new| {
                            old.curve
                                .start_point()
                                .is_ok_and(|p| new.curve.start_point().is_ok_and(|q| p == q))
                                && old
                                    .curve
                                    .end_point()
                                    .is_ok_and(|p| new.curve.end_point().is_ok_and(|q| p == q))
                        })
            });
            let same_incidence = natural.vertices.len() == self.vertices.len()
                && natural.edges.len() == self.edges.len()
                && face.loops[0].trims.iter().all(|old| {
                    natural.faces[0].loops[0]
                        .trims
                        .iter()
                        .find(|t| t.iso == old.iso)
                        .is_some_and(|new| {
                            new.trim_type == old.trim_type
                                && new.edge.is_some() == old.edge.is_some()
                        })
                });
            if same_incidence && exact_sides {
                let mut vertices = self.vertices.clone();
                let mut edges = self.edges.clone();
                let mut loops = face.loops.clone();
                let mut used = std::collections::BTreeSet::new();
                for old in &mut loops[0].trims {
                    let Some(new) = natural.faces[0].loops[0]
                        .trims
                        .iter()
                        .find(|t| t.iso == old.iso)
                    else {
                        return invalid("natural retrim boundary class is not recognized");
                    };
                    if !used.insert(new.iso as usize) {
                        return invalid("natural retrim repeats a boundary side");
                    }
                    if let (Some(old_edge), Some(new_edge)) = (old.edge, new.edge) {
                        let mut edge = natural.edges[new_edge].clone();
                        if new.reversed_3d != old.reversed_3d {
                            edge.curve = edge.curve.reversed()?;
                        }
                        edge.vertices = self.edges[old_edge].vertices;
                        edges[old_edge] = edge;
                    }
                    for (a, b) in old.vertices.into_iter().zip(new.vertices) {
                        vertices[a] = natural.vertices[b];
                    }
                    let vertices = old.vertices;
                    let edge = old.edge;
                    let reversed = old.reversed_3d;
                    *old = new.clone();
                    old.vertices = vertices;
                    old.edge = edge;
                    old.reversed_3d = reversed;
                }
                return Self::try_new(
                    vertices,
                    edges,
                    vec![BrepFace::try_new(surface, face.reversed, loops)?],
                    tolerance,
                );
            }
        }
        if face
            .loops
            .iter()
            .flat_map(|l| &l.trims)
            .any(|t| t.trim_type != BrepTrimType::Boundary)
        {
            return invalid("retrimming seam and singular boundaries is not yet supported");
        }
        let normalized = surface.try_reparameterized(0. ..=1., 0. ..=1.)?;
        let numerical = Tolerance::try_new(
            (tolerance.absolute() * 1e-4).max(Real::MIN_POSITIVE),
            (tolerance.relative() * 1e-4).max(Real::MIN_POSITIVE),
            tolerance.angular(),
        )?;
        let parameters = self
            .vertices
            .iter()
            .map(|v| {
                let (u, v) = normalized.closest_parameters(v.point, numerical)?;
                Point2::try_new(u, v)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let mut vertices = self.vertices.clone();
        for (vertex, uv) in vertices.iter_mut().zip(&parameters) {
            vertex.point = normalized.evaluate(uv.x(), uv.y())?;
            vertex.tolerance = tolerance.absolute();
        }
        let mut edges = self.edges.clone();
        let mut loops = face.loops.clone();
        let domains = [face.surface.domain_u(), face.surface.domain_v()];
        let target = normalized.try_reparameterized(domains[0].clone(), domains[1].clone())?;
        for trim in loops.iter_mut().flat_map(|l| &mut l.trims) {
            let index = trim.edge.ok_or(GeometryError::InvalidBrepTopology {
                context: "retrimming needs spatial edges",
            })?;
            let mut source = self.edges[index].curve.clone();
            if trim.reversed_3d {
                source = source.reversed()?;
            }
            source = source.try_reparameterized(trim.curve.domain())?;
            let uv = projection_curve(
                &normalized,
                &source,
                trim.vertices.map(|i| parameters[i]),
                tolerance,
            )?;
            let controls = uv
                .control_points()
                .iter()
                .map(|p| {
                    let coordinates = [p.point().x(), p.point().y()];
                    WeightedPoint2::try_new(
                        Point2::try_new(
                            crate::remap_scalar(
                                coordinates[0],
                                [0., 1.],
                                [*domains[0].start(), *domains[0].end()],
                            )?,
                            crate::remap_scalar(
                                coordinates[1],
                                [0., 1.],
                                [*domains[1].start(), *domains[1].end()],
                            )?,
                        )?,
                        p.weight(),
                    )
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            trim.curve = NurbsCurve2::try_new_rational(uv.degree(), controls, uv.knots().to_vec())?;
            trim.iso = trim_iso::classify(&trim.curve, &target);
            trim.tolerance = [0.; 2];
            let mut spatial = target.try_pushup_curve_certified(&trim.curve, tolerance)?;
            if trim.reversed_3d {
                spatial = spatial.reversed()?;
            }
            edges[index].curve = spatial;
            edges[index].tolerance = tolerance.absolute();
        }
        Self::try_new(
            vertices,
            edges,
            vec![BrepFace::try_new(target, face.reversed, loops)?],
            tolerance,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_moves_boundaries_to_parallel_surface_and_preserves_holes_and_orientation() {
        let tolerance = Tolerance::DEFAULT;
        let center = Point3::try_new(0., 0., 0.).unwrap();
        let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
        let outer = crate::Circle3::try_new(center, 5., normal, tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let inner = crate::Circle3::try_new(center, 2., normal, tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let original = Brep::try_planar_face_with_holes(&outer, &[inner], tolerance)
            .unwrap()
            .reversed();
        let before = original.clone();
        let target = original.faces[0]
            .surface
            .transformed(AffineTransform3::from_translation(
                Vector3::try_new(0., 0., 1.).unwrap(),
            ))
            .unwrap()
            .try_reparameterized(2. ..=8., -4. ..=10.)
            .unwrap();
        let result = original
            .try_retrimmed_single_surface(target, tolerance)
            .unwrap();
        assert_eq!(original, before);
        assert!(result.faces[0].reversed);
        assert_eq!(result.faces[0].loops.len(), 2);
        assert_eq!(
            result.faces[0].surface.domain_u(),
            original.faces[0].surface.domain_u()
        );
        assert_eq!(
            result.faces[0].surface.domain_v(),
            original.faces[0].surface.domain_v()
        );
        assert!((result.area(tolerance).unwrap() - 21. * std::f64::consts::PI).abs() < 1e-7);
        for vertex in &result.vertices {
            assert!((vertex.point.z() - 1.).abs() < 1e-12);
        }
        for trim in result.faces[0].loops.iter().flat_map(|l| &l.trims) {
            let mut edge = result.edges[trim.edge.unwrap()].curve.clone();
            if trim.reversed_3d {
                edge = edge.reversed().unwrap();
            }
            assert!(
                result.faces[0]
                    .surface
                    .parameter_curve_deviation_bound(&trim.curve, &edge, tolerance.absolute())
                    .unwrap()
                    .is_some()
            );
        }
    }
    #[test]
    fn natural_retrim_preserves_reordered_edges_reversed_uses_and_exact_isocurves() {
        let tolerance = Tolerance::DEFAULT;
        let surface = NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(4., 6., 2.).unwrap(),
            Point3::try_new(0., 6., 0.).unwrap(),
        ])
        .unwrap();
        let mut original = Brep::try_surface_face(surface, tolerance)
            .unwrap()
            .reordered_edges(&[2, 0, 3, 1], tolerance)
            .unwrap()
            .reversed();
        let edge = original.faces[0].loops[0].trims[1].edge.unwrap();
        original.edges[edge].curve = original.edges[edge].curve.reversed().unwrap();
        original.edges[edge].vertices.swap(0, 1);
        original.faces[0].loops[0].trims[1].reversed_3d = true;
        original =
            Brep::try_new(original.vertices, original.edges, original.faces, tolerance).unwrap();
        let before = original.clone();
        let target =
            crate::try_rebuild_nurbs_surface(&original.faces[0].surface, [10, 10], [3, 3]).unwrap();
        let result = original
            .try_retrimmed_single_surface(target, tolerance)
            .unwrap();
        assert_eq!(original, before);
        assert!(result.faces[0].reversed);
        for (a, b) in original.faces[0].loops[0]
            .trims
            .iter()
            .zip(&result.faces[0].loops[0].trims)
        {
            assert_eq!(a.vertices, b.vertices);
            assert_eq!(a.edge, b.edge);
            assert_eq!(a.reversed_3d, b.reversed_3d);
            assert_eq!(a.iso, b.iso);
            let edge = &result.edges[b.edge.unwrap()];
            assert_eq!(edge.vertices, original.edges[a.edge.unwrap()].vertices);
            let spatial = if b.reversed_3d {
                edge.curve.reversed().unwrap()
            } else {
                edge.curve.clone()
            };
            assert!(
                result.faces[0]
                    .surface
                    .parameter_curve_deviation_bound(&b.curve, &spatial, 1e-9)
                    .unwrap()
                    .is_some()
            );
        }
    }
}
