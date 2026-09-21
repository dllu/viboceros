use super::*;
use crate::{CurveSegment3, WeightedPoint3};

#[test]
fn linear_polycurve_extraction_maps_every_vertex_into_parent_parameters() {
    let a = LineSegment::try_new(p(0., 0.), p(2., 0.), Tolerance::DEFAULT)
        .unwrap()
        .try_reparameterized(-7.0..=-3.0)
        .unwrap();
    let b = NurbsCurve::try_new_rational(
        1,
        [(p(2., 0.), 1.), (p(2., 3.), 4.), (p(5., 3.), 2.)]
            .into_iter()
            .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .collect(),
        vec![10., 10., 11., 14., 14.],
    )
    .unwrap();
    let source = Curve3::PolyCurve(
        PolyCurve3::try_with_segment_domains(
            vec![CurveSegment3::Line(a), CurveSegment3::NurbsCurve(b)],
            vec![100., 102., 110.],
        )
        .unwrap(),
    );
    let before = source.clone();
    assert!(is_linear(&source));
    let result = linear_form(&source, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        result.vertices(),
        &[p(0., 0.), p(2., 0.), p(2., 3.), p(5., 3.)]
    );
    assert_eq!(result.parameters(), &[100., 102., 104., 110.]);
    assert_eq!(source, before);
}

#[test]
fn polynomial_bezier_lines_are_not_join_polyline_encodings() {
    for degree in [2, 3, 5] {
        let curve = Curve3::NurbsCurve(
            NurbsCurve::try_clamped_uniform(
                degree,
                (0..=degree)
                    .map(|i| p(i as f64 / degree as f64, 0.))
                    .collect(),
            )
            .unwrap(),
        );
        assert!(!is_linear(&curve));
        for style in [CurveJoinStyle::Batch, CurveJoinStyle::Seeded] {
            let joined = join_curves(
                &[curve.clone(), line([1., 0.], [1., 2.])],
                CurveJoinOptions {
                    tolerance: 0.,
                    preserve_direction: false,
                    style,
                },
                Tolerance::DEFAULT,
            )
            .unwrap();
            let Curve3::PolyCurve(result) = joined[0].curve() else {
                panic!("degree must be retained")
            };
            let CurveSegment3::NurbsCurve(retained) = &result.segments()[0] else {
                panic!("NURBS leaf")
            };
            assert_eq!(retained.degree(), degree);
        }
    }
}

#[test]
fn mixed_linear_run_rebuilds_raw_domains_independent_of_pick_order() {
    let arc = Curve3::Arc(
        CircularArc3::try_from_three_points(p(0., 0.), p(1., 1.), p(2., 0.), Tolerance::DEFAULT)
            .unwrap(),
    );
    let chain = [arc, line([2., 0.], [2., 3.]), line([2., 3.], [4., 3.])];
    for indices in [[0, 1, 2], [2, 1, 0]] {
        let inputs = indices.map(|i| chain[i].clone());
        let before = inputs.clone();
        let joined = join_curves(
            &inputs,
            CurveJoinOptions {
                tolerance: 0.01,
                preserve_direction: false,
                style: CurveJoinStyle::Seeded,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        let Curve3::PolyCurve(result) = joined[0].curve() else {
            panic!("mixed curve")
        };
        assert_eq!(result.segments().len(), 2);
        assert_eq!(result.domain(), 0.0..=std::f64::consts::PI + 5.);
        assert_eq!(
            result.segments()[0].as_ref().domain(),
            0.0..=std::f64::consts::PI
        );
        assert_eq!(result.segments()[1].as_ref().domain(), 0.0..=5.);
        assert_eq!(
            result.evaluate(std::f64::consts::PI + 3.).unwrap(),
            p(2., 3.)
        );
        assert_eq!(inputs, before);
    }
}

#[test]
fn closing_a_polycurve_uses_its_actual_terminal_leaves() {
    let arc = Curve3::Arc(
        CircularArc3::try_from_three_points(p(0., 0.), p(1., 1.), p(2., 0.), Tolerance::DEFAULT)
            .unwrap(),
    );
    let first = Curve3::PolyCurve(
        PolyCurve3::concatenate(&[
            arc.to_polycurve().unwrap(),
            line([2., 0.], [2., 3.]).to_polycurve().unwrap(),
        ])
        .unwrap(),
    );
    let joined = join_curves(
        &[first, line([2., 3.], [0., 0.])],
        CurveJoinOptions {
            tolerance: 0.01,
            preserve_direction: false,
            style: CurveJoinStyle::Seeded,
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    let Curve3::PolyCurve(result) = joined[0].curve() else {
        panic!("mixed closed curve")
    };
    assert!(result.is_closed().unwrap());
    assert_eq!(result.segments().len(), 2);
    assert!(matches!(result.segments()[0], CurveSegment3::Arc(_)));
    assert!(matches!(result.segments()[1], CurveSegment3::Polyline(_)));
    assert_eq!(result.evaluate(0.).unwrap(), p(0., 0.));
}

#[test]
fn nonlinear_closer_appends_to_an_accumulated_polyline_but_prepends_to_one_line() {
    let closer = Curve3::Arc(
        CircularArc3::try_from_three_points(p(2., 3.), p(-1., 1.), p(0., 0.), Tolerance::DEFAULT)
            .unwrap(),
    );
    let inputs = [line([0., 0.], [2., 0.]), line([2., 0.], [2., 3.]), closer];
    let result = join_curves(
        &inputs,
        CurveJoinOptions {
            tolerance: 0.01,
            preserve_direction: false,
            style: CurveJoinStyle::Seeded,
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    let Curve3::PolyCurve(curve) = result[0].curve() else {
        panic!("closed mixed curve")
    };
    assert!(curve.is_closed().unwrap());
    assert!(matches!(curve.segments()[0], CurveSegment3::Polyline(_)));
    assert!(matches!(curve.segments()[1], CurveSegment3::Arc(_)));
    assert!(
        curve
            .evaluate(*curve.domain().start())
            .unwrap()
            .distance_to(p(0., 0.))
            .unwrap()
            < 1e-12
    );

    let closer = Curve3::Arc(
        CircularArc3::try_from_three_points(p(2., 0.), p(1., -1.), p(0., 0.), Tolerance::DEFAULT)
            .unwrap(),
    );
    let result = join_curves(
        &[line([0., 0.], [2., 0.]), closer],
        CurveJoinOptions {
            tolerance: 0.01,
            preserve_direction: false,
            style: CurveJoinStyle::Seeded,
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    let Curve3::PolyCurve(curve) = result[0].curve() else {
        panic!("closed mixed curve")
    };
    assert!(matches!(curve.segments()[0], CurveSegment3::Arc(_)));
    assert!(matches!(curve.segments()[1], CurveSegment3::Line(_)));
}

#[test]
fn rebuilding_a_closed_composite_records_the_seed_in_the_new_domain() {
    let seed = Curve3::PolyCurve(
        PolyCurve3::concatenate(&[
            line([0., 0.], [2., 0.]).to_polycurve().unwrap(),
            line([2., 0.], [2., 3.]).to_polycurve().unwrap(),
        ])
        .unwrap(),
    );
    let closer = Curve3::NurbsCurve(
        NurbsCurve::try_new(
            2,
            vec![p(2., 3.), p(-1., 1.), p(0., 0.)],
            vec![11., 11., 11., 13., 13., 13.],
        )
        .unwrap(),
    );
    let result = join_curves(
        &[seed, closer],
        CurveJoinOptions {
            tolerance: 0.01,
            preserve_direction: false,
            style: CurveJoinStyle::Seeded,
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(result[0].seed_start_parameter(), Some(13.));
    assert_eq!(result[0].curve().as_ref().domain(), 11.0..=18.0);
    assert_eq!(result[0].curve().as_ref().evaluate(13.).unwrap(), p(0., 0.));
    let restored = result[0]
        .curve()
        .try_change_closed_seam(result[0].seed_start_parameter().unwrap())
        .unwrap();
    assert_eq!(restored.as_ref().domain(), 13.0..=20.0);
    assert_eq!(restored.as_ref().start_point().unwrap(), p(0., 0.));
}
