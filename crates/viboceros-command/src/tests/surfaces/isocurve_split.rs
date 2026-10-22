//! surfaces isocurve split command regressions.
use super::*;

#[test]
fn split_surface_at_isocurve_preserves_exact_pieces_and_document_metadata() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New SplitSurfaces")
        .unwrap();
    let source_id = document
        .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
        .unwrap();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "SetObjectName SplitSurfaceSource")
        .unwrap();
    registry
        .execute(&mut document, "Group SplitSurfacePieces")
        .unwrap();
    let source = document.object(source_id).unwrap().clone();

    assert_eq!(
        registry
            .execute(&mut document, "Split Isocurve=4,6,0 Direction=U Shrink=Yes",)
            .unwrap(),
        "Split the selected surface along U isocurves into 2 exact B-rep face(s)"
    );
    assert!(document.object(source_id).is_none());
    let outputs = document.objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for object in &outputs {
        assert_eq!(object.attributes(), source.attributes());
        assert!(document.is_selected(object.id()));
    }
    let domains = outputs
        .iter()
        .map(|object| match object.geometry() {
            Geometry::Brep(brep) if brep.faces().len() == 1 => {
                let face = &brep.faces()[0];
                assert_eq!(
                    face.rectangular_trim_bounds(Tolerance::DEFAULT).unwrap(),
                    Some([
                        [
                            *face.surface().domain_u().start(),
                            *face.surface().domain_u().end()
                        ],
                        [
                            *face.surface().domain_v().start(),
                            *face.surface().domain_v().end()
                        ],
                    ])
                );
                (face.surface().domain_u(), face.surface().domain_v())
            }
            geometry => panic!("isocurve Split created unexpected geometry {geometry:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        domains,
        vec![(0.0..=1.0, 0.0..=0.6), (0.0..=1.0, 0.6..=1.0)]
    );
    assert_eq!(
        document
            .group_by_name("SplitSurfacePieces")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        outputs.iter().map(|object| object.id()).collect()
    );
    assert_eq!(document.undo_label(), Some("Split"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().count(), 1);
    assert_eq!(document.object(source_id).unwrap(), &source);
}

#[test]
fn split_surface_at_isocurve_supports_v_and_both_directions() {
    let registry = CommandRegistry::with_builtins();
    let split = |direction| {
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
            .unwrap();
        document.select_object(id, SelectionMode::Replace).unwrap();
        registry
            .execute(
                &mut document,
                &format!("Split Isocurve 4,6,0 Direction={direction}"),
            )
            .unwrap();
        document
    };

    let v = split("V");
    let v_domains = v
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Brep(brep) if brep.faces().len() == 1 => {
                let surface = brep.faces()[0].surface();
                (surface.domain_u(), surface.domain_v())
            }
            geometry => panic!("V isocurve Split created unexpected geometry {geometry:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        v_domains,
        vec![(0.0..=0.4, 0.0..=1.0), (0.4..=1.0, 0.0..=1.0)]
    );

    let both = split("Both");
    let mut both_domains = both
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Brep(brep) if brep.faces().len() == 1 => {
                let surface = brep.faces()[0].surface();
                [
                    *surface.domain_u().start(),
                    *surface.domain_u().end(),
                    *surface.domain_v().start(),
                    *surface.domain_v().end(),
                ]
            }
            geometry => {
                panic!("two-direction isocurve Split created unexpected geometry {geometry:?}")
            }
        })
        .collect::<Vec<_>>();
    both_domains.sort_by(|left, right| {
        left.iter()
            .zip(right)
            .find_map(|(left, right)| {
                let ordering = left.total_cmp(right);
                ordering.ne(&std::cmp::Ordering::Equal).then_some(ordering)
            })
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    assert_eq!(
        both_domains,
        vec![
            [0.0, 0.4, 0.0, 0.6],
            [0.0, 0.4, 0.6, 1.0],
            [0.4, 1.0, 0.0, 0.6],
            [0.4, 1.0, 0.6, 1.0],
        ]
    );
    assert_eq!(both.selected_object_count(), 4);
}

#[test]
fn split_surface_at_isocurve_can_retain_the_complete_underlying_surface() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = planar_intersection_surface();
    let source_id = document
        .add_geometry(Geometry::NurbsSurface(source.clone()))
        .unwrap();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "Split Isocurve=4,6,0 Direction=Both Shrink=No",
            )
            .unwrap(),
        "Split the selected surface along U and V isocurves into 4 exact B-rep face(s)"
    );
    assert!(document.object(source_id).is_none());
    let expected_bounds = [
        [[0.0, 0.4], [0.0, 0.6]],
        [[0.0, 0.4], [0.6, 1.0]],
        [[0.4, 1.0], [0.0, 0.6]],
        [[0.4, 1.0], [0.6, 1.0]],
    ];
    for (object, expected) in document.objects().zip(expected_bounds) {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("unshrunk isocurve Split must create B-rep faces")
        };
        assert_eq!(brep.faces().len(), 1);
        assert_eq!(brep.faces()[0].surface(), &source);
        assert_eq!(
            brep.faces()[0]
                .rectangular_trim_bounds(document.tolerance())
                .unwrap(),
            Some(expected)
        );
        assert!(document.is_selected(object.id()));
        brep.tessellate(2, document.tolerance()).unwrap();
    }
}

#[test]
fn split_surface_at_isocurve_resplits_rectangular_brep_faces() {
    let registry = CommandRegistry::with_builtins();
    let source_surface = planar_intersection_surface();
    let expected_bounds = [
        [[0.2, 0.4], [0.1, 0.6]],
        [[0.2, 0.4], [0.6, 0.9]],
        [[0.4, 0.8], [0.1, 0.6]],
        [[0.4, 0.8], [0.6, 0.9]],
    ];

    for shrink in [false, true] {
        let mut document = Document::default();
        let source_id = document
            .add_geometry(Geometry::Brep(
                Brep::try_rectangular_surface_face_with_orientation(
                    source_surface.clone(),
                    0.2..=0.8,
                    0.1..=0.9,
                    true,
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .select_object(source_id, SelectionMode::Replace)
            .unwrap();

        registry
            .execute(
                &mut document,
                &format!(
                    "Split Isocurve=4,6,0 Direction=Both Shrink={}",
                    if shrink { "Yes" } else { "No" }
                ),
            )
            .unwrap();
        assert!(document.object(source_id).is_none());
        assert_eq!(document.objects().count(), 4);
        for (object, expected) in document.objects().zip(expected_bounds) {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("re-splitting a rectangular face must create B-rep faces")
            };
            let face = &brep.faces()[0];
            assert!(face.is_reversed());
            assert_eq!(
                face.rectangular_trim_bounds(document.tolerance()).unwrap(),
                Some(expected)
            );
            if shrink {
                assert_eq!(
                    [
                        [
                            *face.surface().domain_u().start(),
                            *face.surface().domain_u().end()
                        ],
                        [
                            *face.surface().domain_v().start(),
                            *face.surface().domain_v().end()
                        ],
                    ],
                    expected
                );
            } else {
                assert_eq!(face.surface(), &source_surface);
            }
            assert!(document.is_selected(object.id()));
        }
    }

    let mut outside = Document::default();
    let source_id = outside
        .add_geometry(Geometry::Brep(
            Brep::try_rectangular_surface_face(
                source_surface,
                0.2..=0.8,
                0.1..=0.9,
                outside.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    outside
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    let before = outside.object(source_id).unwrap().clone();
    let history = outside.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut outside, "Split Isocurve=9,5,0 Direction=U Shrink=No"),
        Err(CommandError::SplitIsocurvePointOutsideFaceTrim)
    ));
    assert_eq!(outside.object(source_id).unwrap(), &before);
    assert_eq!(outside.undo_label(), history.as_deref());
}

#[test]
fn split_surface_at_isocurve_preserves_closed_seams_and_singular_poles() {
    let registry = CommandRegistry::with_builtins();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();

    let mut cylinder_document = Document::default();
    let cylinder_id = cylinder_document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_cylinder(frame, 2.0, 0.0, 2.0).unwrap(),
        ))
        .unwrap();
    cylinder_document
        .select_object(cylinder_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut cylinder_document,
            "Split Isocurve=0,2,1 Direction=U Shrink=Yes",
        )
        .unwrap();
    assert_eq!(cylinder_document.objects().count(), 2);
    for object in cylinder_document.objects() {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("closed-surface Split must create B-rep faces")
        };
        assert_eq!(brep.vertices().len(), 2);
        assert_eq!(brep.edges().len(), 3);
        let trims = brep.faces()[0].loops()[0].trims();
        assert_eq!(
            trims
                .iter()
                .filter(|trim| trim.trim_type() == BrepTrimType::Seam)
                .count(),
            2
        );
        assert_eq!(trims[1].edge(), trims[3].edge());
        assert!(!trims[1].is_reversed_3d());
        assert!(trims[3].is_reversed_3d());
    }

    let mut sphere_document = Document::default();
    let sphere = NurbsSurface::try_sphere(frame, 2.0).unwrap();
    let sphere_id = sphere_document
        .add_geometry(Geometry::NurbsSurface(sphere.clone()))
        .unwrap();
    sphere_document
        .select_object(sphere_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut sphere_document,
            "Split Isocurve=2,0,0 Direction=U Shrink=No",
        )
        .unwrap();
    assert_eq!(sphere_document.objects().count(), 2);
    for object in sphere_document.objects() {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("singular-surface Split must create B-rep faces")
        };
        assert_eq!(brep.vertices().len(), 2);
        assert_eq!(brep.edges().len(), 2);
        assert_eq!(brep.faces()[0].surface(), &sphere);
        let trims = brep.faces()[0].loops()[0].trims();
        assert_eq!(
            trims
                .iter()
                .filter(|trim| trim.trim_type() == BrepTrimType::Singular)
                .count(),
            1
        );
        assert_eq!(
            trims
                .iter()
                .filter(|trim| trim.trim_type() == BrepTrimType::Seam)
                .count(),
            2
        );
    }
}

#[test]
fn split_surface_at_isocurve_rejects_invalid_requests_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Split Isocurve",
        "Split Isocurve=1,2,3 Direction=W",
        "Split Isocurve=1,2,3 Direction=U Direction=V",
        "Split Isocurve=1,2,3 Shrink=Maybe",
    ] {
        assert!(matches!(
            registry.execute(&mut document, command),
            Err(CommandError::Usage(SPLIT_CURVE_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut document, "Split Isocurve=1,2,3"),
        Err(CommandError::SplitIsocurveRequiresOneSurface { actual: 0 })
    ));

    let source_id = document
        .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
        .unwrap();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "Line 0,0 1,1").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut document, "Split Isocurve=4,6,0"),
        Err(CommandError::SplitIsocurveRequiresOneSurface { actual: 1 })
    ));
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}
