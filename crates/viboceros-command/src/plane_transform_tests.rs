use super::*;

fn front_context() -> CommandContext {
    CommandContext {
        construction_plane: Frame3::try_from_directions(
            Point3::try_new(10.0, 20.0, 30.0).unwrap(),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    }
}

#[test]
fn front_plane_transforms_are_not_world_xy_operations() {
    let registry = CommandRegistry::with_builtins();
    let p = |a| Point3::try_from(a).unwrap();
    for (command, expected) in [
        ("Rotate 0,0,0 90", [-3.0, 2.0, 1.0]),
        ("Mirror 0,0,0 0,0,1", [-1.0, 2.0, 3.0]),
        ("Scale2D 0,0,0 2", [2.0, 2.0, 6.0]),
        ("Shear 0,0,0 1,0,0 45", [1.0, 2.0, 4.0]),
        ("ProjectToCPlane DeleteInput=Yes", [1.0, 20.0, 3.0]),
    ] {
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::Point(p([1.0, 2.0, 3.0])))
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute_in_context(&mut document, command, front_context())
            .unwrap();
        let Geometry::Point(actual) = document.object(id).unwrap().geometry() else {
            panic!("point")
        };
        assert!(
            actual.distance_to(p(expected)).unwrap() < 1e-12,
            "{command}: {actual:?}"
        );
    }
}

#[test]
fn picked_scale2d_uses_spatial_distances_even_for_normal_only_references() {
    let registry = CommandRegistry::with_builtins();
    for (references, factor) in [("0,0,5 0,0,10", 2.0), ("3,4,5 6,8,-5", 2.5_f64.sqrt())] {
        let mut document = Document::default();
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, &format!("Scale2D 0,0,0 {references}"))
            .unwrap();
        let Geometry::Point(p) = document.objects().next().unwrap().geometry() else {
            panic!("point")
        };
        assert_eq!(p.z(), 3.0);
        assert!((p.x() - factor).abs() < 1e-12);
        assert!((p.y() - 2.0 * factor).abs() < 1e-12);
    }
}

#[test]
fn shear_uses_spatial_angle_obliquity_and_exact_parallel_identity() {
    let registry = CommandRegistry::with_builtins();
    for (target, factor) in [
        ("25", 25.0_f64.to_radians().tan() * 2.0_f64.sqrt()),
        ("0,6,2", 2.0_f64.sqrt() * 844.0_f64.sqrt() / 34.0),
        ("6,8,10", 0.0),
        ("6,8,-2", 3.0 / 2.0_f64.sqrt()),
    ] {
        let mut document = Document::default();
        registry.execute(&mut document, "Point 3,4,7").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, &format!("Shear 0,0,0 3,4,5 {target}"))
            .unwrap();
        let Geometry::Point(p) = document.objects().next().unwrap().geometry() else {
            panic!("point")
        };
        assert!((p.x() - (3.0 - 4.0 * factor)).abs() < 1e-12);
        assert!((p.y() - (4.0 + 3.0 * factor)).abs() < 1e-12);
        assert_eq!(p.z(), 7.0);
    }
}

#[test]
fn contextual_transforms_preserve_attributes_groups_identity_and_one_step_undo() {
    let registry = CommandRegistry::with_builtins();
    for command in [
        "Rotate 0,0,0 25",
        "Mirror 0,0,0 0,0,1",
        "Scale2D 0,0,0 -2",
        "Shear 0,0,0 1,0,0 25",
        "ProjectToCPlane",
    ] {
        for copy in [false, true] {
            let mut document = Document::default();
            for input in [
                "Line 1,2,3 4,5,6",
                "SelAll",
                "SetObjectName Source",
                "Group SourceGroup",
            ] {
                registry.execute(&mut document, input).unwrap();
            }
            let id = document.objects().next().unwrap().id();
            let before = document.object(id).unwrap().clone();
            let undo = document.undo_label().map(str::to_owned);
            let option = if command == "ProjectToCPlane" {
                format!("DeleteInput={}", if copy { "No" } else { "Yes" })
            } else {
                format!("Copy={}", if copy { "Yes" } else { "No" })
            };
            registry
                .execute_in_context(
                    &mut document,
                    &format!("{command} {option}"),
                    front_context(),
                )
                .unwrap();
            assert_eq!(document.objects().len(), if copy { 2 } else { 1 });
            assert_eq!(document.groups().len(), if copy { 2 } else { 1 });
            assert_eq!(
                document
                    .group_by_name("SourceGroup")
                    .unwrap()
                    .members()
                    .len(),
                1
            );
            assert!(document.is_selected(id));
            assert_eq!(
                document.object(id).unwrap().attributes(),
                before.attributes()
            );
            if copy {
                assert_eq!(document.object(id).unwrap().geometry(), before.geometry());
            }
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().len(), 1);
            assert_eq!(document.groups().len(), 1);
            assert_eq!(document.object(id).unwrap(), &before);
            assert_eq!(document.undo_label(), undo.as_deref());
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(document.objects().len(), if copy { 2 } else { 1 });
            assert_eq!(document.groups().len(), if copy { 2 } else { 1 });
        }
    }
}

#[test]
fn rejected_planar_transforms_do_not_mutate_the_document() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for input in ["Point 1,2,3", "SelAll"] {
        registry.execute(&mut document, input).unwrap();
    }
    let before = document.objects().next().unwrap().clone();
    for command in [
        "Rotate 0,0,0 0,1,0 1,0,0",
        "Mirror 0,0,0 0,1,0",
        "Shear 0,0,0 0,1,0 30",
        "Scale2D 0,0,0 0",
        "ProjectToCPlane DeleteInput=Maybe",
    ] {
        assert!(
            registry
                .execute_in_context(&mut document, command, front_context())
                .is_err(),
            "{command}"
        );
        assert_eq!(document.objects().next().unwrap(), &before);
        assert_eq!(document.undo_label(), Some("Point"));
        assert!(document.is_selected(before.id()));
    }
}

#[test]
fn shear_sign_for_parallel_planar_references_is_rotation_covariant() {
    let registry = CommandRegistry::with_builtins();
    for x in [[1.0, 0.0, 0.0], [0.6, 0.8, 0.0]] {
        let frame = Frame3::try_from_directions(
            Point3::try_new(4.0, 5.0, 6.0).unwrap(),
            Vector3::try_from(x).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut document = Document::default();
        let point = |v| frame.point_at(v).unwrap();
        let id = document
            .add_geometry(Geometry::Point(point([3.0, 4.0, 7.0])))
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute_in_context(
                &mut document,
                &format!(
                    "Shear {} {} {}",
                    format_point(point([0.0, 0.0, 0.0])),
                    format_point(point([3.0, 4.0, 5.0])),
                    format_point(point([6.0, 8.0, -2.0]))
                ),
                CommandContext {
                    construction_plane: frame,
                },
            )
            .unwrap();
        let Geometry::Point(actual) = document.object(id).unwrap().geometry() else {
            panic!("point")
        };
        let factor = 3.0 / 2.0_f64.sqrt();
        assert!(
            actual
                .distance_to(point([3.0 - 4.0 * factor, 4.0 + 3.0 * factor, 7.0]))
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn heavily_tilted_shear_retains_a_resolved_clockwise_projection() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in ["Point 3,4,7", "SelAll", "Shear 0,0,0 3,4,1e100 6,0,1e100"] {
        registry.execute(&mut document, command).unwrap();
    }
    let Geometry::Point(p) = document.objects().next().unwrap().geometry() else {
        panic!("point")
    };
    assert!(
        p.distance_to(Point3::try_new(7.0, 1.0, 7.0).unwrap())
            .unwrap()
            < 1e-12
    );
}
