//! surfaces extend command regressions.
use super::*;

#[test]
fn extend_surface_preserves_tensor_state_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New SurfaceExtensions")
        .unwrap();
    registry
        .execute(&mut document, "SrfPt 0,0,0 3,0,1 3,4,4 0,4,2")
        .unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut document, "SetObjectName ExtendedPatch")
        .unwrap();
    registry
        .execute(&mut document, "Group SurfaceExtension")
        .unwrap();
    let before = document.object(id).unwrap().clone();
    let Geometry::NurbsSurface(source) = before.geometry() else {
        panic!("SrfPt must create a NURBS surface")
    };
    let expected = source.try_extended_u(-0.5..=2.0).unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtendSrf Direction=U Domain=-0.5,2 Type=Smooth Merge=Yes"
            )
            .unwrap(),
        "Smoothly extended the selected surface in U to include domain -0.5..2, merging the extension"
    );
    let object = document.object(id).unwrap();
    assert_eq!(object.geometry(), &Geometry::NurbsSurface(expected.clone()));
    assert_eq!(object.attributes(), before.attributes());
    assert!(document.is_selected(id));
    assert!(
        document
            .group_by_name("SurfaceExtension")
            .unwrap()
            .members()
            .any(|member| member == id)
    );
    assert_eq!(document.undo_label(), Some("ExtendSrf"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.object(id).unwrap(), &before);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(expected)
    );
}

#[test]
fn extend_surface_accepts_a_rhino_compatible_edge_distance() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "SrfPt 0,0,0 10,0,0 20,10,0 0,10,0")
        .unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();
    let Geometry::NurbsSurface(source) = document.object(id).unwrap().geometry() else {
        panic!("SrfPt must create a NURBS surface")
    };
    let expected = source
        .try_extended_by_length(SurfaceExtensionEdge::East, 3.0)
        .unwrap();
    let expected_line = source
        .try_extended_linearly_by_length(SurfaceExtensionEdge::East, 3.0)
        .unwrap();
    let expected_shrink = source
        .try_shrunk_by_length(SurfaceExtensionEdge::East, 2.0, None, document.tolerance())
        .unwrap();
    let expected_shrink_at = source
        .try_shrunk_by_length(
            SurfaceExtensionEdge::East,
            2.0,
            Some(0.0),
            document.tolerance(),
        )
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtendSrf Edge=East Distance=3 Type=Smooth Merge=Yes"
            )
            .unwrap(),
        "Smoothly extended the selected surface at its East edge by distance 3, merging the extension"
    );
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(expected)
    );
    assert_eq!(document.undo_label(), Some("ExtendSrf"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtendSrf Edge=East Distance=3 Type=Line Merge=Yes"
            )
            .unwrap(),
        "Linearly extended the selected surface at its East edge by distance 3, merging the extension"
    );
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(expected_line)
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "ExtendSrf Edge=East Distance=-2 Type=Line")
            .unwrap(),
        "Shrank the selected surface at its East edge inward by distance 2"
    );
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(expected_shrink)
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "ExtendSrf Edge=East Distance=-2 At=0")
            .unwrap(),
        "Shrank the selected surface at its East edge inward by distance 2"
    );
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(expected_shrink_at)
    );
}

#[test]
fn extend_surface_point_pick_resolves_the_natural_edge_and_shrink_path() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "SrfPt 0,0,0 10,0,0 20,10,0 0,10,0")
        .unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();
    let Geometry::NurbsSurface(source) = document.object(id).unwrap().geometry() else {
        panic!("SrfPt must create a NURBS surface")
    };
    let source = source.clone();
    let u = source.parameter_at_u(0.37).unwrap();
    let v = source.parameter_at_v(0.25).unwrap();
    let domain_u = source.domain_u();
    let domain_v = source.domain_v();
    for (expected_edge, expected_parameter, pick) in [
        (
            SurfaceExtensionEdge::South,
            u,
            source.evaluate(u, *domain_v.start()).unwrap(),
        ),
        (
            SurfaceExtensionEdge::East,
            v,
            source.evaluate(*domain_u.end(), v).unwrap(),
        ),
        (
            SurfaceExtensionEdge::North,
            u,
            source.evaluate(u, *domain_v.end()).unwrap(),
        ),
        (
            SurfaceExtensionEdge::West,
            v,
            source.evaluate(*domain_u.start(), v).unwrap(),
        ),
    ] {
        let (edge, parameter) =
            closest_surface_extension_edge(&source, pick, document.tolerance()).unwrap();
        assert_eq!(edge, expected_edge);
        assert!(
            document
                .tolerance()
                .approx_eq(parameter, expected_parameter)
        );
    }

    let east_pick = source.evaluate(*domain_u.end(), v).unwrap();
    let expected_shrink = source
        .try_shrunk_by_length(
            SurfaceExtensionEdge::East,
            2.0,
            Some(v),
            document.tolerance(),
        )
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                &format!(
                    "ExtendSrf {:.17},{:.17},{:.17} Distance=-2 Type=Line Merge=Yes",
                    east_pick.x(),
                    east_pick.y(),
                    east_pick.z()
                ),
            )
            .unwrap(),
        "Shrank the selected surface at its East edge inward by distance 2"
    );
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(expected_shrink)
    );

    registry.execute(&mut document, "Undo").unwrap();
    let south_pick = source.evaluate(u, *domain_v.start()).unwrap();
    let expected_extension = source
        .try_extended_linearly_by_length(SurfaceExtensionEdge::South, 2.0)
        .unwrap();
    registry
        .execute(
            &mut document,
            &format!(
                "ExtendSrf {:.17},{:.17},{:.17} Distance=2 Type=Line Merge=Yes",
                south_pick.x(),
                south_pick.y(),
                south_pick.z()
            ),
        )
        .unwrap();
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(expected_extension)
    );
}

#[test]
fn extend_surface_merge_no_creates_an_attribute_preserving_patch() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New SeparateExtensions")
        .unwrap();
    registry
        .execute(&mut document, "SrfPt 0,0,0 10,0,0 20,10,0 0,10,0")
        .unwrap();
    let source_id = document.objects().next().unwrap().id();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "SetObjectName SeparatePatch")
        .unwrap();
    registry
        .execute(&mut document, "Group ExtensionPair")
        .unwrap();
    let source_before = document.object(source_id).unwrap().clone();
    let Geometry::NurbsSurface(source) = source_before.geometry() else {
        panic!("SrfPt must create a NURBS surface")
    };
    let extended = source
        .try_extended_by_length(SurfaceExtensionEdge::East, 2.0)
        .unwrap();
    let expected =
        separate_surface_extension(source, &extended, SurfaceExtensionEdge::East).unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtendSrf Edge=East Distance=2 Type=Smooth Merge=No"
            )
            .unwrap(),
        "Created a separate smooth surface extension at the East edge by distance 2"
    );
    assert_eq!(document.object(source_id).unwrap(), &source_before);
    let extension = document
        .objects()
        .find(|object| object.id() != source_id)
        .unwrap();
    let extension_id = extension.id();
    assert_eq!(extension.geometry(), &Geometry::NurbsSurface(expected));
    assert_eq!(extension.attributes(), source_before.attributes());
    assert!(!document.is_selected(source_id));
    assert!(!document.is_selected(extension_id));
    assert_eq!(
        document
            .group_by_name("ExtensionPair")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([source_id, extension_id])
    );
    assert_eq!(document.undo_label(), Some("ExtendSrf"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().count(), 1);
    assert_eq!(document.object(source_id).unwrap(), &source_before);
}

#[test]
fn extend_surface_rejects_invalid_requests_selection_and_closed_directions() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "ExtendSrf",
        "ExtendSrf Direction=U",
        "ExtendSrf Domain=0,2",
        "ExtendSrf Direction=Both Domain=0,2",
        "ExtendSrf Direction=U Domain=0",
        "ExtendSrf Direction=U Domain=0,2 Type=Natural",
        "ExtendSrf Direction=U Domain=0,2 Merge=No",
        "ExtendSrf Direction=U Direction=V Domain=0,2",
        "ExtendSrf Edge=East",
        "ExtendSrf Distance=2",
        "ExtendSrf Edge=Middle Distance=2",
        "ExtendSrf Edge=East Distance=2 Direction=U Domain=0,2",
        "ExtendSrf Direction=U Distance=2",
        "ExtendSrf Edge=East Domain=0,2",
        "ExtendSrf 1,2,3",
        "ExtendSrf 1,2,3 Distance=2 Edge=East",
        "ExtendSrf 1,2,3 Distance=-2 At=0.5",
        "ExtendSrf 1,2,3 Direction=U Domain=0,2",
        "ExtendSrf Direction=U Domain=0,2 At=0.5",
        "ExtendSrf Edge=East Distance=2 At=0.5",
        "ExtendSrf Edge=East At=0.5",
        "ExtendSrf Edge=East Distance=-2 Merge=No",
        "ExtendSrf Edge=East Edge=West Distance=2",
        "ExtendSrf Edge=East Distance=2 Distance=3",
    ] {
        assert!(matches!(
            registry.execute(&mut document, command),
            Err(CommandError::Usage(EXTEND_SURFACE_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut document, "ExtendSrf Direction=U Domain=-1,2"),
        Err(CommandError::ExtendSurfaceRequiresOneSurface { actual: 0 })
    ));

    registry.execute(&mut document, "Sphere 0,0,0 2").unwrap();
    let sphere = document.objects().next().unwrap().id();
    document
        .select_object(sphere, SelectionMode::Replace)
        .unwrap();
    let before = document.object(sphere).unwrap().clone();
    assert!(matches!(
        registry.execute(&mut document, "ExtendSrf Direction=U Domain=-1,7"),
        Err(CommandError::Geometry(
            GeometryError::SurfaceExtensionDirectionMustBeOpen { direction: "U" }
        ))
    ));
    assert_eq!(document.object(sphere).unwrap(), &before);
    assert!(matches!(
        registry.execute(&mut document, "ExtendSrf 2,0,0 Distance=1"),
        Err(CommandError::NoExtendableSurfaceEdges)
    ));
    assert_eq!(document.object(sphere).unwrap(), &before);

    document.clear_selection();
    registry
        .execute(&mut document, "SrfPt 0,0 2,0 2,2 0,2")
        .unwrap();
    let patch = document
        .objects()
        .find(|object| {
            object.id() != sphere && matches!(object.geometry(), Geometry::NurbsSurface(_))
        })
        .unwrap()
        .id();
    document
        .select_object(patch, SelectionMode::Replace)
        .unwrap();
    let before = document.object(patch).unwrap().clone();
    assert!(matches!(
        registry.execute(&mut document, "ExtendSrf Edge=East Distance=0"),
        Err(CommandError::Geometry(
            GeometryError::InvalidSurfaceExtensionLength
        ))
    ));
    assert_eq!(document.object(patch).unwrap(), &before);
}
