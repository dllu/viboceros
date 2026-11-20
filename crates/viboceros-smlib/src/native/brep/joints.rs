//! Close adjacent UV trims only after proving the spatial trim image.
use super::*;

pub(super) fn close_numerical_joints(
    surface: &NurbsSurface,
    edges: &[BrepEdge],
    mut trims: Vec<BrepTrim>,
    tolerance: Tolerance,
) -> Result<Vec<BrepTrim>, Error> {
    let domains = [surface.domain_u(), surface.domain_v()];
    let numerical = domains.map(|d| (d.end() - d.start()).abs() * (256. * f64::EPSILON));
    for i in 0..trims.len() {
        let current = &trims[i];
        let next = &trims[(i + 1) % trims.len()];
        if current.vertices()[1] != next.vertices()[0] {
            continue;
        }
        let end = current.curve().end_point()?;
        let target = next.curve().start_point()?;
        if end == target {
            continue;
        }
        if (end.x() - target.x()).abs() > numerical[0]
            || (end.y() - target.y()).abs() > numerical[1]
        {
            continue;
        }
        let Some(edge) = current.edge().and_then(|e| edges.get(e)) else {
            continue;
        };
        if let Some(updated) =
            closed_endpoint(surface, edge, current, target, true, tolerance, true)?
        {
            trims[i] = updated;
        }
    }
    Ok(trims)
}

/// Stitch can place two adjacent trims at different UVs on the same native
/// vertex. Try both incident curves, keeping the original support and edge.
/// Leave periodic branches and unresolved gaps to the caller's validity check.
pub(super) fn close_stitched_joints(
    surface: &NurbsSurface,
    edges: &[BrepEdge],
    mut trims: Vec<BrepTrim>,
    tolerance: Tolerance,
) -> Result<Vec<BrepTrim>, Error> {
    let local = [surface.domain_u(), surface.domain_v()].map(|d| (d.end() - d.start()) / 128.);
    for i in 0..trims.len() {
        let j = (i + 1) % trims.len();
        let (a, b) = (&trims[i], &trims[j]);
        let (Some(a_edge), Some(b_edge)) = (a.edge(), b.edge()) else {
            continue;
        };
        if a.vertices()[1] != b.vertices()[0] {
            continue;
        }
        let (end, start) = (a.curve().end_point()?, b.curve().start_point()?);
        if end == start
            || (end.x() - start.x()).abs() > local[0]
            || (end.y() - start.y()).abs() > local[1]
        {
            continue;
        }
        if let Some(updated) =
            closed_endpoint(surface, &edges[a_edge], a, start, true, tolerance, false)?
        {
            trims[i] = updated;
        } else if let Some(updated) =
            closed_endpoint(surface, &edges[b_edge], b, end, false, tolerance, false)?
        {
            trims[j] = updated;
        }
    }
    Ok(trims)
}

fn closed_endpoint(
    surface: &NurbsSurface,
    edge: &BrepEdge,
    trim: &BrepTrim,
    target: Point2,
    end: bool,
    tolerance: Tolerance,
    require_certification: bool,
) -> Result<Option<BrepTrim>, Error> {
    let source = trim.curve();
    let degree = source.degree();
    let knots = source.knots();
    let end_knots = if end {
        &knots[knots.len() - degree - 1..]
    } else {
        &knots[..=degree]
    };
    if end_knots.iter().any(|k| *k != end_knots[0]) {
        return Ok(None);
    }
    let mut points = source.control_points().to_vec();
    let i = if end { points.len() - 1 } else { 0 };
    points[i] = WeightedPoint2::try_new(target, points[i].weight())?;
    let candidate = NurbsCurve2::try_new_rational(degree, points, knots.to_vec())?;
    let witness = if trim.is_reversed_3d() {
        candidate.reversed()?
    } else {
        candidate.clone()
    };
    let limit = tolerance.absolute().max(edge.tolerance());
    let bound = match surface.parameter_curve_deviation_bound(&witness, edge.curve(), limit) {
        Ok(bound) => bound,
        // When stitching, leave this trim alone and let the caller try
        // closing the joint on the other incident trim.
        Err(GeometryError::SurfaceCurveCertificateWorkLimit) if !require_certification => None,
        Err(error) => return Err(error.into()),
    };
    if bound.is_none() {
        if require_certification {
            return Err(Error::Kernel(
                "Numerical native UV joint cannot be continuously certified".into(),
            ));
        }
        return Ok(None);
    }
    let iso = classify_iso(surface, &candidate);
    Ok(Some(BrepTrim::try_new(
        trim.vertices(),
        trim.edge(),
        trim.is_reversed_3d(),
        candidate,
        trim.trim_type(),
        iso,
        trim.tolerance(),
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(scale: f64, gap: f64) -> (NurbsSurface, Vec<BrepEdge>, Vec<BrepTrim>) {
        let s = NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            [
                [0., 0., 0.],
                [scale, 0., 0.],
                [0., scale, 0.],
                [scale, scale, 0.],
            ]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let mut edges = Vec::new();
        let mut trims = Vec::new();
        for i in 0..2 {
            let points = if i == 0 {
                [[0., 0., 0.], [scale, 0., 0.]]
            } else {
                [[scale, 0., 0.], [0., 0., 0.]]
            };
            let uv = if i == 0 {
                [[0., 0.], [1., 0.]]
            } else {
                [[1. - gap, 0.], [0., 0.]]
            };
            let vertices = if i == 0 { [0, 1] } else { [1, 0] };
            let edge = NurbsCurve::try_new(
                1,
                points.map(|p| Point3::try_from(p).unwrap()).to_vec(),
                vec![0., 0., 1., 1.],
            )
            .unwrap();
            edges.push(BrepEdge::try_new(vertices, edge, 0.).unwrap());
            trims.push(
                BrepTrim::try_new(
                    vertices,
                    Some(i),
                    false,
                    NurbsCurve2::try_line(
                        Point2::try_from(uv[0]).unwrap(),
                        Point2::try_from(uv[1]).unwrap(),
                    )
                    .unwrap(),
                    BrepTrimType::Boundary,
                    SurfaceIso::NotIso,
                    [0.; 2],
                )
                .unwrap(),
            );
        }
        (s, edges, trims)
    }
    #[test]
    fn small_uv_joint_is_welded_with_a_continuous_spatial_certificate() {
        let (s, e, t) = fixture(1., 4. * f64::EPSILON);
        let output = close_numerical_joints(&s, &e, t, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            output[0].curve().end_point().unwrap(),
            output[1].curve().start_point().unwrap()
        );
        assert_eq!(output[0].tolerance(), [0.; 2]);
    }
    #[test]
    fn small_uv_gap_that_exceeds_the_physical_error_budget_is_rejected() {
        let (s, e, t) = fixture(1e9, 4. * f64::EPSILON);
        assert!(
            close_numerical_joints(&s, &e, t, Tolerance::try_new(1e-10, 1e-14, 1e-10).unwrap())
                .is_err()
        );
    }
    #[test]
    fn stitch_recovers_a_local_joint_beyond_coordinate_roundoff() {
        let (surface, edges, trims) = fixture(1., 5e-6);
        let tolerance = Tolerance::try_new(1e-3, 1e-12, 1e-6).unwrap();
        let output = close_stitched_joints(&surface, &edges, trims, tolerance).unwrap();
        assert_eq!(
            output[0].curve().end_point().unwrap(),
            output[1].curve().start_point().unwrap()
        );
        assert_eq!(output[0].edge(), Some(0));
        assert_eq!(output[1].edge(), Some(1));
    }
    #[test]
    fn separate_parameter_branches_are_not_joined() {
        let (s, e, t) = fixture(1., 0.5);
        assert_eq!(
            close_numerical_joints(&s, &e, t.clone(), Tolerance::DEFAULT).unwrap(),
            t
        );
    }
    #[test]
    fn a_welded_curve_gets_its_actual_isoparametric_classification() {
        let (surface, mut edges, mut trims) = fixture(1., 4. * f64::EPSILON);
        let gap = 4. * f64::EPSILON;
        for i in 0..2 {
            let vertices = if i == 0 { [0, 1] } else { [1, 0] };
            let spatial = if i == 0 {
                [[0.5, 0., 0.], [0.5, 1., 0.]]
            } else {
                [[0.5, 1., 0.], [0.5, 0., 0.]]
            };
            let uv = if i == 0 {
                [[0.5, 0.], [0.5, 1.]]
            } else {
                [[0.5 - gap, 1.], [0.5, 0.]]
            };
            edges[i] = BrepEdge::try_new(
                vertices,
                NurbsCurve::try_new(
                    1,
                    spatial.map(|p| Point3::try_from(p).unwrap()).to_vec(),
                    vec![0., 0., 1., 1.],
                )
                .unwrap(),
                0.,
            )
            .unwrap();
            trims[i] = BrepTrim::try_new(
                vertices,
                Some(i),
                false,
                NurbsCurve2::try_line(
                    Point2::try_from(uv[0]).unwrap(),
                    Point2::try_from(uv[1]).unwrap(),
                )
                .unwrap(),
                BrepTrimType::Boundary,
                if i == 0 {
                    SurfaceIso::InteriorUConstant
                } else {
                    SurfaceIso::NotIso
                },
                [0.; 2],
            )
            .unwrap();
        }
        let output = close_numerical_joints(&surface, &edges, trims, Tolerance::DEFAULT).unwrap();
        assert_eq!(output[0].iso(), SurfaceIso::NotIso);
        assert_eq!(
            output[0].curve().end_point().unwrap(),
            output[1].curve().start_point().unwrap()
        );
    }
}
