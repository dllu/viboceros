use super::*;
use crate::exact_scalar::rational;
use crate::{Point2, WeightedPoint2};

fn polygon(curve: &NurbsCurve2, remaining: &mut usize) -> Option<Vec<[Real; 2]>> {
    trim::polygon(curve, remaining).map(Iterator::collect)
}

fn curve(degree: usize, points: &[[Real; 2]], weights: &[Real], knots: Vec<Real>) -> NurbsCurve2 {
    assert_eq!(points.len(), weights.len());
    NurbsCurve2::try_new_rational(
        degree,
        points
            .iter()
            .zip(weights)
            .map(|(p, &w)| {
                WeightedPoint2::try_new(Point2::try_new(p[0], p[1]).unwrap(), w).unwrap()
            })
            .collect(),
        knots,
    )
    .unwrap()
}

#[test]
fn segment_certificate_is_exact_at_extreme_scales_and_under_common_weight_gauges() {
    for exponent in [-1000, -500, 0, 500, 1000] {
        let s = 2_f64.powi(exponent);
        let a = [-s, -s];
        let b = [s, s];
        let mid = [s * 0.5, s * 0.5];
        for gauge in [2_f64.powi(-900), -1., 2_f64.powi(900)] {
            let make = |p| {
                curve(
                    2,
                    &[a, p, b],
                    &[gauge, gauge * 2., gauge],
                    vec![0., 0., 0., 1., 1., 1.],
                )
            };
            for c in [make(mid), make(mid).reversed().unwrap()] {
                let mut remaining = 24;
                let actual = polygon(&c, &mut remaining).unwrap();
                assert_eq!(
                    actual,
                    vec![
                        c.control_points()[0].point().to_array(),
                        c.control_points()[2].point().to_array()
                    ]
                );
                assert_eq!(remaining, 0);
                assert_eq!(polygon(&c, &mut 23), None);
            }
            // One ULP is still a real bulge. Floating determinants can round
            // this difference away or overflow/underflow at these scales.
            assert_eq!(polygon(&make([mid[0], mid[1].next_up()]), &mut 100), None);
            assert_eq!(polygon(&make([s.next_up(), s.next_up()]), &mut 100), None);
        }
    }
}

#[test]
fn trim_certificate_requires_clamping_continuity_and_same_sign_weights() {
    let (a, b) = ([0., 0.], [1., 1.]);
    for degree in [1, 2, 3, 7] {
        for multiplicity in 0..=degree + 1 {
            let count = degree + 1 + multiplicity;
            let points = (0..count)
                .map(|i| if i == count - 1 { b } else { a })
                .collect::<Vec<_>>();
            let weights = vec![1.; count];
            for exponent in [-500, 0, 500] {
                let s = 2_f64.powi(exponent);
                let mut knots = vec![-s; degree + 1];
                knots.extend(std::iter::repeat_n(0., multiplicity));
                knots.extend(std::iter::repeat_n(s, degree + 1));
                let c = curve(degree, &points, &weights, knots.clone());
                assert_eq!(polygon(&c, &mut 1000).is_some(), multiplicity <= degree);
                knots[0] = -2. * s;
                let c = curve(degree, &points, &weights, knots);
                assert_eq!(polygon(&c, &mut 1000), None);
            }
        }
    }
    let knots = vec![0., 0., 0., 1., 1., 1.];
    for (points, weights) in [
        ([a, [0.5, 0.5], b], [1., -0.125, 1.]),
        ([a, [0.5, 0.5], a], [1., 1., 1.]),
        ([a, a, a], [1., 1., 1.]),
    ] {
        assert_eq!(
            polygon(&curve(2, &points, &weights, knots.clone()), &mut 100),
            None
        );
    }
    // Degree-one polygon corners must not be silently collapsed into a segment.
    let points = [a, [1., 0.], b];
    let c = curve(1, &points, &[1., 2., 3.], vec![0., 0., 0.5, 1., 1.]);
    assert_eq!(polygon(&c, &mut 100), Some(points.to_vec()));
}

#[test]
fn mixed_weight_quartic_is_pole_free_and_has_the_segment_image_but_is_unsupported() {
    let points = [0., 0.25, 0.5, 0.75, 1.].map(|s| [s, s]);
    let weights = [1., 1., -0.125, 1., 1.];
    let c = curve(
        4,
        &points,
        &weights,
        vec![0., 0., 0., 0., 0., 1., 1., 1., 1., 1.],
    );
    assert_eq!(polygon(&c, &mut 100), None);
    // Writing d=(27/4)t²(1-t)², its denominator is W=1-d>=37/64.
    // Its coordinate numerator N=t-d/2 obeys N>=t/2>=0, because
    // 4-27t(1-t)²=(1-3t)²(4-3t)>=0 on [0,1]. By symmetry
    // W-N >= (1-t)/2. Hence N/W stays in [0,1], is continuous, and
    // attains both endpoints: this is a genuine segment, not a pole or overshoot.
    // Check exact Bernstein evaluations against those factorizations; these
    // samples validate the test recipe, not the production certificate.
    for i in 0..=64 {
        let t = rational(i as Real / 64.);
        let one = rational(1.);
        let s = &one - &t;
        let basis = [
            s.pow(4),
            rational(4.) * &t * s.pow(3),
            rational(6.) * t.pow(2) * s.pow(2),
            rational(4.) * t.pow(3) * &s,
            t.pow(4),
        ];
        let w = basis
            .iter()
            .zip(weights)
            .map(|(b, w)| b * rational(w))
            .sum::<crate::exact_scalar::Rational>();
        let n = basis
            .iter()
            .zip(weights)
            .zip(points)
            .map(|((b, w), p)| b * rational(w) * rational(p[0]))
            .sum::<crate::exact_scalar::Rational>();
        let d = rational(6.75) * t.pow(2) * s.pow(2);
        assert_eq!(w, &one - &d);
        assert_eq!(n, &t - &d / rational(2.));
        assert_eq!(
            rational(4.) - rational(27.) * &t * s.pow(2),
            (&one - rational(3.) * &t).pow(2) * (rational(4.) - rational(3.) * &t)
        );
        assert!(w >= rational(37. / 64.));
        assert!(n >= &t / rational(2.));
        assert!(&w - &n >= s / rational(2.));
        let evaluated = c.evaluate(i as Real / 64.).unwrap();
        let expected = crate::exact_scalar::scalar(&(n / w)).unwrap();
        assert!((evaluated.x() - expected).abs() <= 2e-15);
        assert_eq!(evaluated.x(), evaluated.y());
    }
}

#[test]
fn higher_degree_segment_retraces_keep_the_oriented_image_and_rectangle() {
    let progress = [0., 1., 0., 0., 1.];
    let knots = vec![0., 0., 0., 0., 0., 1., 1., 1., 1., 1.];
    let retrace = curve(4, &progress.map(|s| [s, s]), &[1.; 5], knots.clone());
    assert!(retrace.evaluate(0.25).unwrap().x() > retrace.evaluate(0.5).unwrap().x());
    assert_eq!(polygon(&retrace, &mut 100), Some(vec![[0., 0.], [1., 1.]]));
    let mut brep = cube([0.; 3], 1.);
    for face in &mut brep.faces {
        let expected = rectangle::bounds(face, &mut 1000);
        for trim in &mut face.loops[0].trims {
            let cp = trim.curve.control_points();
            let a = cp[0].point().to_array();
            let b = cp[1].point().to_array();
            trim.curve = curve(
                4,
                &progress.map(|s| if s == 0. { a } else { b }),
                &[1.; 5],
                knots.clone(),
            );
        }
        assert_eq!(rectangle::bounds(face, &mut 1000), expected);
        assert_eq!(rectangle::bounds(face, &mut 159), None);
    }
    let brep = Brep::try_new(brep.vertices, brep.edges, brep.faces, Tolerance::DEFAULT).unwrap();
    assert_eq!(brep.solid_orientation().unwrap(), Outward);
    assert_eq!(brep.reversed().solid_orientation().unwrap(), Inward);
}

fn reencode(mut brep: Brep, degree: usize, multispan: bool, gauge: Real) -> Brep {
    let count = degree + 1 + if multispan { degree } else { 0 };
    let mut knots = vec![-3.; degree + 1];
    if multispan {
        knots.extend(std::iter::repeat_n(2., degree));
    }
    knots.extend(std::iter::repeat_n(7., degree + 1));
    for face in &mut brep.faces {
        for boundary in &mut face.loops {
            for trim in &mut boundary.trims {
                let controls = trim.curve.control_points();
                assert_eq!(controls.len(), 2);
                let (a, b) = (controls[0].point(), controls[1].point());
                // These tetrahedral UV endpoints are not a unit triangle:
                // interpolating even at dyadic fractions can round off-line.
                // Endpoint copies are exactly collinear. Alternating them also
                // exercises non-monotone control polygons; stationary spans
                // have separate constructor and correspondence regressions.
                let controls = (0..count)
                    .map(|i| {
                        WeightedPoint2::try_new(
                            if i % 2 == 0 && i != count - 1 { a } else { b },
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
    Brep::try_new(brep.vertices, brep.edges, brep.faces, Tolerance::DEFAULT).unwrap_or_else(
        |error| panic!("degree={degree}, multispan={multispan}, gauge={gauge}: {error}"),
    )
}

#[test]
fn higher_degree_rational_trim_images_classify_planar_and_curved_shells() {
    let mesh = TriangleMesh::try_new(
        [[0., 0., 0.], [1., 10., 0.], [1., 11., 1.], [2., 15., 0.]]
            .map(point)
            .to_vec(),
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let sources = [
        cube([0.; 3], 1.),
        Brep::try_from_mesh(&mesh, true, Tolerance::DEFAULT).unwrap(),
        Brep::try_surface_face(
            NurbsSurface::try_sphere(frame(), 2.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    ];
    for (kind, source) in sources.iter().enumerate() {
        for degree in [2, 3, 7] {
            for multispan in [false, true] {
                for gauge in [1., -1.] {
                    let brep = reencode(source.clone(), degree, multispan, gauge);
                    let before = brep.clone();
                    assert!(brep.is_solid());
                    assert_eq!(
                        brep.solid_orientation().unwrap(),
                        Outward,
                        "source={kind}, degree={degree}, multispan={multispan}, gauge={gauge}"
                    );
                    assert_eq!(brep.reversed().solid_orientation().unwrap(), Inward);
                    assert_eq!(brep.solid_orientation_with_budget(1).unwrap(), Unknown);
                    assert_eq!(brep, before);
                }
            }
        }
    }
}
