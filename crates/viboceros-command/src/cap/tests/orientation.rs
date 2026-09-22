use super::*;
use viboceros_geometry::{
    BrepFace, BrepLoop, BrepSolidOrientation, BrepTrim, NurbsCurve2, Point2, WeightedPoint2,
};

fn box_shell(x: Real, radius: Real, opened: bool) -> Brep {
    let source = Brep::try_box(
        CommandContext::default().construction_plane,
        [
            [x - radius, x + radius],
            [-radius, radius],
            [-radius, radius],
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    if opened {
        source
            .sub_brep(&[0, 1, 2, 3, 4], Tolerance::DEFAULT)
            .unwrap()
    } else {
        source
    }
}

#[test]
fn cap_spatial_orientation_does_not_reverse_a_negative_volume_compound() {
    for opening in 0..3 {
        for reverse in [false, true] {
            for swap in [false, true] {
                for preselect in [false, true] {
                    let mut parts = vec![
                        box_shell(-10., 1., opening != 2),
                        box_shell(10., 2., opening != 1).reversed(),
                    ];
                    if swap {
                        parts.reverse();
                    }
                    let mut source = Brep::try_combine(parts, Tolerance::DEFAULT).unwrap();
                    if reverse {
                        source = source.reversed();
                    }
                    assert!(!source.is_solid());
                    let capped = source
                        .try_cap_planar_holes(Tolerance::DEFAULT)
                        .unwrap()
                        .unwrap();
                    // The smaller left shell must face outward. The larger
                    // right shell remains inward, so total volume is -56.
                    let expected = if reverse { capped.reversed() } else { capped };
                    assert_eq!(
                        expected.solid_orientation().unwrap(),
                        BrepSolidOrientation::Outward
                    );
                    assert!(
                        (expected.signed_volume(Tolerance::DEFAULT).unwrap() + 56.).abs() < 1e-10
                    );
                    let mut doc = Document::default();
                    let id = doc
                        .add_geometry_with_attributes(
                            Geometry::Brep(source),
                            ObjectAttributes::on_layer(doc.current_layer_id())
                                .with_name("mixed shells"),
                        )
                        .unwrap();
                    let group = doc.add_group(Some("compound".into()), [id]).unwrap();
                    doc.select_objects_direct([id], SelectionMode::Replace)
                        .unwrap();
                    let before = doc.object(id).unwrap().clone();
                    let registry = CommandRegistry::with_builtins();
                    if preselect {
                        registry.execute(&mut doc, "Cap").unwrap();
                    } else {
                        registry
                            .execute_postselected(&mut doc, "Cap", Default::default())
                            .unwrap();
                    }
                    let after = doc.object(id).unwrap().clone();
                    assert_eq!(after.geometry(), &Geometry::Brep(expected));
                    assert_eq!(after.attributes(), before.attributes());
                    assert_eq!(after.group_ids(), &[group]);
                    assert_eq!(doc.is_selected(id), preselect);
                    assert_eq!(doc.objects().len(), 1);
                    assert_eq!(doc.undo_label(), Some("Cap"));
                    registry.execute(&mut doc, "Undo").unwrap();
                    assert_eq!(doc.object(id), Some(&before));
                    registry.execute(&mut doc, "Redo").unwrap();
                    assert_eq!(doc.object(id), Some(&after));
                }
            }
        }
    }
}

#[test]
fn cap_normalizes_inward_compounds_even_when_signed_volume_is_zero() {
    for reverse in [false, true] {
        let mut source = Brep::try_combine(
            vec![
                box_shell(-10., 1., true),
                box_shell(10., 1., true).reversed(),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        if reverse {
            source = source.reversed();
        }
        let capped = source
            .try_cap_planar_holes(Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(capped.signed_volume(Tolerance::DEFAULT).unwrap().abs() < 1e-12);
        let expected = if reverse { capped.reversed() } else { capped };
        assert_eq!(
            expected.solid_orientation().unwrap(),
            BrepSolidOrientation::Outward
        );
        let mut doc = Document::default();
        let id = doc.add_geometry(Geometry::Brep(source)).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        CommandRegistry::with_builtins()
            .execute(&mut doc, "Cap")
            .unwrap();
        assert_eq!(
            doc.object(id).unwrap().geometry(),
            &Geometry::Brep(expected)
        );
    }
}

// Both encodings have exactly the original segment image. The mixed-weight
// quartic is pole-free but remains outside the same-sign hull certificate;
// its denominator and image bounds are proved in the kernel trim tests.
fn nonlinear_trims(source: Brep, mixed: bool) -> Brep {
    let faces = source
        .faces()
        .iter()
        .map(|face| {
            let loops = face
                .loops()
                .iter()
                .map(|l| {
                    let trims = l
                        .trims()
                        .iter()
                        .map(|trim| {
                            let cp = trim.curve().control_points();
                            assert_eq!(cp.len(), 2);
                            let (a, b) = (cp[0].point(), cp[1].point());
                            let degree = if mixed { 4 } else { 2 };
                            let controls = (0..=degree)
                                .map(|i| {
                                    let t = i as Real / degree as Real;
                                    WeightedPoint2::try_new(
                                        Point2::try_new(
                                            a.x() + t * (b.x() - a.x()),
                                            a.y() + t * (b.y() - a.y()),
                                        )
                                        .unwrap(),
                                        if mixed && i == 2 { -0.125 } else { 1. },
                                    )
                                    .unwrap()
                                })
                                .collect();
                            let mut knots = vec![0.; degree + 1];
                            knots.extend(vec![1.; degree + 1]);
                            let curve =
                                NurbsCurve2::try_new_rational(degree, controls, knots).unwrap();
                            BrepTrim::try_new(
                                trim.vertices(),
                                trim.edge(),
                                trim.is_reversed_3d(),
                                curve,
                                trim.trim_type(),
                                trim.iso(),
                                trim.tolerance(),
                            )
                            .unwrap()
                        })
                        .collect();
                    BrepLoop::try_new(l.loop_type(), trims).unwrap()
                })
                .collect();
            BrepFace::try_new(face.surface().clone(), face.is_reversed(), loops).unwrap()
        })
        .collect();
    Brep::try_new(
        source.vertices().to_vec(),
        source.edges().to_vec(),
        faces,
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn cap_preserves_unresolved_compound_sense_and_reports_uncertainty_atomically() {
    for reverse in [false, true] {
        let mut source = Brep::try_combine(
            vec![
                nonlinear_trims(box_shell(-10., 1., true), true),
                nonlinear_trims(box_shell(10., 2., true), true).reversed(),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        if reverse {
            source = source.reversed();
        }
        let expected = source
            .try_cap_planar_holes(Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(expected.is_solid());
        assert_eq!(expected.edge_connected_face_components().len(), 2);
        assert_eq!(
            expected.solid_orientation().unwrap(),
            BrepSolidOrientation::Unknown
        );
        let volume = expected.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!((volume - if reverse { 56. } else { -56. }).abs() < 1e-10);
        let mut doc = Document::default();
        let id = doc.add_geometry(Geometry::Brep(source)).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let before = doc.object(id).unwrap().clone();
        let registry = CommandRegistry::with_builtins();
        let message = registry.execute(&mut doc, "Cap").unwrap();
        assert!(message.ends_with("orientation unresolved for 1 compound solid(s)"));
        let after = doc.object(id).unwrap().clone();
        assert_eq!(after.geometry(), &Geometry::Brep(expected));
        assert_eq!(doc.undo_label(), Some("Cap"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(id), Some(&before));
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.object(id), Some(&after));
    }
}

#[test]
fn cap_normalizes_higher_degree_linear_trims_without_using_compound_volume_sign() {
    for reverse in [false, true] {
        let mut source = Brep::try_combine(
            vec![
                nonlinear_trims(box_shell(-10., 1., true), false),
                nonlinear_trims(box_shell(10., 2., true), false).reversed(),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        if reverse {
            source.reverse_orientation();
        }
        let capped = source
            .try_cap_planar_holes(Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        let expected = if reverse { capped.reversed() } else { capped };
        assert_eq!(
            expected.solid_orientation().unwrap(),
            BrepSolidOrientation::Outward
        );
        assert!((expected.signed_volume(Tolerance::DEFAULT).unwrap() + 56.).abs() < 1e-10);
        let mut doc = Document::default();
        let id = doc.add_geometry(Geometry::Brep(source)).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let before = doc.object(id).unwrap().clone();
        let registry = CommandRegistry::with_builtins();
        let message = registry.execute(&mut doc, "Cap").unwrap();
        assert!(!message.contains("orientation unresolved"));
        let after = doc.object(id).unwrap().clone();
        assert_eq!(after.geometry(), &Geometry::Brep(expected));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(id), Some(&before));
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.object(id), Some(&after));
    }
}
