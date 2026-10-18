#![cfg(feature = "native")]
use viboceros_geometry::{
    AffineTransform3, Brep, Frame3, NurbsSurface, Point3, Tolerance, Vector3, WeightedPoint3,
};
use viboceros_smlib::{
    SurfaceFilletOptions as Options, SurfaceFilletSide as Side, fillet_surfaces,
};
fn tol() -> Tolerance {
    Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap()
}
fn surface(points: [[f64; 3]; 4]) -> NurbsSurface {
    NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        points.map(|p| Point3::try_from(p).unwrap()).to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
fn parents(length: [f64; 2]) -> [NurbsSurface; 2] {
    [
        surface([[0., 0., 0.], [10., 0., 0.], [0., 10., 0.], [10., 10., 0.]]),
        surface([
            [5., length[0], -5.],
            [5., length[0], 5.],
            [5., length[1], -5.],
            [5., length[1], 5.],
        ]),
    ]
}
fn output(p: &[NurbsSurface; 2], options: Options) -> Brep {
    fillet_surfaces(&p[0], &p[1], options, tol())
        .unwrap()
        .unwrap()
}
fn sign(s: Side) -> f64 {
    if s == Side::Positive { 1. } else { -1. }
}
fn check_cylinder(b: &Brep, options: Options, y: [f64; 2], area_length: f64) {
    assert!(!b.is_solid());
    assert_eq!(b.faces().len(), 1);
    assert!(b.edges().len() >= 4);
    let cx = 5. - options.radius * sign(options.sides[1]);
    let cz = options.radius * sign(options.sides[0]);
    let radial = |p: Point3| {
        assert!(
            ((p.x() - cx).hypot(p.z() - cz) - options.radius).abs() < 1e-8,
            "{p:?}"
        );
    };
    for face in b.faces() {
        let s = face.surface();
        for i in 0..=8 {
            for j in 0..=8 {
                let u = s.parameter_at_u(i as f64 / 8.).unwrap();
                let v = s.parameter_at_v(j as f64 / 8.).unwrap();
                if !face.contains_parameters(u, v, tol()).unwrap() {
                    continue;
                }
                let p = s.evaluate(u, v).unwrap();
                radial(p);
                if p.z().abs() < 1e-8 {
                    assert!((s.normal_at(u, v).unwrap().as_vector().z().abs() - 1.).abs() < 1e-8);
                }
                if (p.x() - 5.).abs() < 1e-8 {
                    assert!((s.normal_at(u, v).unwrap().as_vector().x().abs() - 1.).abs() < 1e-8);
                }
            }
        }
    }
    for e in b.edges() {
        let c = e.curve();
        for i in 0..=8 {
            let p = c.evaluate(c.parameter_at(i as f64 / 8.).unwrap()).unwrap();
            radial(p);
            assert!(p.y() >= y[0] - 1e-8 && p.y() <= y[1] + 1e-8);
        }
    }
    let min = b
        .vertices()
        .iter()
        .map(|v| v.point().y())
        .fold(f64::INFINITY, f64::min);
    let max = b
        .vertices()
        .iter()
        .map(|v| v.point().y())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (min - y[0]).abs() < 1e-8 && (max - y[1]).abs() < 1e-8,
        "y range {min}..{max}, expected {y:?}"
    );
    let expected = options.radius * std::f64::consts::FRAC_PI_2 * area_length;
    assert!((b.area(tol()).unwrap() - expected).abs() < 1e-7);
}
#[test]
fn four_normal_side_choices_create_tangent_circular_sheets_and_preserve_sources() {
    let p = parents([0., 10.]);
    let before = p.clone();
    for first in [Side::Positive, Side::Negative] {
        for second in [Side::Positive, Side::Negative] {
            for radius in [0.2, 2.] {
                let options = Options {
                    radius,
                    sides: [first, second],
                    extend: false,
                };
                check_cylinder(&output(&p, options), options, [0., 10.], 10.);
                assert_eq!(p, before);
            }
        }
    }
}
#[test]
fn unequal_parent_lengths_keep_minimal_or_maximal_fillet_extents() {
    for short in [[3., 7.], [4.99, 5.01]] {
        let p = parents(short);
        for extend in [false, true] {
            let options = Options {
                radius: 1.,
                extend,
                ..Options::default()
            };
            let width = short[1] - short[0];
            check_cylinder(
                &output(&p, options),
                options,
                if extend { [0., 10.] } else { short },
                if extend { (10. + width) / 2. } else { width },
            );
        }
    }
}

#[test]
fn translated_scaled_and_rotated_sources_reuse_the_shared_local_frame() {
    let source = parents([0., 10.]);
    for scale in [1e-40, 1., 1e40] {
        let transform = AffineTransform3::try_new(
            [[0., 0., scale], [scale, 0., 0.], [0., scale, 0.]],
            Vector3::try_new(1000. * scale, -2000. * scale, 3000. * scale).unwrap(),
        )
        .unwrap();
        let p = source.each_ref().map(|s| s.transformed(transform).unwrap());
        let before = p.clone();
        let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-8).unwrap();
        let result = fillet_surfaces(
            &p[0],
            &p[1],
            Options {
                radius: scale,
                extend: false,
                ..Options::default()
            },
            tolerance,
        )
        .unwrap_or_else(|e| panic!("scale {scale}: {e}"))
        .unwrap();
        for f in result.faces() {
            let s = f.surface();
            for i in 0..=4 {
                for j in 0..=4 {
                    let q = s
                        .evaluate(
                            s.parameter_at_u(i as f64 / 4.).unwrap(),
                            s.parameter_at_v(j as f64 / 4.).unwrap(),
                        )
                        .unwrap();
                    let x = q.y() / scale + 2000.;
                    let z = q.x() / scale - 1000.;
                    assert!(((x - 4.).hypot(z - 1.) - 1.).abs() < 1e-7);
                }
            }
        }
        assert_eq!(p, before);
    }
}
#[test]
fn unclamped_bases_distant_charts_and_common_signed_weights_preserve_the_same_fillet() {
    for gauge in [-1e280, 1e-280] {
        let a = NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            [0., 10.]
                .into_iter()
                .flat_map(|y| {
                    [-5., 5., 15.].into_iter().map(move |x| {
                        WeightedPoint3::try_new(Point3::try_new(x, y, 0.).unwrap(), gauge).unwrap()
                    })
                })
                .collect(),
            [-2., -1., 0., 1., 2., 3.]
                .map(|x| x * 2_f64.powi(-360))
                .to_vec(),
            vec![1e12, 1e12, 1e12 + 10., 1e12 + 10.],
        )
        .unwrap();
        let p = [a, parents([0., 10.])[1].clone()];
        let before = p.clone();
        let options = Options {
            radius: 1.,
            extend: false,
            ..Options::default()
        };
        check_cylinder(&output(&p, options), options, [0., 10.], 10.);
        assert_eq!(p, before);
    }
}
#[test]
fn invalid_or_disjoint_requests_leave_sources_and_later_calls_intact() {
    let p = parents([0., 10.]);
    let before = p.clone();
    for radius in [0., -1., f64::INFINITY, f64::NAN] {
        assert!(
            fillet_surfaces(
                &p[0],
                &p[1],
                Options {
                    radius,
                    ..Options::default()
                },
                tol()
            )
            .is_err()
        );
    }
    assert!(
        fillet_surfaces(
            &p[0],
            &p[1],
            Options {
                radius: 100.,
                ..Options::default()
            },
            tol()
        )
        .unwrap()
        .is_none()
    );
    let far = p[1]
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(100., 0., 0.).unwrap(),
        ))
        .unwrap();
    assert!(
        fillet_surfaces(&p[0], &far, Options::default(), tol())
            .unwrap()
            .is_none()
    );
    assert_eq!(p, before);
    for _ in 0..4 {
        let b = output(&p, Options::default());
        drop(b);
    }
}
#[test]
fn plane_and_cylinder_produce_the_analytic_torus_fillet() {
    let plane = surface([
        [-10., -10., 0.],
        [10., -10., 0.],
        [-10., 10., 0.],
        [10., 10., 0.],
    ]);
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        tol(),
    )
    .unwrap();
    let cylinder = NurbsSurface::try_cylinder(frame, 5., -5., 5.)
        .unwrap()
        .try_trimmed(0. ..=std::f64::consts::FRAC_PI_2, -5. ..=5.)
        .unwrap();
    let before = [plane.clone(), cylinder.clone()];
    let result = fillet_surfaces(
        &plane,
        &cylinder,
        Options {
            extend: false,
            ..Options::default()
        },
        tol(),
    )
    .unwrap()
    .unwrap();
    assert!(!result.faces().is_empty());
    for f in result.faces() {
        let s = f.surface();
        for i in 0..=12 {
            for j in 0..=8 {
                let u = s.parameter_at_u(i as f64 / 12.).unwrap();
                let v = s.parameter_at_v(j as f64 / 8.).unwrap();
                if !f.contains_parameters(u, v, tol()).unwrap() {
                    continue;
                }
                let p = s.evaluate(u, v).unwrap();
                let error = ((p.x().hypot(p.y()) - 6.).hypot(p.z() - 1.) - 1.).abs();
                assert!(error < 1e-8, "torus error {error}, point {p:?}");
            }
        }
    }
    assert_eq!([plane, cylinder], before);
}

#[test]
fn captured_fitted_sheet_mass_queries_survive_uv_swap() {
    // Frozen native output keeps the original integration counterexample even
    // when the fillet continuation policy and its generated surface improve.
    // This qualifies measurement of that sheet, not its agreement with Rhino's
    // extended fillet geometry.
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), include_bytes!("data/fitted-fillet-sheet.3dm")).unwrap();
    let model = viboceros_io::read_3dm_file(file.path(), tol()).unwrap();
    assert_eq!(model.objects.len(), 1);
    let viboceros_io::ThreeDmGeometry::Brep(brep) = &model.objects[0].geometry else {
        panic!("captured sheet was not imported as a B-rep")
    };
    assert_eq!(brep.faces().len(), 1);
    let surface = brep.faces()[0].surface();
    assert!(surface.knots_u().windows(2).filter(|w| w[0] < w[1]).count() > 200);
    assert_eq!(
        surface.knots_v().windows(2).filter(|w| w[0] < w[1]).count(),
        1
    );
    let before = brep.clone();
    let area = brep.area(tol()).unwrap();
    let mass = brep.area_mass_properties(tol()).unwrap();
    let center = mass.centroid().unwrap();
    // Independent reference: original integrator applied after transposing the
    // frozen face, at the same model tolerance, gave this area before the fix.
    assert!((area - 28.941496050285952).abs() < 1e-7);
    assert!((mass.area().unwrap() - area).abs() < 1e-7);
    for reversed in [false, true] {
        let mut swapped = if reversed {
            brep.reversed()
        } else {
            brep.clone()
        };
        swapped.try_swap_face_uv(&[0]).unwrap();
        assert!((swapped.area(tol()).unwrap() - area).abs() < 1e-7);
        let other = swapped.area_mass_properties(tol()).unwrap();
        assert!((other.area().unwrap() - area).abs() < 1e-7);
        assert!(other.centroid().unwrap().distance_to(center).unwrap() < 1e-8);
    }
    assert_eq!(brep, &before);
}
