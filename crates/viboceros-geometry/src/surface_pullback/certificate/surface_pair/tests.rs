use super::*;
use crate::WeightedPoint3;

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn sheet(degree: usize, height: impl Fn(usize, usize) -> Real) -> NurbsSurface {
    let mut points = Vec::new();
    for j in 0..=degree {
        for i in 0..=degree {
            points.push(point(
                i as Real / degree as Real,
                j as Real / degree as Real,
                height(i, j),
            ));
        }
    }
    let knots = [vec![0.; degree + 1], vec![1.; degree + 1]].concat();
    NurbsSurface::try_new(
        degree,
        degree,
        degree + 1,
        degree + 1,
        points,
        knots.clone(),
        knots,
    )
    .unwrap()
}

#[test]
fn rational_tensors_have_an_outward_bound_with_independent_weight_gauges() {
    let original = sheet(2, |i, j| (i * j) as Real / 8.);
    let weighted = |gauge, dz| {
        NurbsSurface::try_new_rational(
            2,
            2,
            3,
            3,
            original
                .control_points()
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let [x, y, z] = p.point().to_array();
                    WeightedPoint3::try_new(point(x, y, z + dz), gauge * [1., 0.5, 2.][i % 3])
                        .unwrap()
                })
                .collect(),
            original.knots_u().to_vec(),
            original.knots_v().to_vec(),
        )
        .unwrap()
    };
    let a = weighted(2., 0.);
    let same = weighted(-8., 0.)
        .try_reparameterized(2. ..=5., -7. ..=1.)
        .unwrap();
    assert_eq!(
        a.parameter_surface_deviation_bound(&same, 0.).unwrap(),
        Some(0.)
    );
    let shifted = weighted(-4., 0.125);
    assert_eq!(
        a.parameter_surface_deviation_bound(&shifted, 0.125)
            .unwrap(),
        Some(0.125)
    );
    assert_eq!(
        a.parameter_surface_deviation_bound(&shifted, 0.125_f64.next_down())
            .unwrap(),
        None
    );
}

#[test]
fn subdivision_accepts_a_surface_whose_initial_control_hull_is_too_wide() {
    let a = sheet(2, |_, _| 0.);
    let b = sheet(2, |i, j| if i == 1 && j == 1 { 1. } else { 0. });
    assert_eq!(b.evaluate(0.5, 0.5).unwrap().z(), 0.25);
    let bound = a
        .parameter_surface_deviation_bound(&b, 0.3)
        .unwrap()
        .unwrap();
    assert!((0.25..=0.3).contains(&bound));
    assert_eq!(
        a.parameter_surface_deviation_bound(&b, 0.249).unwrap(),
        None
    );
}

#[test]
fn a_local_interior_bump_cannot_hide_between_a_grid_and_four_equal_edges() {
    let sites = [0., 1. / 64., 1. / 32., 3. / 64., 1.];
    let knots = vec![0., 0., 1. / 64., 1. / 32., 3. / 64., 1., 1.];
    let surface = |height| {
        let mut controls = Vec::new();
        for (j, &y) in sites.iter().enumerate() {
            for (i, &x) in sites.iter().enumerate() {
                controls.push(point(x, y, if i == 2 && j == 2 { height } else { 0. }));
            }
        }
        NurbsSurface::try_new(1, 1, 5, 5, controls, knots.clone(), knots.clone()).unwrap()
    };
    let a = surface(0.);
    let b = surface(1. / 128.);
    for i in 0..=16 {
        for j in 0..=16 {
            assert_eq!(
                a.evaluate(i as Real / 16., j as Real / 16.).unwrap(),
                b.evaluate(i as Real / 16., j as Real / 16.).unwrap()
            );
        }
    }
    for t in [0., 1.] {
        assert_eq!(a.isocurve_u(t).unwrap(), b.isocurve_u(t).unwrap());
        assert_eq!(a.isocurve_v(t).unwrap(), b.isocurve_v(t).unwrap());
    }
    assert_eq!(a.parameter_surface_deviation_bound(&b, 1e-9).unwrap(), None);
    assert_eq!(
        a.parameter_surface_deviation_bound(&b, 1. / 128.).unwrap(),
        Some(1. / 128.)
    );
    // A rounded floating-point norm may underflow; the exact zero test cannot.
    assert_eq!(
        a.parameter_surface_deviation_bound(&surface(1e-300), 0.)
            .unwrap(),
        None
    );
}

#[test]
fn differing_knot_grids_degrees_and_unclamped_sources_keep_their_original_image() {
    let a = sheet(1, |i, j| (i + j) as Real / 4.);
    let b = a
        .try_change_degree(3, 2, false)
        .unwrap()
        .try_insert_knot_u(0.25, 1)
        .unwrap()
        .try_insert_knot_v(0.75, 1)
        .unwrap();
    assert!(
        a.parameter_surface_deviation_bound(&b, 1e-12)
            .unwrap()
            .unwrap()
            <= 1e-12
    );
    let unclamped = NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        (0..9)
            .map(|i| point((i % 3) as Real, (i / 3) as Real, (i % 4) as Real / 8.))
            .collect(),
        vec![-2., -1., 0., 1., 2., 3.],
        vec![-4., -2., 0., 2., 4., 6.],
    )
    .unwrap();
    let clamped = unclamped.try_clamped_to_active_domain().unwrap();
    assert!(
        unclamped
            .parameter_surface_deviation_bound(&clamped, 1e-12)
            .unwrap()
            .unwrap()
            <= 1e-12
    );
}

#[test]
fn both_limits_of_a_discontinuous_knot_are_compared() {
    let make = |left_z, right_z| {
        NurbsSurface::try_new(
            1,
            1,
            4,
            2,
            (0..2)
                .flat_map(|j| {
                    [0., 0.5, 0.5, 1.]
                        .into_iter()
                        .enumerate()
                        .map(move |(i, x)| {
                            point(x, j as Real, if i < 2 { left_z } else { right_z })
                        })
                })
                .collect(),
            vec![0., 0., 0.5, 0.5, 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    };
    let a = make(0., 1.);
    assert_eq!(
        a.parameter_surface_deviation_bound(&a, 0.).unwrap(),
        Some(0.)
    );
    for b in [make(0.25, 1.), make(0., 1.25)] {
        assert_eq!(
            a.parameter_surface_deviation_bound(&b, 0.125).unwrap(),
            None
        );
        assert_eq!(
            a.parameter_surface_deviation_bound(&b, 0.25).unwrap(),
            Some(0.25)
        );
    }
}

#[test]
fn correspondence_is_explicit_and_bad_limits_are_errors() {
    let a = sheet(1, |_, _| 0.);
    assert_eq!(
        a.parameter_surface_deviation_bound(&a.try_reversed_u().unwrap(), 0.1)
            .unwrap(),
        None
    );
    assert_eq!(
        a.parameter_surface_deviation_bound(&a.try_swapped_uv().unwrap(), 0.1)
            .unwrap(),
        None
    );
    for invalid in [-1., f64::NAN, f64::INFINITY] {
        assert!(matches!(
            a.parameter_surface_deviation_bound(&a, invalid),
            Err(GeometryError::InvalidTolerance)
        ));
    }
}

#[test]
fn unsupported_signs_degrees_and_exhausted_work_do_not_prove_a_bound() {
    let a = sheet(1, |_, _| 0.);
    let mut controls = a.control_points().to_vec();
    controls[0] = WeightedPoint3::try_new(controls[0].point(), -1.).unwrap();
    let mixed = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        controls,
        a.knots_u().to_vec(),
        a.knots_v().to_vec(),
    )
    .unwrap();
    assert_eq!(
        mixed.parameter_surface_deviation_bound(&mixed, 0.).unwrap(),
        None
    );
    let high = sheet(17, |_, _| 0.);
    assert_eq!(
        high.parameter_surface_deviation_bound(&high, 0.).unwrap(),
        None
    );
    assert!(matches!(
        bound(&a, &a, 0., &mut Budget(0)),
        Err(GeometryError::SurfaceCurveCertificateWorkLimit)
    ));
}
