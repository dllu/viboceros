use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn patch(bump: bool) -> Geometry {
    Geometry::NurbsSurface(
        NurbsSurface::try_new(
            2,
            2,
            3,
            3,
            (0..3)
                .flat_map(|j| {
                    (0..3).map(move |i| {
                        p(
                            i as f64 * 3. - 3.,
                            j as f64 * 3. - 3.,
                            if bump && i == 1 && j == 1 { 1. } else { 0. },
                        )
                    })
                })
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap(),
    )
}
#[test]
fn surface_taper_options_and_face_selectors_are_strict() {
    let input =
        SurfaceTaperedInput::parse(&["DraftAngle=10", "Solid=No", "Face=0", "FlipAngle"]).unwrap();
    assert_eq!(input.options.draft_angle, -10.);
    assert!(!input.options.solid);
    assert_eq!(input.faces, SurfaceFaces::Indices(vec![0]));
    assert!(SurfaceTaperedInput::parse(&["DraftAngle=90"]).is_err());
    assert!(SurfaceTaperedInput::parse(&["Face=0", "Faces=All"]).is_err());
    assert!(SurfaceTaperedInput::default().options.solid);
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn surface_taper_portable_decline_is_atomic() {
    let mut doc = Document::default();
    let s = doc.add_geometry(patch(true)).unwrap();
    doc.select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    assert!(matches!(
        CommandRegistry::with_builtins().execute(&mut doc, "ExtrudeSrfTapered 2 DeleteInput=Yes"),
        Err(CommandError::SurfaceTaperedExtrusionRequiresNativeKernel)
    ));
    assert_eq!(format!("{doc:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_taper_curved_base_delete_history_and_explicit_displacement() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let s = doc.add_geometry(patch(true)).unwrap();
    let original = doc.object(s).unwrap().clone();
    doc.select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut doc,
            "ExtrudeSrfTapered 0,0,0 0,0,2 DraftAngle=10 DeleteInput=Yes",
        )
        .unwrap();
    assert!(doc.object(s).is_none());
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(b.is_solid());
    let mut max_normalized_uv_gap = 0_f64;
    for face in b.faces() {
        let s = face.surface();
        let du = s.domain_u().end() - s.domain_u().start();
        let dv = s.domain_v().end() - s.domain_v().start();
        for boundary in face.loops() {
            let trims = boundary.trims();
            for (i, t) in trims.iter().enumerate() {
                let c = t.curve();
                let n = trims[(i + 1) % trims.len()].curve();
                let a = c.evaluate(*c.domain().end()).unwrap();
                let q = n.evaluate(*n.domain().start()).unwrap();
                max_normalized_uv_gap = max_normalized_uv_gap
                    .max((a.x() - q.x()).abs() / du)
                    .max((a.y() - q.y()).abs() / dv);
            }
        }
    }
    eprintln!(
        "normalized UV endpoint gap: {max_normalized_uv_gap:e}; pure certified bounds: {:?}",
        b.tight_bounds(doc.tolerance()).map(|_| ())
    );
    assert!(max_normalized_uv_gap <= 1e-12);

    let k = 10_f64.to_radians().tan();
    assert!(
        (b.signed_volume(doc.tolerance()).unwrap().abs() - (68. - 48. * k + 32. / 3. * k * k))
            .abs()
            < 1e-4
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(s), Some(&original));
    registry.execute(&mut doc, "Redo").unwrap();
    assert!(doc.object(s).is_none());
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_taper_later_oblique_face_preserves_batch_and_redo() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let a = doc.add_geometry(patch(false)).unwrap();
    let b = doc
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![p(0., 0., 0.), p(0., 6., 0.), p(0., 0., 6.), p(0., 6., 6.)],
                vec![0., 0., 1., 1.],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    registry.execute(&mut doc, "Point 50,50,50").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    assert!(registry.execute(&mut doc,&format!("ExtrudeSrfTapered 2 Direction=0,0,1 DraftAngle=10 DeleteInput=Yes FaceRefs={a}:0,{b}:0")).is_err());
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_taper_deleting_a_face_keeps_remainder_groups_and_zero_draft() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let s = doc
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                CommandContext::default().construction_plane,
                [[0., 6.], [0., 6.], [0., 6.]],
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let group = doc.add_group(None, [s]).unwrap();
    let original = doc.object(s).unwrap().clone();
    registry
        .execute(
            &mut doc,
            &format!("ExtrudeSrfTapered 2 FaceRefs={s}:0,{s}:0 DraftAngle=0 DeleteInput=Yes"),
        )
        .unwrap();
    let remaining = doc.object(s).unwrap();
    assert_eq!(remaining.attributes(), original.attributes());
    assert!(matches!(remaining.geometry(),Geometry::Brep(b)if b.faces().len()==5));
    assert_eq!(
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![s]
    );
    assert_eq!(doc.selected_object_count(), 1);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(s), Some(&original));
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_taper_numeric_direction_uses_the_face_plane_independently_of_cplane() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let s = doc
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![p(0., 0., 0.), p(0., 6., 0.), p(0., 0., 6.), p(0., 6., 6.)],
                vec![0., 0., 1., 1.],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    registry
        .execute(
            &mut doc,
            &format!("ExtrudeSrfTapered 2 DraftAngle=10 FaceRefs={s}:0"),
        )
        .unwrap();
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(b.is_solid());
    assert!((b.bounds().max().x() - 2.).abs() < 1e-7);
    let k = 10_f64.to_radians().tan();
    assert!(
        (b.signed_volume(doc.tolerance()).unwrap().abs() - (72. - 48. * k + 32. / 3. * k * k))
            .abs()
            < 1e-4
    );
}
