use super::*;
use crate::CommandRegistry;
use viboceros_document::{Geometry, SelectionMode};
use viboceros_geometry::{NurbsCurve, PointCloud3, WeightedPoint3};

mod projection;

fn p(a: [f64; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}
fn cloud(a: [f64; 3], b: [f64; 3]) -> Geometry {
    Geometry::PointCloud(PointCloud3::try_new(vec![p(a), p(b)]).unwrap())
}
fn setup() -> Document {
    let mut doc = Document::default();
    doc.add_geometry(cloud([0., 0., 0.], [2., 1., 3.])).unwrap();
    doc.add_geometry(cloud([5., 2., 0.], [9., 4., 2.])).unwrap();
    doc.add_geometry(cloud([16., 8., 5.], [17., 10., 8.]))
        .unwrap();
    doc.select_all();
    doc
}
fn locations(doc: &Document) -> Vec<[f64; 3]> {
    doc.objects()
        .map(|o| match o.geometry() {
            Geometry::PointCloud(c) => c.points()[0].to_array(),
            Geometry::Point(p) => p.to_array(),
            _ => panic!("point fixture"),
        })
        .collect()
}

#[test]
fn seven_modes_use_correct_axes_and_overall_extrema_or_centers() {
    for (mode, expected) in [
        ("Left", [[0., 0., 0.], [0., 2., 0.], [0., 8., 5.]]),
        ("Right", [[15., 0., 0.], [13., 2., 0.], [16., 8., 5.]]),
        ("Bottom", [[0., 0., 0.], [5., 0., 0.], [16., 0., 5.]]),
        ("Top", [[0., 9., 0.], [5., 8., 0.], [16., 8., 5.]]),
        ("HorizCenter", [[0., 4.5, 0.], [5., 4., 0.], [16., 4., 5.]]),
        ("VertCenter", [[7.5, 0., 0.], [6.5, 2., 0.], [8., 8., 5.]]),
        ("Concentric", [[7.5, 4.5, 0.], [6.5, 4., 0.], [8., 4., 5.]]),
    ] {
        let mut doc = setup();
        CommandRegistry::with_builtins()
            .execute(&mut doc, &format!("Align {mode}"))
            .unwrap();
        assert_eq!(locations(&doc), expected, "{mode}");
    }
}

#[test]
fn picked_point_ignores_height_and_world_bypasses_rotated_cplane() {
    let plane = Frame3::try_from_directions(
        p([10., -20., 30.]),
        Vector3::try_from([0., 1., 0.]).unwrap(),
        Vector3::try_from([0., 0., 1.]).unwrap(),
        viboceros_geometry::Tolerance::DEFAULT,
    )
    .unwrap();
    for (system, expected) in [
        ("World", [[19., -4.5, 0.], [18., -5., 0.], [19.5, -5., 5.]]),
        (
            "CPlane",
            [[0., -4.5, 15.5], [5., -5., 16.], [16., -5., 15.5]],
        ),
    ] {
        let mut doc = setup();
        CommandRegistry::with_builtins()
            .execute_in_context(
                &mut doc,
                &format!("Align Concentric AlignTo={system} 20,-4,17"),
                CommandContext {
                    construction_plane: plane,
                },
            )
            .unwrap();
        assert_eq!(locations(&doc), expected);
    }
}

#[test]
fn shared_top_membership_and_partial_selection_preserve_hidden_peers() {
    let mut doc = setup();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.add_group(Some("older".into()), [ids[0], ids[1]])
        .unwrap();
    doc.add_group(Some("newer".into()), [ids[1], ids[2]])
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "Align Concentric 20,-4,17")
        .unwrap();
    assert_eq!(
        locations(&doc),
        [[19., -4.5, 0.], [14., -8., 0.], [25., -2., 5.]]
    );
    doc.undo().unwrap();
    doc.select_objects_direct([ids[0], ids[2]], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut doc, "Align Concentric 20,-4,17")
        .unwrap();
    assert_eq!(
        locations(&doc),
        [[19., -4.5, 0.], [5., 2., 0.], [19.5, -5., 5.]]
    );
    assert!(!doc.is_selected(ids[1]));
}

#[test]
fn alignment_preserves_identity_attributes_groups_and_selection_across_history() {
    let mut doc = setup();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.add_group(Some("pair".into()), [ids[0], ids[1]])
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Align Right 20,-4,17").unwrap();
    let after = doc.objects().cloned().collect::<Vec<_>>();
    assert_eq!(doc.undo_label(), Some("Align"));
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(a.id(), b.id());
        assert_eq!(a.attributes(), b.attributes());
        assert_eq!(a.group_ids(), b.group_ids());
    }
    for _ in 0..2 {
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        doc.redo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
    }
}

#[test]
fn no_op_and_rejected_input_preserve_redo_and_document() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = setup();
    registry.execute(&mut doc, "Align Left").unwrap();
    doc.undo().unwrap();
    for input in [
        "Align",
        "Align Left Right",
        "Align Left Auto 1,2,3",
        "Align Left AlignTo=Space",
        "Align Left NaN,0,0",
        "Align Left AlignTo=World AlignTo=CPlane",
    ] {
        let before = format!("{doc:?}");
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(format!("{doc:?}"), before, "{input}");
    }
    let ids = doc.selected_object_ids().collect::<Vec<_>>();
    doc.select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    registry.execute(&mut doc, "Align Concentric Auto").unwrap();
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn tight_curve_bounds_do_not_align_the_control_polygon() {
    let curve = NurbsCurve::try_new_rational(
        2,
        [[0., 0., 0.], [1., 4., 0.], [2., 0., 0.]]
            .into_iter()
            .map(|a| WeightedPoint3::try_new(p(a), 1.).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    doc.add_geometry(Geometry::Point(p([9., 8., 0.]))).unwrap();
    doc.select_all();
    CommandRegistry::with_builtins()
        .execute(&mut doc, "Align Top")
        .unwrap();
    let Geometry::NurbsCurve(curve) = doc.object(id).unwrap().geometry() else {
        panic!("curve")
    };
    assert!((curve.evaluate(0.5).unwrap().y() - 8.).abs() < 1e-8);
    assert!((curve.evaluate(0.).unwrap().y() - 6.).abs() < 1e-8);
}

#[test]
fn bounds_failure_is_atomic_even_after_other_units_are_validated() {
    let pole = NurbsCurve::try_new_rational(
        2,
        [[0., 0., 0.], [1., 4., 0.], [2., 0., 0.]]
            .into_iter()
            .zip([1., -1., 1.])
            .map(|(a, w)| WeightedPoint3::try_new(p(a), w).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let mut doc = setup();
    doc.add_geometry(Geometry::NurbsCurve(pole)).unwrap();
    doc.select_all();
    let before = format!("{doc:?}");
    assert!(
        CommandRegistry::with_builtins()
            .execute(&mut doc, "Align Left")
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn far_display_plane_origin_does_not_erase_local_extents() {
    let mut doc = setup();
    let plane = CommandContext::default()
        .construction_plane
        .with_origin(p([1e200, -1e200, 1e200]));
    CommandRegistry::with_builtins()
        .execute_in_context(
            &mut doc,
            "Align Concentric",
            CommandContext {
                construction_plane: plane,
            },
        )
        .unwrap();
    assert_eq!(
        locations(&doc),
        [[7.5, 4.5, 0.], [6.5, 4., 0.], [8., 4., 5.]]
    );
}

#[test]
fn coordinate_preference_is_shared_by_ui_and_commands_but_not_registries() {
    let registry = CommandRegistry::with_builtins();
    registry
        .accept_object_selection_input("Align AlignTo=World")
        .unwrap();
    assert!(
        registry
            .object_selection_prompt("_Align _Top")
            .unwrap()
            .unwrap()
            .command_line()
            .contains("AlignTo=World")
    );
    assert!(
        CommandRegistry::with_builtins()
            .object_selection_prompt("Align")
            .unwrap()
            .unwrap()
            .command_line()
            .contains("AlignTo=CPlane")
    );
    assert!(
        registry
            .accept_object_selection_input("Align AlignTo=CPlane AlignTo=World")
            .is_err()
    );
    assert!(
        registry
            .object_selection_prompt("Align")
            .unwrap()
            .unwrap()
            .command_line()
            .contains("AlignTo=World")
    );
    assert!(
        registry
            .object_selection_prompt("Align Left Auto")
            .unwrap()
            .is_none()
    );
    assert!(
        registry
            .object_selection_prompt("Align Left 1,2,3")
            .unwrap()
            .is_none()
    );
}

#[test]
fn late_unrepresentable_translation_rolls_back_staged_valid_objects() {
    let mut doc = Document::default();
    for x in [0., f64::MAX] {
        doc.add_geometry(Geometry::Point(p([x, 0., 0.]))).unwrap();
    }
    doc.select_all();
    let before = format!("{doc:?}");
    let result = CommandRegistry::with_builtins()
        .execute(&mut doc, &format!("Align Left {},0,0", -f64::MAX));
    assert!(result.is_err());
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn postselected_alignment_deselects_only_after_success() {
    let mut doc = setup();
    let registry = CommandRegistry::with_builtins();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    assert!(
        registry
            .execute_postselected(&mut doc, "Align Left Auto 0,0,0", CommandContext::default())
            .is_err()
    );
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
    registry
        .execute_postselected(&mut doc, "Align Left", CommandContext::default())
        .unwrap();
    assert_eq!(doc.selected_object_ids().count(), 0);
    assert_eq!(doc.undo_label(), Some("Align"));
}
