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
        return Ok(uv);
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
    crate::surface_pullback::constrain_curve_endpoints(uv, ends)?.ok_or(
        GeometryError::InvalidBrepTopology {
            context: "projected trim endpoints cannot be constrained",
        },
    )
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
}
