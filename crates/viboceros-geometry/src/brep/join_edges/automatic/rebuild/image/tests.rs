use super::*;

#[test]
fn surface_images_cover_both_values_at_discontinuous_trim_endpoints() {
    for right in [1., 10.] {
        let surface = NurbsSurface::try_new_rational(
            1,
            1,
            4,
            2,
            [0., 3.]
                .into_iter()
                .flat_map(|y| {
                    [0., 1., right, right + 1.].map(|x| {
                        WeightedPoint3::try_new(Point3::try_new(x, y, 0.).unwrap(), 1.).unwrap()
                    })
                })
                .collect(),
            vec![0., 0., 0.5, 0.5, 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        for (interval, points) in [
            ([0.25, 0.5], [0.5, 1.]),
            ([0.5, 0.75], [right, right + 0.5]),
        ] {
            for reverse in [false, true] {
                let mut interval = interval;
                let mut points = points;
                if reverse {
                    interval.reverse();
                    points.reverse();
                }
                let trim = BrepTrim::try_new(
                    [0, 1],
                    Some(0),
                    false,
                    NurbsCurve2::try_line(
                        Point2::try_new(interval[0], 0.).unwrap(),
                        Point2::try_new(interval[1], 0.).unwrap(),
                    )
                    .unwrap(),
                    BrepTrimType::Boundary,
                    SurfaceIso::South,
                    [0.; 2],
                )
                .unwrap();
                let mut budget = Budget(MAX_WORK);
                let image = BoundaryImage::new(&surface, &trim, &mut budget)
                    .unwrap()
                    .unwrap();
                let edge = NurbsCurve::try_new(
                    1,
                    points.map(|x| Point3::try_new(x, 0., 0.).unwrap()).to_vec(),
                    vec![0., 0., 1., 1.],
                )
                .unwrap();
                for reversed_3d in [false, true] {
                    let edge = if reversed_3d {
                        edge.reversed().unwrap()
                    } else {
                        edge.clone()
                    };
                    assert_eq!(
                        image.bound(&edge, reversed_3d, true, &mut budget).unwrap(),
                        Some(right - 1.)
                    );
                }
                for end in [false, true] {
                    let i = usize::from(end);
                    let point = Point3::try_new(points[i], 0., 0.).unwrap();
                    assert_eq!(
                        image.endpoint_bound(point, end, &mut budget).unwrap(),
                        Some(if interval[i] == 0.5 { right - 1. } else { 0. })
                    );
                }
            }
        }
    }
}

fn surface(origin: Real, transpose: bool, unclamped: bool) -> NurbsSurface {
    let profile = [[0., 0., 0.], [15., 15., 0.], [30., 0., 0.]];
    let mut points = [0., 3.]
        .into_iter()
        .flat_map(|z| {
            profile.into_iter().zip([1., 2., 1.]).map(move |(p, w)| {
                WeightedPoint3::try_new(Point3::try_new(p[0], p[1], z).unwrap(), w).unwrap()
            })
        })
        .collect::<Vec<_>>();
    let knots = if unclamped {
        [-2., -1., 0., 1., 2., 3.]
    } else {
        [0., 0., 0., 1., 1., 1.]
    }
    .map(|t| origin + t)
    .to_vec();
    if transpose {
        points = (0..3)
            .flat_map(|u| (0..2).map(move |v| v * 3 + u))
            .map(|i| points[i])
            .collect();
        NurbsSurface::try_new_rational(1, 2, 2, 3, points, vec![0., 0., 1., 1.], knots).unwrap()
    } else {
        NurbsSurface::try_new_rational(2, 1, 3, 2, points, knots, vec![0., 0., 1., 1.]).unwrap()
    }
}

#[test]
fn partial_natural_images_cover_both_axes_unclamped_rows_and_large_uv_origins() {
    for origin in [0., 1e12] {
        for transpose in [false, true] {
            for unclamped in [false, true] {
                let surface = surface(origin, transpose, unclamped);
                let source = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
                let cuts = if origin == 0. {
                    [0.1, 0.8]
                } else {
                    [0.125, 0.875]
                };
                let cuts = cuts
                    .into_iter()
                    .map(|t| if transpose { -t } else { t })
                    .collect::<Vec<_>>();
                let source = source
                    .try_split_edges_at_parameters(
                        &[(if transpose { 3 } else { 0 }, cuts)],
                        Tolerance::DEFAULT,
                    )
                    .unwrap();
                let before = source.clone();
                let mut budget = Budget(MAX_WORK);
                for usage in source.trim_uses() {
                    let trim = usage.trim;
                    let image =
                        BoundaryImage::new(&source.faces[usage.face].surface, trim, &mut budget)
                            .unwrap();
                    let (start, end) = (
                        trim.curve.start_point().unwrap(),
                        trim.curve.end_point().unwrap(),
                    );
                    let along_profile = if transpose {
                        start.x() == end.x()
                    } else {
                        start.y() == end.y()
                    };
                    if unclamped && !along_profile {
                        // The other two sides fix the *unclamped* direction:
                        // they are not exact surface rows and remain unsupported.
                        assert!(image.is_none());
                        continue;
                    }
                    let image = image.unwrap();
                    let edge = &source.edges[trim.edge.unwrap()];
                    assert!(
                        image
                            .bound(&edge.curve, trim.reversed_3d, true, &mut budget)
                            .unwrap()
                            .unwrap()
                            < 1e-12
                    );
                    for end in [false, true] {
                        let point = source.vertices[trim.vertices[usize::from(end)]].point;
                        assert!(
                            image
                                .endpoint_bound(point, end, &mut budget)
                                .unwrap()
                                .unwrap()
                                < 1e-12
                        );
                    }
                }
                assert_eq!(source, before);
            }
        }
    }
}

#[test]
fn unsupported_interior_and_nonisoparametric_trims_do_not_get_a_false_row_certificate() {
    let source = Brep::try_rectangular_surface_face(
        surface(0., false, false),
        0.125..=0.875,
        0.25..=0.75,
        Tolerance::DEFAULT,
    )
    .unwrap();
    for usage in source.trim_uses() {
        assert!(
            BoundaryImage::new(&source.faces[0].surface, usage.trim, &mut Budget(MAX_WORK))
                .unwrap()
                .is_none()
        );
    }
    let mut trim = source.faces[0].loops[0].trims[0].clone();
    trim.curve = NurbsCurve2::try_line(
        Point2::try_new(0., 0.).unwrap(),
        Point2::try_new(1., 1.).unwrap(),
    )
    .unwrap();
    trim.iso = SurfaceIso::South;
    assert!(
        BoundaryImage::new(&source.faces[0].surface, &trim, &mut Budget(MAX_WORK))
            .unwrap()
            .is_none()
    );
    assert!(BoundaryImage::new(&source.faces[0].surface, &trim, &mut Budget(0)).is_err());
}

#[test]
fn partial_image_certificates_do_not_erase_source_uncertainty() {
    let mut a = super::super::tests::sheet([0., 3.], 0.);
    let b = super::super::tests::sheet([3.0005, 6.], 0.);
    for e in &mut a.edges {
        e.tolerance = 0.1;
    }
    for v in &mut a.vertices {
        v.tolerance = 0.2;
    }
    let a = a
        .try_split_edges_at_parameters(&[(2, vec![-0.875, -0.125])], Tolerance::DEFAULT)
        .unwrap();
    let b = b
        .try_split_edges_at_parameters(&[(0, vec![0.125, 0.875])], Tolerance::DEFAULT)
        .unwrap();
    let originals = [a.clone(), b.clone()];
    let tolerance = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    let result = join_breps(&[&a, &b], 0.002, tolerance).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].joined_edge_count, 3);
    let joined = &result[0].brep;
    for (edge, uses) in joined.edges.iter().zip(joined.edge_use_counts()) {
        if uses == 2 {
            assert_eq!(edge.tolerance, 0.1);
        }
    }
    assert!(
        joined
            .vertices
            .iter()
            .filter(|v| v.tolerance >= 0.2)
            .count()
            >= 4
    );
    assert_eq!([a, b], originals);
}
