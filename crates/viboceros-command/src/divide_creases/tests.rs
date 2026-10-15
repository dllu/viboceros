use super::*;
fn surface() -> NurbsSurface {
    NurbsSurface::try_new(
        1,
        1,
        3,
        2,
        [0., 2.]
            .into_iter()
            .flat_map(|y| {
                [(0., 0.), (1., 0.), (1., 1.)]
                    .into_iter()
                    .map(move |(x, z)| Point3::try_new(x, y, z).unwrap())
            })
            .collect(),
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
#[test]
fn division_options_are_strict_typed_remembered_and_independent_between_registries() {
    let r = CommandRegistry::with_builtins();
    let prompt = r
        .object_selection_prompt("DivideAlongCreases")
        .unwrap()
        .unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::SurfaceComponents);
    assert_eq!(
        prompt.workflow,
        ObjectSelectionWorkflow::OptionsDuringSelection
    );
    assert!(prompt.options.iter().all(|o| o.value));
    let mut d = Document::default();
    let id = d.add_geometry(Geometry::NurbsSurface(surface())).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    d.add_geometry(Geometry::Point(Point3::try_new(8., 9., 10.).unwrap()))
        .unwrap();
    d.undo().unwrap();
    let original = format!("{d:?}");
    assert_eq!(
        r.execute(
            &mut d,
            "DivideAlongCreases _SplitAtKinks _No SplitAtTangents=No"
        )
        .unwrap(),
        "Added 0 face(s) in 0 object(s)"
    );
    assert_eq!(format!("{d:?}"), original);
    assert!(d.can_redo());
    assert!(
        r.object_selection_prompt("DivideAlongCreases")
            .unwrap()
            .unwrap()
            .options
            .iter()
            .all(|o| !o.value)
    );
    for input in [
        "DivideAlongCreases Bad=Yes",
        "DivideAlongCreases SplitAtKinks=True",
        "DivideAlongCreases SplitAtKinks=No SplitAtKinks=Yes",
        "DivideAlongCreases SplitAtTangents",
        "DivideAlongCreases 1",
    ] {
        assert!(r.execute(&mut d, input).is_err(), "{input}");
        assert_eq!(format!("{d:?}"), original);
    }
    assert!(
        CommandRegistry::with_builtins()
            .object_selection_prompt("DivideAlongCreases")
            .unwrap()
            .unwrap()
            .options
            .iter()
            .all(|o| o.value)
    );
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn division_requires_native_kernel_without_changing_sources_or_history() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let id = d.add_geometry(Geometry::NurbsSurface(surface())).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(
        r.execute(&mut d, "DivideAlongCreases")
            .unwrap_err()
            .to_string()
            .contains("native-smlib")
    );
    assert_eq!(format!("{d:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn division_retains_identity_groups_and_attributes_in_one_reversible_edit() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let id = d
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(surface()),
            ObjectAttributes::on_layer(d.current_layer_id()).with_name("fold"),
        )
        .unwrap();
    let smooth = d
        .add_geometry(Geometry::NurbsSurface(
            surface().try_trimmed_u(0.0..=0.75).unwrap(),
        ))
        .unwrap();
    let group = d.add_group(None, [id, smooth]).unwrap();
    d.select_objects_direct([id, smooth], SelectionMode::Replace)
        .unwrap();
    let originals = d.objects().cloned().collect::<Vec<_>>();
    assert_eq!(
        r.execute(&mut d, "DivideAlongCreases").unwrap(),
        "Added 1 face(s) in 1 object(s)"
    );
    assert_eq!(
        d.object(id).unwrap().attributes(),
        originals[0].attributes()
    );
    assert_eq!(d.object(smooth), Some(&originals[1]));
    assert!(d.is_selected(id) && d.is_selected(smooth));
    assert_eq!(
        d.group(group)
            .unwrap()
            .members()
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([id, smooth])
    );
    assert!(
        matches!(d.object(id).unwrap().geometry(),Geometry::Brep(b) if b.faces().len()==2 && (b.area(d.tolerance()).unwrap()-4.).abs()<1e-7)
    );
    assert_eq!(
        geometry_check::check_geometry(d.object(id).unwrap().geometry()).status,
        geometry_check::GeometryCheckStatus::Valid
    );
    let output = d.objects().cloned().collect::<Vec<_>>();
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), originals);
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), output);
    r.execute(&mut d, "Undo").unwrap();
    let snapshot = CreaseDivisionSelection::prepare(&d, CreaseDivisionOptions::default()).unwrap();
    d.set_tolerance(Tolerance::try_new(1e-7, 1e-12, 1e-8).unwrap());
    let before = format!("{d:?}");
    assert!(snapshot.commit(&mut d).is_err());
    assert_eq!(format!("{d:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn division_staging_failure_and_noop_preserve_redo_and_all_sources() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let id = d.add_geometry(Geometry::NurbsSurface(surface())).unwrap();
    let unsupported = d
        .add_geometry(Geometry::Point(Point3::try_new(8., 9., 10.).unwrap()))
        .unwrap();
    d.add_geometry(Geometry::Point(Point3::try_new(4., 5., 6.).unwrap()))
        .unwrap();
    d.undo().unwrap();
    d.select_objects_direct([id, unsupported], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(r.execute(&mut d, "DivideAlongCreases").is_err());
    assert_eq!(format!("{d:?}"), before);
    assert!(d.can_redo());
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let prepared = CreaseDivisionSelection::prepare(&d, CreaseDivisionOptions::default()).unwrap();
    d.replace_object_geometries([(
        id,
        Geometry::NurbsSurface(surface().try_swapped_uv().unwrap()),
    )])
    .unwrap();
    let before = format!("{d:?}");
    assert!(prepared.commit(&mut d).is_err());
    assert_eq!(format!("{d:?}"), before);
}
