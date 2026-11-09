use super::*;
use crate::{Point3, WeightedPoint3};

fn strip(curved: bool) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        3,
        1,
        4,
        2,
        (0..2)
            .flat_map(|j| {
                (0..4).map(move |i| {
                    WeightedPoint3::try_new(
                        Point3::try_new(
                            i as Real / 3.,
                            j as Real,
                            if curved { (i * i) as Real / 9. } else { 0. },
                        )
                        .unwrap(),
                        [1., 0.5, 2., 1.][i],
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn automatic_curved_image_fits_retain_recheckable_maps_and_use_fewer_controls() {
    let base = strip(true)
        .try_reparameterized(2. ..=5., -7. ..=-1.)
        .unwrap();
    for swap in [false, true] {
        let source = if swap {
            base.try_swapped_uv().unwrap()
        } else {
            base.clone()
        };
        let before = source.clone();
        let options = SurfaceFitOptions {
            degree: if swap { [1, 3] } else { [3, 1] },
            maximum_control_count: if swap { [2, 16] } else { [16, 2] },
        };
        let mut reduced = false;
        for limit in [0.01, 0.002] {
            let uv = try_fit_surface_with_bound(&source, options, limit)
                .unwrap()
                .unwrap();
            let fit = try_fit_surface_image_with_bound(&source, options, limit)
                .unwrap()
                .unwrap();
            assert!(fit.maximum_deviation_bound <= limit);
            assert_eq!(fit.surface.domain_u(), source.domain_u());
            assert_eq!(fit.surface.domain_v(), source.domain_v());
            assert_eq!(
                fit.certificate
                    .verify(&source, &fit.surface, limit)
                    .unwrap(),
                Some(fit.maximum_deviation_bound)
            );
            if fit.surface.control_points().len() < uv.surface.control_points().len() {
                assert!(matches!(
                    fit.certificate,
                    SurfaceImageFitCertificate::SeparableUv(_)
                ));
                reduced = true;
            }
        }
        assert!(reduced);
        assert_eq!(source, before);
    }
}

#[test]
fn a_spike_between_every_map_preflight_station_cannot_authorize_an_image_fit() {
    let mut source = strip(false);
    for t in [1. / 512., 2. / 512., 3. / 512., 4. / 512., 5. / 512.] {
        source = source.try_insert_knot_u(t, 3).unwrap();
    }
    let nu = source.control_point_count_u();
    let index = (0..nu)
        .find(|&i| source.knots_u()[i] > 0. && source.knots_u()[i + 4] < 1. / 32.)
        .unwrap();
    let mut points = source.control_points().to_vec();
    for j in 0..2 {
        let p = points[j * nu + index];
        points[j * nu + index] = WeightedPoint3::try_new(
            Point3::try_new(p.point().x(), p.point().y(), 10.).unwrap(),
            p.weight(),
        )
        .unwrap();
    }
    let source = NurbsSurface::try_new_rational(
        3,
        1,
        nu,
        2,
        points,
        source.knots_u().to_vec(),
        source.knots_v().to_vec(),
    )
    .unwrap();
    let before = source.clone();
    let target = try_interpolate_surface_in_basis(
        &source,
        [3, 1],
        [&[0., 0., 0., 0., 1., 1., 1., 1.], &[0., 0., 1., 1.]],
    )
    .unwrap();
    assert!(target.control_points().iter().all(|p| p.point().z() == 0.));
    let a = sections(&source).unwrap();
    let b = sections(&target).unwrap();
    let mut cache = [[None; MAX_DIVISIONS + 1]; 2];
    for n in [4, 8, 16] {
        let u = axis_map(
            &a[0],
            &b[0].prepare_closest_parameter_search().unwrap(),
            n,
            Tolerance::DEFAULT,
            &mut cache[0],
        )
        .unwrap();
        let v = axis_map(
            &a[1],
            &b[1].prepare_closest_parameter_search().unwrap(),
            n,
            Tolerance::DEFAULT,
            &mut cache[1],
        )
        .unwrap();
        let basic = [preflight::midpoints(&u), preflight::midpoints(&v)];
        assert!(preflight::within_stations(&source, &target, &basic, 0.05));
        assert!(!preflight::within(
            &source,
            &target,
            &[u.clone(), v.clone()],
            0.05
        ));
        assert!(
            source
                .mapped_surface_deviation_bound(&target, [&u, &v], 0.05)
                .unwrap()
                .is_none()
        );
    }
    let options = SurfaceFitOptions {
        degree: [3, 1],
        maximum_control_count: [4, 2],
    };
    assert!(
        try_fit_surface_image_with_bound(&source, options, 0.05)
            .unwrap()
            .is_none()
    );
    assert_eq!(source, before);
}

#[test]
fn a_rational_peak_missed_even_by_knot_enriched_grids_requires_the_complete_proof() {
    let r = 2_f64.powi(-20);
    let e = r * r;
    let weights = [e, e, (1. + 2. * e) / 3., 1.];
    let x = [0., 1. / 3., 2. * e / (1. + 2. * e), 1.];
    let z = [
        0.,
        (2. * r / 3.) / weights[1],
        (2. * r / 3.) / weights[2],
        0.,
    ];
    let source = NurbsSurface::try_new_rational(
        3,
        1,
        4,
        2,
        (0..2)
            .flat_map(|j| {
                (0..4).map(move |i| {
                    WeightedPoint3::try_new(
                        Point3::try_new(x[i], j as Real, z[i]).unwrap(),
                        weights[i],
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(source.evaluate(r, 0.5).unwrap().z() > 0.99);
    let target =
        try_interpolate_surface_in_basis(&source, [3, 1], [source.knots_u(), source.knots_v()])
            .unwrap();
    assert!(
        target
            .control_points()
            .iter()
            .all(|p| p.point().z().abs() < 1e-4)
    );
    let maps: Maps =
        std::array::from_fn(|_| vec![[0., 0.], [0.25, 0.25], [0.5, 0.5], [0.75, 0.75], [1., 1.]]);
    assert!(preflight::within(&source, &target, &maps, 0.05));
    assert!(
        source
            .mapped_surface_deviation_bound(&target, maps.each_ref().map(Vec::as_slice), 0.05)
            .unwrap()
            .is_none()
    );
    assert!(
        try_fit_surface_image_with_bound(
            &source,
            SurfaceFitOptions {
                degree: [3, 1],
                maximum_control_count: [4, 2]
            },
            0.05
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn invalid_and_singular_clock_proposals_decline_without_a_partial_certificate() {
    let a = strip(true);
    let b = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![Point3::try_new(0., 0., 0.).unwrap(); 4],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(certify(&a, &b, 0.01).unwrap().is_none());
    assert!(certify(&a, &a, 0.).unwrap().is_none());
    let invalid = SurfaceImageFitCertificate::SeparableUv([vec![], vec![[0., 0.], [1., 1.]]]);
    assert_eq!(
        invalid.verify(&a, &a, 0.1),
        Err(GeometryError::InvalidSurfaceCorrespondence)
    );
}
