use super::*;

fn reencode(mut source: Brep, degree: usize, leading: bool, gauge: Real) -> Brep {
    reencode_spans(&mut source, degree, leading, gauge, 1);
    source
}

fn reencode_spans(
    source: &mut Brep,
    degree: usize,
    leading: bool,
    gauge: Real,
    stationary_spans: usize,
) {
    let mut knots = vec![-3.; degree + 1];
    for i in 0..stationary_spans {
        knots.extend(vec![2. + 5. * i as Real; degree]);
    }
    knots.extend(vec![2. + 5. * stationary_spans as Real; degree + 1]);
    for face in &mut source.faces {
        for boundary in &mut face.loops {
            for trim in &mut boundary.trims {
                let cp = trim.curve.control_points();
                assert_eq!(cp.len(), 2);
                let (a, b) = (cp[0].point(), cp[1].point());
                let controls = (0..=(stationary_spans + 1) * degree)
                    .map(|i| {
                        WeightedPoint2::try_new(
                            if i < if leading {
                                degree * stationary_spans + 1
                            } else {
                                degree
                            } {
                                a
                            } else {
                                b
                            },
                            gauge * if i % 2 == 0 { 1. } else { 2. },
                        )
                        .unwrap()
                    })
                    .collect();
                trim.curve =
                    NurbsCurve2::try_new_rational(degree, controls, knots.clone()).unwrap();
            }
        }
    }
}

#[test]
fn stationary_spans_keep_split_correspondence_and_closed_tessellation() {
    let frame = Frame3::try_from_normal(
        p(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let original = Brep::try_box(frame, [[-1., 1.]; 3], Tolerance::DEFAULT).unwrap();
    for leading in [false, true] {
        let mut source = original.clone();
        reencode_spans(&mut source, 2, leading, 1., 4);
        let source = rebuild(source).unwrap();
        let before = source.clone();
        let parameters = [0.01, 0.99]
            .map(|t| source.edges[0].curve.parameter_at(t).unwrap())
            .to_vec();
        let split = source
            .try_split_edges_at_parameters(&[(0, parameters.clone())], Tolerance::DEFAULT)
            .unwrap();
        assert!(split.is_solid());
        assert_eq!(split.edges.len(), source.edges.len() + 2);
        for (&parameter, vertex) in parameters
            .iter()
            .rev()
            .zip(&split.vertices[source.vertices.len()..])
        {
            assert_eq!(
                vertex.point,
                source.edges[0].curve.evaluate(parameter).unwrap()
            );
        }
        for face in &split.faces {
            assert!(source.faces.iter().any(|f| f.surface == face.surface));
        }
        for brep in [&source, &split] {
            let mesh = brep.tessellate(4, Tolerance::DEFAULT).unwrap();
            assert!(mesh.topology().is_solid());
            for point in mesh.vertices() {
                assert!(
                    point
                        .to_array()
                        .iter()
                        .all(|c| (-1.000000001..=1.000000001).contains(c))
                );
            }
        }
        assert_eq!(source, before);
    }
}

#[test]
fn equivalent_stationary_trim_spans_validate_without_relaxing_tolerance() {
    let frame = Frame3::try_from_normal(
        p(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            p(0., 0., 0.),
            p(1., 10., 0.),
            p(1., 11., 1.),
            p(2., 15., 0.),
        ],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let sources = [
        Brep::try_box(frame, [[-1., 1.]; 3], Tolerance::DEFAULT).unwrap(),
        Brep::try_from_mesh(&mesh, true, Tolerance::DEFAULT).unwrap(),
        Brep::try_surface_face(
            NurbsSurface::try_sphere(frame, 2.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    ];
    for (kind, source) in sources.iter().enumerate() {
        for degree in [2, 3, 7] {
            for leading in [false, true] {
                for gauge in [1., -1.] {
                    let encoded = reencode(source.clone(), degree, leading, gauge);
                    // Clamping, continuity, same-sign weights and endpoint-copy
                    // controls prove the unchanged segment image independently
                    // of sampled correspondence or closest-point refinement.
                    assert_eq!(
                        encoded.solid_orientation().unwrap(),
                        BrepSolidOrientation::Outward
                    );
                    let rebuilt = rebuild(encoded.clone()).unwrap_or_else(|error| {
                        panic!("source={kind}, degree={degree}, leading={leading}, gauge={gauge}: {error}")
                    });
                    assert_eq!(rebuilt, encoded);
                    assert_eq!(rebuild(encoded.reversed()).unwrap(), encoded.reversed());
                }
            }
        }
    }
}

#[test]
fn stationary_spans_do_not_hide_a_trim_bulge_or_an_edge_excursion() {
    let mut bulge = reencode(square(), 2, false, 1.);
    let trim = &mut bulge.faces[0].loops[0].trims[0];
    let mut controls = trim.curve.control_points().to_vec();
    controls[1] = WeightedPoint2::try_new(Point2::try_new(0., 0.125).unwrap(), 2.).unwrap();
    trim.curve = NurbsCurve2::try_new_rational(2, controls, trim.curve.knots().to_vec()).unwrap();
    trim.iso = SurfaceIso::NotIso;
    assert!(rebuild(bulge).is_err());

    let mut excursion = reencode(square(), 2, true, -1.);
    excursion.edges[0].curve = NurbsCurve::try_clamped_uniform(
        1,
        vec![p(0., 0., 0.), p(1., 0., 0.), p(2., 0., 0.), p(1., 0., 0.)],
    )
    .unwrap();
    assert!(rebuild(excursion).is_err());
}

#[test]
fn stationary_sampling_removes_only_consecutive_exact_uv_duplicates() {
    let mut source = square();
    reencode_spans(&mut source, 2, false, 1., 4);
    let sampled = sample_trim_loop(&source.faces[0].loops[0], 4).unwrap();
    assert!(sampled.windows(2).all(|p| p[0] != p[1]));
    assert_ne!(sampled.first(), sampled.last());
    assert_eq!(sampled.len(), 16);

    let mut boundary = square().faces[0].loops[0].clone();
    let cp = boundary.trims[0].curve.control_points();
    let (a, b) = (cp[0].point(), cp[1].point());
    boundary.trims[0].curve =
        NurbsCurve2::try_new(1, vec![a, b, a, b], vec![0., 0., 1., 2., 3., 3.]).unwrap();
    let sampled = sample_trim_loop(&boundary, 1).unwrap();
    assert_eq!(&sampled[..4], &[a, b, a, b]);
    assert_eq!(sampled.len(), 6);

    let near = Point2::try_new(Real::from_bits(1), 0.).unwrap();
    boundary.trims[0].curve =
        NurbsCurve2::try_new(1, vec![a, near, b], vec![0., 0., 1., 2., 2.]).unwrap();
    let sampled = sample_trim_loop(&boundary, 1).unwrap();
    assert_eq!(&sampled[..3], &[a, near, b]);
}
