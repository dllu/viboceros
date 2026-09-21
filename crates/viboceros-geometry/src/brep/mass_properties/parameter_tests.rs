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
