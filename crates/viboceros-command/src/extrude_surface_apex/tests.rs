use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn patch() -> Geometry {
    Geometry::NurbsSurface(
        NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![p(0., 0., 0.), p(2., 0., 0.), p(0., 3., 0.), p(2., 3., 1.)],
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap(),
    )
}
#[test]
fn surface_apex_options_parse_strictly_and_update_atomically() {
    let mut options = SurfaceApexOptions::default();
    assert!(!options.solid && !options.split_at_tangents);
    assert!(SurfaceApexInput::parse(&["Solid"]).unwrap().options.solid);
    assert_eq!(
        SurfaceApexInput::parse(&["ToBoundary"]).unwrap().boundary,
        BoundaryInput::Select
    );
    options
        .update(&["Solid=No", "DeleteInput=Yes", "SplitAtTangents=Yes"])
        .unwrap();
    assert!(options.split_at_tangents);
    let before = options;
    assert!(options.update(&["Solid=Yes", "DeleteInput=bad"]).is_err());
    assert_eq!(options, before);
    assert!(SurfaceApexInput::parse(&["1,2,3", "Direction=0,0,1"]).is_err());
    assert!(SurfaceApexInput::parse(&["Solid=Yes", "Solid=No"]).is_err());
    assert!(SurfaceApexInput::parse(&["Face=0", "Faces=All"]).is_err());
    assert!(SurfaceApexInput::parse(&["SplitAtTangents=Yes", "SplitAtTangents=No"]).is_err());
    assert!(
        options
            .update(&["Solid=Yes", "SplitAtTangents=bad"])
            .is_err()
    );
    assert_eq!(options, before);
    assert_eq!(
        SurfaceApexInput::parse(&["1,2,3"]).unwrap().apex,
        Some(ApexPoint::Explicit(p(1., 2., 3.)))
    );
    assert!(SurfaceApexInput::parse(&["1,2,3", "4,5,6"]).is_err());
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn surface_apex_portable_decline_preserves_the_document() {
    let mut doc = Document::default();
    let s = doc.add_geometry(patch()).unwrap();
    doc.select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    assert!(matches!(
        CommandRegistry::with_builtins()
            .execute(&mut doc, "ExtrudeSrfToPoint 1,1.5,6 DeleteInput=Yes"),
        Err(CommandError::SurfaceApexExtrusionRequiresNativeKernel)
    ));
    assert_eq!(format!("{doc:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_curved_cap_delete_history_and_uncapped_output() {
    let mut doc = Document::default();
    let s = doc.add_geometry(patch()).unwrap();
    doc.select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    let source = doc.object(s).unwrap().clone();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(
            &mut doc,
            "ExtrudeSrfToPoint 1,1.5,6 Solid=Yes DeleteInput=Yes",
        )
        .unwrap();
    assert!(doc.object(s).is_none());
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(b.is_solid());
    assert!((b.signed_volume(doc.tolerance()).unwrap().abs() - 11.5).abs() < 1e-5);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(s), Some(&source));
    registry.execute(&mut doc, "Redo").unwrap();
    assert!(doc.object(s).is_none());
    registry.execute(&mut doc, "Undo").unwrap();
    registry
        .execute(
            &mut doc,
            &format!("ExtrudeSrfToPoint 1,1.5,-6 Solid=No FaceRefs={s}:0"),
        )
        .unwrap();
    assert!(
        doc.selected_objects()
            .all(|o| matches!(o.geometry(),Geometry::Brep(b)if !b.is_solid()))
    );
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_face_delete_preserves_remainder_identity_attributes_and_groups() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let s = doc
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                CommandContext::default().construction_plane,
                [[0., 1.], [0., 1.], [0., 1.]],
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let group = doc.add_group(None, [s]).unwrap();
    let source = doc.object(s).unwrap().clone();
    registry
        .execute(
            &mut doc,
            &format!("ExtrudeSrfToPoint 2,2,2 FaceRefs={s}:0,{s}:0 DeleteInput=Yes"),
        )
        .unwrap();
    let remaining = doc.object(s).unwrap();
    assert_eq!(remaining.attributes(), source.attributes());
    let Geometry::Brep(b) = remaining.geometry() else {
        panic!()
    };
    assert_eq!(b.faces().len(), 5);
    assert_eq!(
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![s]
    );
    assert_eq!(doc.selected_object_count(), 1);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(s), Some(&source));
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_coplanar_rectangles_match_rhino_topology() {
    for (apex, open, capped) in [
        (p(1., 1.5, 0.), [4, 8, 5], [5, 8, 5]),
        (p(3.7, 4.1, 0.), [4, 8, 5], [5, 8, 5]),
        (p(1., 0., 0.), [4, 8, 5], [5, 8, 5]),
        (p(0., 0., 0.), [2, 5, 4], [3, 7, 5]),
        (p(3.7, 0., 0.), [4, 8, 5], [5, 8, 5]),
        (p(0., 0., 1e-8), [4, 8, 5], [5, 8, 5]),
        (p(1e-8, 1e-8, 0.), [4, 8, 5], [5, 8, 5]),
    ] {
        for (solid, expected) in [(false, open), (true, capped)] {
            let mut doc = Document::default();
            let source = doc.add_geometry(planar_rectangle(0., 0.)).unwrap();
            let registry = CommandRegistry::with_builtins();
            let [x, y, z] = apex.to_array();
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "ExtrudeSrfToPoint {x},{y},{z} FaceRefs={source}:0 Solid={}",
                        if solid { "Yes" } else { "No" }
                    ),
                )
                .unwrap();
            let [object] = doc.selected_objects().collect::<Vec<_>>()[..] else {
                panic!("linked rectangle should produce one object");
            };
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("expected B-rep");
            };
            assert_eq!(
                [
                    brep.faces().len(),
                    brep.edges().len(),
                    brep.vertices().len()
                ],
                expected,
                "{apex:?}, Solid={solid}"
            );
            assert_eq!(brep.is_solid(), solid && apex != p(0., 0., 0.));
            assert!(brep.vertices().iter().any(|vertex| vertex.point() == apex));
            assert!(brep.faces().iter().all(|face| {
                face.surface()
                    .control_points()
                    .iter()
                    .all(|control| [0., z].contains(&control.point().z()))
            }));
            if y == 0. && z == 0. && x != 0. {
                let (_, sources) = brep
                    .display_mesh_with_face_sources(4, doc.tolerance())
                    .unwrap();
                assert_eq!(
                    sources.into_iter().collect::<BTreeSet<_>>().len(),
                    brep.faces().len() - 1
                );
            }
            if solid {
                let Geometry::NurbsSurface(original) = doc.object(source).unwrap().geometry()
                else {
                    panic!("expected original surface");
                };
                assert_eq!(brep.faces()[0].surface(), original);
            }
        }
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_coplanar_annulus_keeps_closed_rational_profiles_and_trim_holes() {
    let mut doc = Document::default();
    let tolerance = doc.tolerance();
    let center = p(0., 0., 0.);
    let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    let circle = |r| {
        Circle3::try_from_center_point(center, p(r, 0., 0.), normal, tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let base = Brep::try_planar_face_with_holes(&circle(3.), &[circle(1.)], tolerance).unwrap();
    let source = doc.add_geometry(Geometry::Brep(base.clone())).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (point, wall_count) in [("0,0,0", 2), ("4,1,0", 2), ("3,0,0", 1), ("1,0,0", 1)] {
        for solid in [false, true] {
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "ExtrudeSrfToPoint {point} FaceRefs={source}:0 Solid={}",
                        if solid { "Yes" } else { "No" }
                    ),
                )
                .unwrap();
            assert_eq!(
                doc.selected_object_count(),
                if solid { 1 } else { wall_count }
            );
            for object in doc.selected_objects() {
                let Geometry::Brep(brep) = object.geometry() else {
                    panic!("expected B-rep");
                };
                assert_eq!(brep.faces().len(), if solid { wall_count + 1 } else { 1 });
                assert_eq!(brep.is_solid(), solid && wall_count == 2);
                if solid {
                    assert_eq!(brep.faces()[0].loops().len(), 2);
                    assert_eq!(brep.faces()[0].surface(), base.faces()[0].surface());
                }
                for wall in &brep.faces()[usize::from(solid)..] {
                    assert_eq!(wall.surface().degree_v(), 2);
                    assert!(
                        wall.surface()
                            .control_points()
                            .iter()
                            .any(|p| p.weight() != 1.)
                    );
                }
            }
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().len(), 1);
        }
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_at_a_single_closed_seam_keeps_only_the_cap_or_apex() {
    let mut doc = Document::default();
    let tolerance = doc.tolerance();
    let apex = p(3., 0., 0.);
    let outline = Circle3::try_from_center_point(
        p(0., 0., 0.),
        apex,
        UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let base = Brep::try_planar_face(&outline, tolerance).unwrap();
    let source = doc.add_geometry(Geometry::Brep(base.clone())).unwrap();
    let registry = CommandRegistry::with_builtins();
    for solid in [false, true] {
        registry
            .execute(
                &mut doc,
                &format!(
                    "ExtrudeSrfToPoint 3,0,0 FaceRefs={source}:0 Solid={} DeleteInput=Yes",
                    if solid { "Yes" } else { "No" }
                ),
            )
            .unwrap();
        assert!(doc.object(source).is_none());
        let [object] = doc.objects().collect::<Vec<_>>()[..] else {
            panic!("expected one product");
        };
        assert_eq!(
            object.geometry(),
            &if solid {
                Geometry::Brep(base.duplicate_faces(&[0], tolerance).unwrap())
            } else {
                Geometry::Point(apex)
            }
        );
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), 1);
        assert_eq!(
            doc.object(source).unwrap().geometry(),
            &Geometry::Brep(base.clone())
        );
    }
}

#[cfg(feature = "native-smlib")]
fn planar_rectangle(x: f64, z: f64) -> Geometry {
    Geometry::NurbsSurface(
        NurbsSurface::try_bilinear([p(x, 0., z), p(x + 2., 0., z), p(x + 2., 3., z), p(x, 3., z)])
            .unwrap(),
    )
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_later_boundary_miss_preserves_batch_and_redo() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let s = doc.add_geometry(planar_rectangle(0., 0.)).unwrap();
    let other = doc.add_geometry(planar_rectangle(20., 0.)).unwrap();
    let boundary = doc
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                p(-1., -1., 6.),
                p(3., -1., 6.),
                p(3., 4., 6.),
                p(-1., 4., 6.),
            ])
            .unwrap(),
        ))
        .unwrap();
    registry.execute(&mut doc, "Point 50,50,50").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    assert!(
        registry
            .execute(
                &mut doc,
                &format!(
                    "ExtrudeSrfToPoint 1,1.5,12 ToBoundary={boundary} FaceRefs={s}:0,{other}:0 DeleteInput=Yes"
                )
            )
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}
