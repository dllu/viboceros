use super::*;
use crate::{CurveSegment3, LineSegment, NurbsCurve, PolyCurve3, WeightedPoint3};

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn curve(weight: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(2., 0.), weight).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn weighted_line_forward_inverse_and_lookup_queries_use_projective_parameters() {
    for weight in [16., 2_f64.powi(100)] {
        let curve = curve(weight);
        let mut sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        sampler.prepare_repeated_sampling(8).unwrap();
        assert_eq!(sampler.total_length(), 2.);
        let t = sampler.parameter_at_distance(1.).unwrap();
        assert!((t * (weight + 1.) - 1.).abs() < 1e-12);
        assert!((sampler.distance_at_parameter(t).unwrap() - 1.).abs() < 1e-12);
        assert!(
            sampler
                .point_at_distance(1.)
                .unwrap()
                .distance_to(p(1., 0.))
                .unwrap()
                < 1e-12
        );
        assert!(curve.evaluate(t).unwrap().distance_to(p(1., 0.)).unwrap() < 1e-12);
        assert!(
            (sampler.distance_at_parameter(0.5).unwrap() - 2. * weight / (weight + 1.)).abs()
                < 1e-12
        );
        assert!(sampler.lookup_tables.iter().all(Vec::is_empty));
    }
}

#[test]
fn projective_line_spans_keep_world_geometry_inside_polycurve_parameter_maps() {
    let composite = PolyCurve3::try_with_segment_domains(
        vec![
            CurveSegment3::Line(
                LineSegment::try_new(p(-2., 0.), p(0., 0.), Tolerance::DEFAULT).unwrap(),
            ),
            CurveSegment3::NurbsCurve(curve(2_f64.powi(100))),
            CurveSegment3::Line(
                LineSegment::try_new(p(2., 0.), p(2., 3.), Tolerance::DEFAULT).unwrap(),
            ),
        ],
        vec![0., 0.25, 0.75, 1.],
    )
    .unwrap();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::PolyCurve(&composite), Tolerance::DEFAULT).unwrap();
    assert_eq!(sampler.total_length(), 7.);
    for (d, expected) in [(1., p(-1., 0.)), (3., p(1., 0.)), (5., p(2., 1.))] {
        assert!(
            sampler
                .point_at_distance(d)
                .unwrap()
                .distance_to(expected)
                .unwrap()
                < 1e-12
        );
        assert!(
            sampler
                .sample_at_distance(d)
                .unwrap()
                .point()
                .distance_to(expected)
                .unwrap()
                < 1e-12
        );
    }
}
