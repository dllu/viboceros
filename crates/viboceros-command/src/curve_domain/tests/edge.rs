use super::*;

fn parent(document: &mut Document) -> (ObjectId, usize) {
    let brep = Brep::try_box(
        CommandContext::default().construction_plane,
        [[0., 4.], [0., 6.], [0., 2.]],
        document.tolerance(),
    )
    .unwrap();
    let edge = brep
        .edges()
        .iter()
        .position(|e| {
            let c = e.curve();
            c.evaluate(*c.domain().start()).unwrap() == Point3::try_new(0., 0., 0.).unwrap()
                && c.evaluate(*c.domain().end()).unwrap() == Point3::try_new(4., 0., 0.).unwrap()
        })
        .unwrap();
    (document.add_geometry(Geometry::Brep(brep)).unwrap(), edge)
}

#[test]
fn subcurve_edge_forces_copy_and_preserves_parent_metadata_without_parent_selection() {
    for copy in ["Yes", "No"] {
        let registry = CommandRegistry::with_builtins();
        let mut d = Document::default();
        let (id, edge) = parent(&mut d);
        d.set_object_names([(id, Some("parent".into()))]).unwrap();
        d.set_object_user_text([id], "source", Some("original"))
            .unwrap();
        let group = d.add_group(Some("parent".into()), [id]).unwrap();
        let before = d.object(id).unwrap().clone();
        d.clear_history().unwrap();
        registry
            .execute(
                &mut d,
                &format!("SubCrv 1,0 3,0 Edge={id},{edge} Copy={copy}"),
            )
            .unwrap();
        assert_eq!(d.object(id).unwrap(), &before);
        let result = d.objects().last().unwrap();
        assert_eq!(result.attributes(), before.attributes());
        assert_eq!(result.group_ids(), [group]);
        assert!(d.is_selected(result.id()));
        assert!(!d.is_selected(id));
        assert!(
            (result
                .geometry()
                .curve_ref()
                .unwrap()
                .length(d.tolerance())
                .unwrap()
                - 2.)
                .abs()
                < 1e-9
        );
        assert_eq!(registry.subcurve_defaults().copy, copy == "Yes");
        let results = d.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut d, "Undo").unwrap();
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), [before]);
        registry.execute(&mut d, "Redo").unwrap();
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), results);
    }
}

#[test]
fn subcurve_edge_reference_failures_are_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let (id, edge) = parent(&mut d);
    d.clear_history().unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    for tail in [
        "Edge=bad,0".into(),
        format!("Edge={id},-1"),
        format!("Edge={id},999999"),
        format!("Edge={id},{edge},0"),
        format!("Edge={id},{edge} Edge={id},{edge}"),
        "Edge=999999,0".into(),
    ] {
        assert!(
            registry
                .execute(&mut d, &format!("SubCrv 1,0 3,0 {tail}"))
                .is_err(),
            "{tail}"
        );
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!d.can_undo());
    }
    let point = d
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    d.clear_history().unwrap();
    assert!(
        registry
            .execute(&mut d, &format!("SubCrv 1,0 3,0 Edge={point},0"))
            .is_err()
    );
    assert!(!d.can_undo());
}

#[test]
fn subcurve_edge_numeric_and_explicit_parameter_inputs_use_the_edge_domain() {
    for numeric in [false, true] {
        let registry = CommandRegistry::with_builtins();
        let mut d = Document::default();
        let (id, index) = parent(&mut d);
        let resolved = crate::curve_reference::resolve(
            d.object(id).unwrap().geometry(),
            Some(index),
            d.tolerance(),
        )
        .unwrap();
        let curve = resolved.curve();
        let start = curve
            .closest_parameter(Point3::try_new(1., 0., 0.).unwrap(), d.tolerance())
            .unwrap();
        let end = curve
            .closest_parameter(Point3::try_new(3., 0., 0.).unwrap(), d.tolerance())
            .unwrap();
        let location = if numeric {
            format!("Numeric={start},2,{end}")
        } else {
            format!("Parameter={start},{end}")
        };
        registry
            .execute(
                &mut d,
                &format!("SubCrv {location} Edge={id},{index} Copy=No"),
            )
            .unwrap();
        assert_eq!(d.objects().len(), 2);
        let result = d.objects().last().unwrap().geometry().curve_ref().unwrap();
        assert!(
            result
                .start_point()
                .unwrap()
                .distance_to(Point3::try_new(1., 0., 0.).unwrap())
                .unwrap()
                < 1e-9
        );
        assert!(
            result
                .end_point()
                .unwrap()
                .distance_to(Point3::try_new(3., 0., 0.).unwrap())
                .unwrap()
                < 1e-9
        );
    }
}
