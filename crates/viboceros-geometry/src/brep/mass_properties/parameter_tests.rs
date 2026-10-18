use super::*;
use crate::NurbsCurve2;

fn with_trim_domains(source: &Brep, domain: [Real; 2]) -> Brep {
    let mut result = source.clone();
    for trim in result
        .faces
        .iter_mut()
        .flat_map(|f| &mut f.loops)
        .flat_map(|l| &mut l.trims)
    {
        let curve = &trim.curve;
        let old = curve.domain();
        let knots = curve
            .knots()
            .iter()
            .map(|t| {
                let f = (t - old.start()) / (old.end() - old.start());
                if f == 0. {
                    domain[0]
                } else if f == 1. {
                    domain[1]
                } else {
                    domain[0] * (1. - f) + domain[1] * f
                }
            })
            .collect();
        trim.curve =
            NurbsCurve2::try_new_rational(curve.degree(), curve.control_points().to_vec(), knots)
                .unwrap();
    }
    result.validate(Tolerance::DEFAULT).unwrap();
    result
}

#[test]
fn planar_trim_integral_does_not_quantize_large_native_parameter_origins() {
    let solid = tests::round_trim(tests::paraboloid(), &[0.5], true);
    let plane = solid.sub_brep(&[1], Tolerance::DEFAULT).unwrap();
    let tolerance = Tolerance::try_new(1e-12, 1e-13, 1e-10).unwrap();
    for origin in [0., 1e9, -1e9, 1e15, -1e15] {
        let source = with_trim_domains(&plane, [origin, origin + 4.]);
        let before = source.clone();
        let area = source.area(tolerance).unwrap();
        assert!(
            (area - std::f64::consts::PI * 0.25).abs() < 1e-12,
            "origin {origin}: {area}"
        );
        assert_eq!(source, before);
    }
}

#[test]
fn nonplanar_trim_integrals_and_surface_knot_crossings_ignore_native_parameter_origins() {
    let surface = tests::paraboloid()
        .try_insert_knot_u(-0.1, 2)
        .unwrap()
        .try_insert_knot_v(0.17, 2)
        .unwrap();
    let tolerance = Tolerance::try_new(1e-12, 1e-13, 1e-10).unwrap();
    let expected = std::f64::consts::PI / 6. * (2_f64.powf(1.5) - 1.);
    for origin in [0., 1e9, -1e9, 1e15, -1e15] {
        let source = with_trim_domains(
            &tests::round_trim(surface.clone(), &[0.5], true),
            [origin, origin + 4.],
        );
        let before = source.clone();
        let area = source.area(tolerance).unwrap();
        let volume = source.signed_volume(tolerance).unwrap();
        assert!(
            (area - expected - std::f64::consts::PI * 0.25).abs() < 1e-12,
            "origin {origin}: area {area}"
        );
        assert!(
            (volume - std::f64::consts::PI / 32.).abs() < 1e-12,
            "origin {origin}: volume {volume}"
        );
        assert_eq!(source, before);
    }
}

#[test]
fn rational_holes_and_orientation_survive_trim_parameter_scaling() {
    let tolerance = Tolerance::try_new(1e-12, 1e-13, 1e-10).unwrap();
    let disk_area = |r: Real| std::f64::consts::PI / 6. * ((1. + 4. * r * r).powf(1.5) - 1.);
    for domain in [
        [0., 4e-280],
        [0., 4e280],
        [-1e9, -1e9 + 4.],
        [1e15, 1e15 + 4.],
    ] {
        for reversed in [false, true] {
            let mut annulus = with_trim_domains(
                &tests::round_trim(tests::paraboloid(), &[0.5, 0.25], false),
                domain,
            );
            let mut solid = with_trim_domains(
                &tests::round_trim(tests::paraboloid(), &[0.5], true),
                domain,
            );
            if reversed {
                annulus = annulus.reversed();
                solid = solid.reversed();
            }
            assert!(
                (annulus.area(tolerance).unwrap() - disk_area(0.5) + disk_area(0.25)).abs() < 1e-12
            );
            let sign = if reversed { -1. } else { 1. };
            assert!(
                (solid.signed_volume(tolerance).unwrap() - sign * std::f64::consts::PI / 32.).abs()
                    < 1e-12
            );
        }
    }
}

fn densely_refined_paraboloid() -> NurbsSurface {
    let mut surface = tests::paraboloid();
    for i in 1..128 {
        surface = surface.try_insert_knot_u(-1. + i as Real / 64., 1).unwrap();
    }
    surface
}

#[test]
fn dense_knots_in_either_direction_preserve_trimmed_area_volume_and_centroids() {
    let source = tests::round_trim(densely_refined_paraboloid(), &[0.5], true);
    let tolerance = Tolerance::try_new(1e-10, 1e-12, 1e-10).unwrap();
    let pi = std::f64::consts::PI;
    let curved_area = pi / 6. * (2_f64.powf(1.5) - 1.);
    let cap_area = pi * 0.25;
    let curved_first_z = pi / 8. * ((2_f64.powf(2.5) - 1.) / 5. - (2_f64.powf(1.5) - 1.) / 3.);
    let centroid_z = (curved_first_z + cap_area * 0.25) / (curved_area + cap_area);
    for swapped in [false, true] {
        let mut body = source.clone();
        if swapped {
            body.try_swap_face_uv(&(0..body.faces.len()).collect::<Vec<_>>())
                .unwrap();
        }
        for reversed in [false, true] {
            let body = if reversed {
                body.reversed()
            } else {
                body.clone()
            };
            let before = body.clone();
            let area = body.area(tolerance).unwrap();
            assert!(
                (area - curved_area - cap_area).abs() < 1e-10,
                "{swapped} {reversed}: {area}"
            );
            let area_mass = body.area_mass_properties(tolerance).unwrap();
            assert!((area_mass.area().unwrap() - area).abs() < 1e-10);
            let center = area_mass.centroid().unwrap();
            assert!(center.x().abs() < 1e-10 && center.y().abs() < 1e-10);
            assert!((center.z() - centroid_z).abs() < 1e-10, "{center:?}");
            let sign = if reversed { -1. } else { 1. };
            assert!((body.signed_volume(tolerance).unwrap() - sign * pi / 32.).abs() < 1e-10);
            let volume_mass = body.volume_mass_properties(tolerance).unwrap();
            assert!((volume_mass.signed_volume().unwrap() - sign * pi / 32.).abs() < 1e-10);
            let center = volume_mass.centroid().unwrap();
            assert!(center.x().abs() < 1e-10 && center.y().abs() < 1e-10);
            assert!((center.z() - 1. / 6.).abs() < 1e-10, "{center:?}");
            assert_eq!(body, before);
        }
    }
}

#[test]
fn dense_knots_keep_curved_holes_subtractive_in_both_green_forms() {
    let tolerance = Tolerance::try_new(1e-10, 1e-12, 1e-10).unwrap();
    let source = tests::round_trim(densely_refined_paraboloid(), &[0.5, 0.25], false);
    let pi = std::f64::consts::PI;
    let q_outer = 2_f64;
    let q_inner = 1.25_f64;
    let area = pi / 6. * (q_outer.powf(1.5) - q_inner.powf(1.5));
    let first_z = pi / 8.
        * ((q_outer.powf(2.5) - q_inner.powf(2.5)) / 5.
            - (q_outer.powf(1.5) - q_inner.powf(1.5)) / 3.);
    for swapped in [false, true] {
        let mut annulus = source.clone();
        if swapped {
            annulus.try_swap_face_uv(&[0]).unwrap();
        }
        for reversed in [false, true] {
            let annulus = if reversed {
                annulus.reversed()
            } else {
                annulus.clone()
            };
            let before = annulus.clone();
            assert!((annulus.area(tolerance).unwrap() - area).abs() < 1e-10);
            let mass = annulus.area_mass_properties(tolerance).unwrap();
            assert!((mass.area().unwrap() - area).abs() < 1e-10);
            let center = mass.centroid().unwrap();
            assert!(center.x().abs() < 1e-10 && center.y().abs() < 1e-10);
            assert!((center.z() - first_z / area).abs() < 1e-10, "{center:?}");
            assert_eq!(annulus, before);
        }
    }
}
