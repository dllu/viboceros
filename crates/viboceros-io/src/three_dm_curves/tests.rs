use super::*;
use crate::{
    ThreeDmColorSource, ThreeDmGroup, ThreeDmLayer, ThreeDmModel, ThreeDmObject, read_3dm_file,
    write_3dm_file,
};
use viboceros_geometry::{CurveRef, LineSegment, ParameterSide, Point3, Tolerance, WeightedPoint3};

fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

fn blocks(gap: f64) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [
            (0.0, 0.0),
            (1.0, 1.0),
            (2.0, 0.0),
            (2.0 + gap, 0.0),
            (3.0 + gap, 2.0),
            (4.0 + gap, 0.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let scale = if i < 3 {
                2.0_f64.powi(-700)
            } else {
                2.0_f64.powi(700)
            };
            WeightedPoint3::try_new(p(x, y), scale * if i % 3 == 1 { 0.5 } else { 1.0 }).unwrap()
        })
        .collect(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    )
    .unwrap()
}

fn model(geometry: ThreeDmGeometry) -> ThreeDmModel {
    let mut object = ThreeDmObject::new(geometry, 0);
    object.name = Some("Independent spans".into());
    object.visible = false;
    object.locked = true;
    object.object_color = [13, 71, 201];
    object.color_source = ThreeDmColorSource::Object;
    object.wire_density = -1;
    object.group_indices = vec![2, 0];
    ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Geometry".into(),
            color: [30, 60, 90],
            visible: true,
            locked: false,
        }],
        ["Outer", "Empty", "Overlap"]
            .map(|name| ThreeDmGroup { name: name.into() })
            .to_vec(),
        vec![object],
    )
}

fn view(geometry: &ThreeDmGeometry) -> CurveRef<'_> {
    match geometry {
        ThreeDmGeometry::NurbsCurve(c) => CurveRef::NurbsCurve(c),
        ThreeDmGeometry::PolyCurve(c) => CurveRef::PolyCurve(c),
        ThreeDmGeometry::Line(c) => CurveRef::Line(c),
        ThreeDmGeometry::Arc(c) => CurveRef::Arc(c),
        ThreeDmGeometry::Polyline(c) => CurveRef::Polyline(c),
        _ => panic!("expected a curve"),
    }
}

fn round_trip(source: ThreeDmGeometry, count: usize) -> ThreeDmModel {
    let original = model(source);
    let preserved = original.clone();
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("decomposed.3dm");
    let report = write_3dm_file(&path, &original).unwrap();
    assert_eq!(report.source_object_count, 1);
    assert_eq!(report.written_object_count, count);
    assert_eq!(report.adapted_curve_count, 1);
    assert_eq!(original, preserved);
    let decoded = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    assert_eq!(decoded.unsupported_object_count(), 0);
    assert_eq!(decoded.objects.len(), count);
    assert_eq!(decoded.layers, original.layers);
    assert_eq!(decoded.groups, original.groups);
    for object in &decoded.objects {
        let mut attributes = object.clone();
        attributes.geometry = original.objects[0].geometry.clone();
        assert_eq!(attributes, original.objects[0]);
        let actual = view(&object.geometry);
        let expected = view(&original.objects[0].geometry);
        for i in 0..=64 {
            let t = actual.parameter_at(i as f64 / 64.0).unwrap();
            let side = if i == 64 {
                ParameterSide::Left
            } else {
                ParameterSide::Right
            };
            assert!(
                actual
                    .evaluate(t)
                    .unwrap()
                    .distance_to(expected.evaluate_on_side(t, side).unwrap())
                    .unwrap()
                    < 2e-12
            );
        }
    }
    decoded
}

#[test]
fn connected_blocks_export_as_one_native_polycurve_with_independent_scales() {
    let source = blocks(0.0);
    let decoded = round_trip(ThreeDmGeometry::NurbsCurve(source.clone()), 1);
    let ThreeDmGeometry::PolyCurve(actual) = &decoded.objects[0].geometry else {
        panic!("expected polycurve");
    };
    assert_eq!(actual.parameters(), &[0.0, 1.0, 2.0]);
    assert_eq!(actual.segments().len(), 2);
    for (actual, expected) in actual
        .segments()
        .iter()
        .zip(source.try_split_at_full_order_knots().unwrap())
    {
        let CurveSegment3::NurbsCurve(actual) = actual else {
            panic!("NURBS leaf changed type");
        };
        assert_eq!(actual.control_points(), expected.control_points());
        assert_eq!(actual.knots(), expected.knots());
    }
}

#[test]
fn positional_jumps_expand_to_separate_objects_without_bridging() {
    let decoded = round_trip(ThreeDmGeometry::NurbsCurve(blocks(5.0)), 2);
    assert!(
        decoded
            .objects
            .iter()
            .all(|o| matches!(o.geometry, ThreeDmGeometry::NurbsCurve(_)))
    );
    assert_eq!(view(&decoded.objects[0].geometry).domain(), 0.0..=1.0);
    assert_eq!(view(&decoded.objects[1].geometry).domain(), 1.0..=2.0);
}

#[test]
fn nested_full_order_leaf_splits_retain_outer_parameters_and_other_leaf_types() {
    for gap in [0.0, 5.0] {
        let line = LineSegment::try_new(p(-1.0, 0.0), p(0.0, 0.0), Tolerance::DEFAULT).unwrap();
        let tail =
            LineSegment::try_new(p(4.0 + gap, 0.0), p(5.0 + gap, 0.0), Tolerance::DEFAULT).unwrap();
        let source = PolyCurve3::try_with_segment_domains(
            vec![
                CurveSegment3::Line(line),
                CurveSegment3::NurbsCurve(blocks(gap)),
                CurveSegment3::Line(tail),
            ],
            vec![-1.0, 0.0, 4.0, 6.0],
        )
        .unwrap();
        let decoded = round_trip(
            ThreeDmGeometry::PolyCurve(source),
            if gap == 0.0 { 1 } else { 2 },
        );
        for object in &decoded.objects {
            assert!(matches!(object.geometry, ThreeDmGeometry::PolyCurve(_)));
        }
        if gap == 0.0 {
            let ThreeDmGeometry::PolyCurve(c) = &decoded.objects[0].geometry else {
                unreachable!()
            };
            assert_eq!(c.parameters(), &[-1.0, 0.0, 2.0, 4.0, 6.0]);
            assert!(matches!(c.segments()[0], CurveSegment3::Line(_)));
            assert!(matches!(c.segments()[3], CurveSegment3::Line(_)));
        }
    }
}

#[test]
fn a_closed_piece_is_not_embedded_in_an_invalid_multi_segment_polycurve() {
    let source = NurbsCurve::try_new(
        1,
        vec![
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(0.0, 1.0),
            p(0.0, 0.0),
            p(0.0, 0.0),
            p(2.0, 2.0),
        ],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 4.0, 4.0],
    )
    .unwrap();
    let decoded = round_trip(ThreeDmGeometry::NurbsCurve(source), 2);
    assert!(view(&decoded.objects[0].geometry).is_closed().unwrap());
    assert!(!view(&decoded.objects[1].geometry).is_closed().unwrap());
}

#[test]
fn valid_ordinary_curves_are_borrowed_and_keep_their_representation() {
    let source = ThreeDmGeometry::NurbsCurve(blocks(0.0).try_split(1.0).unwrap().0);
    assert!(matches!(&prepare(&source).unwrap()[0], Cow::Borrowed(_)));
    let original = model(source);
    let temporary = tempfile::tempdir().unwrap();
    let report = write_3dm_file(temporary.path().join("ordinary.3dm"), &original).unwrap();
    assert_eq!(report.adapted_curve_count, 0);
    assert_eq!(report.written_object_count, 1);
}

#[test]
fn collapsed_composite_breaks_fail_without_overwriting_an_existing_file() {
    let start: f64 = 1e16;
    let source =
        PolyCurve3::try_with_segment_domains(vec![blocks(0.0)], vec![start, start.next_up()])
            .unwrap();
    let original = model(ThreeDmGeometry::PolyCurve(source));
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("existing.3dm");
    std::fs::write(&path, b"preserve existing file").unwrap();
    assert!(
        matches!(write_3dm_file(&path,&original),Err(ThreeDmError::InvalidModel(message)) if message.contains("span collapsed"))
    );
    assert_eq!(std::fs::read(path).unwrap(), b"preserve existing file");
}

#[test]
fn standalone_pieces_lifted_out_of_a_polycurve_keep_the_outer_domain() {
    let source = PolyCurve3::try_with_segment_domains(vec![blocks(5.0)], vec![-7.0, 13.0]).unwrap();
    let decoded = round_trip(ThreeDmGeometry::PolyCurve(source), 2);
    assert_eq!(view(&decoded.objects[0].geometry).domain(), -7.0..=3.0);
    assert_eq!(view(&decoded.objects[1].geometry).domain(), 3.0..=13.0);
}

#[test]
fn segment_limit_flushes_a_complete_group_without_losing_the_next_piece() {
    let pieces = (0..=MAX_POLYCURVE_SEGMENTS)
        .map(|i| {
            let start = i as f64;
            Piece {
                curve: LineSegment::try_new(p(start, 0.0), p(start + 1.0, 0.0), Tolerance::DEFAULT)
                    .unwrap()
                    .into(),
                start,
                end: start + 1.0,
                source_junction: false,
            }
        })
        .collect();
    let result = group_pieces(pieces).unwrap();
    assert_eq!(result.len(), 2);
    let ThreeDmGeometry::PolyCurve(first) = &result[0] else {
        panic!("expected full group")
    };
    assert_eq!(first.segments().len(), MAX_POLYCURVE_SEGMENTS);
    assert_eq!(first.domain(), 0.0..=MAX_POLYCURVE_SEGMENTS as f64);
    assert!(matches!(result[1], ThreeDmGeometry::Line(_)));
    assert_eq!(
        view(&result[1]).domain(),
        MAX_POLYCURVE_SEGMENTS as f64..=MAX_POLYCURVE_SEGMENTS as f64 + 1.0
    );
}

#[test]
fn intervals_with_overflowing_total_width_do_not_create_invalid_polycurves() {
    let source = NurbsCurve::try_new(
        1,
        vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)],
        vec![-1e308, -1e308, 0.0, 0.0, 1e308, 1e308],
    )
    .unwrap();
    let decoded = round_trip(ThreeDmGeometry::NurbsCurve(source), 2);
    assert_eq!(view(&decoded.objects[0].geometry).domain(), -1e308..=0.0);
    assert_eq!(view(&decoded.objects[1].geometry).domain(), 0.0..=1e308);
}

#[test]
fn closed_seam_edit_with_independent_scales_exports_without_rotating_again() {
    let source = NurbsCurve::try_new_rational(
        1,
        [
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(1.0, 0.0),
            p(0.0, 1.0),
            p(0.0, 1.0),
            p(0.0, 0.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, point)| {
            WeightedPoint3::try_new(point, 2.0_f64.powi(if i / 2 == 1 { 700 } else { -700 }))
                .unwrap()
        })
        .collect(),
        vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0],
    )
    .unwrap()
    .try_change_closed_seam(2.5)
    .unwrap();
    assert!(source.full_order_knots().next().is_some());
    let decoded = round_trip(ThreeDmGeometry::NurbsCurve(source), 1);
    assert_eq!(view(&decoded.objects[0].geometry).domain(), 2.5..=5.5);
    assert!(view(&decoded.objects[0].geometry).is_closed().unwrap());
}
