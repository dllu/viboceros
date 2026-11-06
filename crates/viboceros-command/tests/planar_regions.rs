use viboceros_command::CommandRegistry;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, SelectionMode};
use viboceros_geometry::{LineSegment, NurbsCurve, Point3};
fn rectangle(a: f64, b: f64, c: f64, d: f64) -> Geometry {
    Geometry::NurbsCurve(
        NurbsCurve::try_new(
            1,
            [[a, b, 0.], [c, b, 0.], [c, d, 0.], [a, d, 0.], [a, b, 0.]]
                .into_iter()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            vec![0., 0., 1., 2., 3., 4., 4.],
        )
        .unwrap(),
    )
}
#[test]
fn planar_region_duplicates_delete_all_valid_sources_in_one_history_step() {
    for delete in [false, true] {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let layer = doc.add_layer("Input", ColorRgb::new(37, 83, 149)).unwrap();
        let output_layer = doc.current_layer_id();
        let a = rectangle(2., 2., 6., 6.);
        let mut geometries = vec![
            rectangle(0., 0., 10., 10.),
            a.clone(),
            a,
            rectangle(4., 4., 8., 8.),
        ];
        geometries.push(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(20., 0., 0.).unwrap(),
                Point3::try_new(21., 0., 0.).unwrap(),
                doc.tolerance(),
            )
            .unwrap(),
        ));
        let ids = geometries
            .into_iter()
            .map(|g| {
                doc.add_geometry_with_attributes(
                    g,
                    ObjectAttributes::on_layer(layer).with_name("Source"),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let group = doc
            .add_group(Some("Originals".into()), ids.iter().copied())
            .unwrap();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(
                &mut doc,
                if delete {
                    "PlanarSrf DeleteInput=Yes"
                } else {
                    "PlanarSrf"
                },
            )
            .unwrap();
        let outputs = doc
            .objects()
            .filter(|o| !ids.contains(&o.id()))
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        for o in &outputs {
            assert_eq!(o.attributes(), &ObjectAttributes::on_layer(output_layer));
            assert!(!doc.is_selected(o.id()));
        }
        let mut areas = outputs
            .iter()
            .map(|o| {
                let Geometry::Brep(b) = o.geometry() else {
                    panic!("B-rep")
                };
                b.area(doc.tolerance()).unwrap()
            })
            .collect::<Vec<_>>();
        areas.sort_by(f64::total_cmp);
        assert!((areas[0] - 16.).abs() < 1e-10);
        assert!((areas[1] - 84.).abs() < 1e-10);
        for (i, id) in ids.iter().enumerate() {
            if delete && i < 4 {
                assert!(doc.object(*id).is_none());
            } else {
                assert_eq!(doc.object(*id).unwrap(), &before[i]);
                assert!(doc.is_selected(*id));
            }
        }
        assert_eq!(
            doc.group(group).unwrap().members().len(),
            if delete { 1 } else { 5 }
        );
        assert_eq!(doc.undo_label(), Some("PlanarSrf"));
        let after = doc.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.group(group).unwrap().members().len(), 5);
        assert_eq!(doc.undo_label(), None);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    }
}
#[test]
fn planar_region_budget_declines_before_editing_the_model() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let ids = (0..129)
        .map(|i| {
            doc.add_geometry(rectangle(i as f64 * 20., 0., i as f64 * 20. + 10., 10.))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let group = doc
        .add_group(Some("Originals".into()), ids.iter().copied())
        .unwrap();
    registry.execute(&mut doc, "Point 4000,0,0").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let label = doc.undo_label().map(str::to_string);
    assert!(
        registry
            .execute(&mut doc, "PlanarSrf DeleteInput=Yes")
            .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.undo_label(), label.as_deref());
    assert_eq!(doc.redo_label(), Some("Point"));
    assert_eq!(doc.group(group).unwrap().members().len(), 129);
    assert!(ids.iter().all(|id| doc.is_selected(*id)));
}
