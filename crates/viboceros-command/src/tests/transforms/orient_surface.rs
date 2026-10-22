//! transforms orient surface command regressions.
use super::*;

fn orient_surface_quarter_cylinder() -> NurbsSurface {
    let middle_weight = 0.5_f64.sqrt();
    let mut controls = Vec::new();
    for z in [0.0, 10.0] {
        controls.extend([
            WeightedPoint3::try_new(Point3::try_new(10.0, 0.0, z).unwrap(), 1.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(10.0, 10.0, z).unwrap(), middle_weight)
                .unwrap(),
            WeightedPoint3::try_new(Point3::try_new(0.0, 10.0, z).unwrap(), 1.0).unwrap(),
        ]);
    }
    NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        controls,
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap()
}

fn oriented_surface_curve(document: &Document, name: &str, original: ObjectId) -> NurbsCurve {
    document
        .objects()
        .find(|object| object.id() != original && object.attributes().name() == Some(name))
        .map(|object| match object.geometry() {
            Geometry::NurbsCurve(curve) => curve.clone(),
            _ => panic!("expected a deformable surface-orient curve"),
        })
        .unwrap()
}

#[test]
fn orient_surface_named_copy_does_not_override_the_saved_copy_mode() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    add_orient_triad(&mut document);
    add_named_surface(&mut document, orient_surface_quarter_cylinder(), "Copy");
    registry
            .execute(&mut document,
                "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Copy=No Rigid=Yes SurfaceName Copy")
            .unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    registry
            .execute(&mut document,
                "OrientOnSurface 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Rigid=Yes SurfaceName Copy")
            .unwrap();
    assert_eq!(document.objects().len(), 4);
    assert_eq!(registry.copy_default("OrientOnSrf"), Some(false));
}

#[test]
fn orient_on_surface_deformable_matches_rhino_splop_groups_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let originals = add_orient_triad(&mut document);
    let target = add_named_surface(&mut document, orient_surface_quarter_cylinder(), "Target");
    let message = registry
        .execute(
            &mut document,
            "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 \
                 Copy=Yes Rigid=No SurfaceName=target",
        )
        .unwrap();
    assert!(message.contains("deformable"));
    assert!(message.contains("creating 3 copy object(s)"));
    assert_eq!(document.objects().len(), 7);
    assert_eq!(document.groups().len(), 2);
    assert_eq!(document.undo_label(), Some("OrientOnSrf"));
    assert!(!document.is_selected(target));
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        originals.into_iter().collect()
    );
    let expected_ends = [
        Point3::try_new(8.484425274005353, 5.292875189329445, 4.0).unwrap(),
        Point3::try_new(8.973756499953724, 4.412674277525845, 5.0).unwrap(),
        Point3::try_new(9.8711321499491, 4.85394170527843, 4.0).unwrap(),
    ];
    let base = Point3::try_new(8.973756499953726, 4.412674277525846, 4.0).unwrap();
    let copy_ids = document
        .objects()
        .filter(|object| !originals.contains(&object.id()) && object.id() != target)
        .map(|object| object.id())
        .collect::<Vec<_>>();
    for (((name, original), expected_end), copy_id) in ["x", "y", "z"]
        .into_iter()
        .zip(originals)
        .zip(expected_ends)
        .zip(copy_ids.iter().copied())
    {
        let curve = oriented_surface_curve(&document, name, original);
        assert_eq!(curve.degree(), 3);
        let Geometry::Line(source_line) = document.object(original).unwrap().geometry() else {
            panic!("expected original line");
        };
        let surface = orient_surface_quarter_cylinder();
        let frame = Frame3::try_from_directions(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            document.tolerance(),
        )
        .unwrap();
        let morph = SurfacePointMorph::try_new(
            frame,
            &surface,
            0.3,
            0.4,
            1.0,
            0.0,
            false,
            document.tolerance(),
        )
        .unwrap();
        for i in 0..=256 {
            let fraction = i as f64 / 256.0;
            let expected = viboceros_geometry::PointMorph::morph_point(
                &morph,
                source_line.point_at(fraction).unwrap(),
            )
            .unwrap();
            let actual = curve
                .evaluate(curve.parameter_at(fraction).unwrap())
                .unwrap();
            assert!(actual.distance_to(expected).unwrap() <= document.tolerance().absolute());
        }
        assert!(
            curve
                .evaluate(*curve.domain().start())
                .unwrap()
                .is_near(base, document.tolerance())
        );
        assert!(
            curve
                .evaluate(*curve.domain().end())
                .unwrap()
                .is_near(expected_end, document.tolerance())
        );
        assert!(!document.is_selected(copy_id));
    }

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 4);
    assert_eq!(document.groups().len(), 1);
    registry.execute(&mut document, "Redo").unwrap();
    assert!(copy_ids.iter().all(|id| document.object(*id).is_some()));
    assert_eq!(document.groups().len(), 2);
}

#[test]
fn orient_on_surface_fits_surfaces_and_breps_and_restores_copy_and_in_place_history() {
    use viboceros_geometry::PointMorph;
    let registry = CommandRegistry::with_builtins();
    let source = NurbsSurface::try_bilinear([
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Point3::try_new(2.0, 2.0, 3.0).unwrap(),
        Point3::try_new(2.0, 3.0, 3.2).unwrap(),
        Point3::try_new(1.0, 3.0, 3.0).unwrap(),
    ])
    .unwrap();
    let surface = orient_surface_quarter_cylinder();
    for (copy, as_brep) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut document = Document::default();
        let geometry = if as_brep {
            Geometry::Brep(Brep::try_surface_face(source.clone(), document.tolerance()).unwrap())
        } else {
            Geometry::NurbsSurface(source.clone())
        };
        let original = document
            .add_geometry_with_attributes(
                geometry,
                ObjectAttributes::on_layer(document.current_layer_id()).with_name("Source"),
            )
            .unwrap();
        let target = add_named_surface(&mut document, surface.clone(), "Target");
        document
            .add_group(Some("Shared".into()), [original])
            .unwrap();
        document
            .select_objects_direct([original], SelectionMode::Replace)
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(
                &mut document,
                &format!(
                    "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 \
                 Copy={} Rigid=No SurfaceName=Target",
                    if copy { "Yes" } else { "No" },
                ),
            )
            .unwrap();
        assert_eq!(document.objects().len(), if copy { 3 } else { 2 });
        assert_eq!(document.groups().len(), if copy { 2 } else { 1 });
        assert_eq!(document.undo_label(), Some("OrientOnSrf"));
        assert!(document.is_selected(original));
        assert!(!document.is_selected(target));
        let result = document
            .objects()
            .find(|object| object.id() != target && (!copy || object.id() != original))
            .unwrap();
        let fitted = match (result.geometry(), as_brep) {
            (Geometry::NurbsSurface(surface), false) => surface,
            (Geometry::Brep(brep), true) => {
                assert_eq!(brep.faces().len(), 1);
                assert_eq!(brep.edges().len(), 4);
                brep.faces()[0].surface()
            }
            _ => panic!("surface morph changed the source representation"),
        };
        assert_eq!(fitted.domain_u(), source.domain_u());
        assert_eq!(fitted.domain_v(), source.domain_v());
        let frame = Frame3::try_from_directions(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            document.tolerance(),
        )
        .unwrap();
        let morph = SurfacePointMorph::try_new(
            frame,
            &surface,
            0.3,
            0.4,
            1.0,
            0.0,
            false,
            document.tolerance(),
        )
        .unwrap();
        let fraction = |i| match i {
            0 => 0.0,
            32 => 1.0,
            _ => (i as f64 - 0.3819660112501051) / 32.0,
        };
        for j in 0..=32 {
            for i in 0..=32 {
                let u = source.parameter_at_u(fraction(i)).unwrap();
                let v = source.parameter_at_v(fraction(j)).unwrap();
                let expected = morph.morph_point(source.evaluate(u, v).unwrap()).unwrap();
                let actual = fitted.evaluate(u, v).unwrap();
                assert!(actual.distance_to(expected).unwrap() <= document.tolerance().absolute());
            }
        }
        let after = document.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.groups().len(), 1);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
        assert_eq!(document.groups().len(), if copy { 2 } else { 1 });
    }
}

#[test]
fn orient_on_surface_rigid_flip_constrained_normal_and_errors_are_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let originals = add_orient_triad(&mut document);
    let target = add_named_surface(&mut document, orient_surface_quarter_cylinder(), "Target");
    registry
        .execute(
            &mut document,
            "OrientOnSurface 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 \
                 Scale=2 Rotation=90 Copy=Yes Rigid=Yes SurfaceName=Target",
        )
        .unwrap();
    let base = Point3::try_new(8.973756499953726, 4.412674277525846, 4.0).unwrap();
    let expected = [
        Point3::try_new(base.x(), base.y(), 6.0).unwrap(),
        Point3::try_new(9.856291355458895, 2.6179229775351006, 4.0).unwrap(),
        Point3::try_new(10.76850779994447, 5.295209133030891, 4.0).unwrap(),
    ];
    for ((name, expected_end), original) in ["x", "y", "z"].into_iter().zip(expected).zip(originals)
    {
        let copy = orient_line(&document, name, &[original]);
        assert!(copy.start().is_near(base, document.tolerance()));
        assert!(copy.end().is_near(expected_end, document.tolerance()));
    }
    registry.execute(&mut document, "Undo").unwrap();

    let before = document.objects().cloned().collect::<Vec<_>>();
    assert!(registry.execute(&mut document,
            "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Scale=1 Rotation=0 Copy=No Rigid=No ConstrainNormal=Yes SurfaceName=Target"
        ).is_err());
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    registry.execute(&mut document,
            "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Scale=1 Rotation=0 Copy=No Rigid=No ConstrainNormal=Yes DestinationNormal=1,0,0 SurfaceName=Target"
        ).unwrap();
    let Geometry::NurbsCurve(z) = document.object(originals[2]).unwrap().geometry() else {
        panic!("expected a fitted curve")
    };
    assert!(z.evaluate(*z.domain().end()).unwrap().is_near(
        Point3::try_new(base.x() + 1. / 0.8973756499953726, base.y(), base.z()).unwrap(),
        document.tolerance()
    ));
    registry.execute(&mut document, "Undo").unwrap();

    registry
            .execute(
                &mut document,
                "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 \
                 Scale=1 Rotation=0 ConstrainNormal=No Copy=No Rigid=No Flip=Yes SurfaceName=Target",
            )
            .unwrap();
    assert_eq!(document.objects().len(), 4);
    assert_eq!(document.groups().len(), 1);
    let y = match document.object(originals[1]).unwrap().geometry() {
        Geometry::NurbsCurve(curve) => curve,
        _ => panic!("expected an in-place surface morph"),
    };
    assert!(y.evaluate(*y.domain().end()).unwrap().is_near(
        Point3::try_new(base.x(), base.y(), 3.0).unwrap(),
        document.tolerance()
    ));
    registry.execute(&mut document, "Undo").unwrap();

    let object_count = document.objects().len();
    let group_count = document.groups().len();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "OrientOnSrf 1,2,3 1,2,3 8.973756499953726,4.412674277525846,4 SurfaceName=Target",
        "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Scale=0 SurfaceName=Target",
        "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Scale=-1 SurfaceName=Target",
        "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 SourceNormal=1,0,0 SurfaceName=Target",
        "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 ConstrainNormal=0,0,1 SurfaceName=Target",
        "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Rigid=Yes ConstrainNormal=Yes SurfaceName=Target",
        "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 SurfaceName=Missing",
        "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Copy=Yes Copy=No SurfaceName=Target",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
    }
    assert_eq!(document.objects().len(), object_count);
    assert_eq!(document.groups().len(), group_count);
    assert_eq!(document.undo_label(), history.as_deref());
    assert!(!document.is_selected(target));
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        originals.into_iter().collect()
    );
}
