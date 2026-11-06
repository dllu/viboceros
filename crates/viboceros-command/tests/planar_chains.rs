use viboceros_command::{CommandError, CommandRegistry};
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, SelectionMode};
use viboceros_geometry::{GeometryError, NurbsCurve, Point3, Polyline3, Tolerance};
fn polyline(points: &[[f64; 3]]) -> Geometry {
    Geometry::NurbsCurve(
        Polyline3::try_new(
            points
                .iter()
                .map(|&p| Point3::try_from(p).unwrap())
                .collect(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap()
        .to_native_nurbs()
        .unwrap(),
    )
}
#[test]
fn planar_chains_delete_only_boundary_contributors_and_preserve_attributes_groups_and_history() {
    for delete in [false, true] {
        let mut doc = Document::default();
        let commands = CommandRegistry::with_builtins();
        let layer = doc
            .add_layer("Sources", ColorRgb::new(20, 80, 160))
            .unwrap();
        let current = doc.current_layer_id();
        let p = [
            [0., 0., 0.],
            [10., 0., 0.],
            [10., 10., 0.],
            [0., 10., 0.],
            [0., 0., 0.],
        ];
        let mut curves = p.windows(2).map(polyline).collect::<Vec<_>>();
        curves.push(polyline(&[[8., 0., 0.], [2., 0., 0.]])); // partial reversed alias
        curves.push(polyline(&[[0., 0., 0.], [-2., -2., 0.]])); // point contact
        curves.push(polyline(&[[0., 0., 0.], [10., 10., 0.]])); // unused diagonal
        let ids = curves
            .into_iter()
            .map(|g| {
                doc.add_geometry_with_attributes(
                    g,
                    ObjectAttributes::on_layer(layer).with_name("Original"),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let group = doc.add_group(None, ids.iter().copied()).unwrap();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        commands
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
        assert_eq!(outputs.len(), 1);
        let output = outputs[0];
        assert_eq!(output.attributes(), &ObjectAttributes::on_layer(current));
        assert!(!doc.is_selected(output.id()));
        let Geometry::Brep(b) = output.geometry() else {
            panic!("B-rep")
        };
        assert!((b.area(doc.tolerance()).unwrap() - 100.).abs() < 1e-9);
        for (i, id) in ids.iter().enumerate() {
            if delete && i < 5 {
                assert!(doc.object(*id).is_none());
            } else {
                assert_eq!(doc.object(*id), Some(&before[i]));
                assert!(doc.is_selected(*id));
            }
        }
        assert_eq!(
            doc.group(group).unwrap().members().len(),
            if delete { 2 } else { 7 }
        );
        let after = doc.objects().cloned().collect::<Vec<_>>();
        assert_eq!(doc.undo_label(), Some("PlanarSrf"));
        commands.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.group(group).unwrap().members().len(), 7);
        commands.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    }
}
#[test]
fn planar_chains_control_budget_failure_is_atomic_and_preserves_redo() {
    let mut doc = Document::default();
    let commands = CommandRegistry::with_builtins();
    let count = 100_001;
    let curve = NurbsCurve::try_new(
        1,
        (0..count)
            .map(|i| Point3::try_new(i as f64, (i % 2) as f64, 0.).unwrap())
            .collect(),
        std::iter::once(0.)
            .chain((0..count).map(|i| i as f64))
            .chain(std::iter::once((count - 1) as f64))
            .collect(),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    let group = doc.add_group(None, [id]).unwrap();
    commands.execute(&mut doc, "Point 0,0,1").unwrap();
    commands.execute(&mut doc, "Undo").unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let undo = doc.undo_label().map(str::to_owned);
    let error = commands
        .execute(&mut doc, "PlanarSrf DeleteInput=Yes")
        .unwrap_err();
    assert!(
        matches!(
            error,
            CommandError::Geometry(GeometryError::CurveJoinLimit {
                resource: "planar boundary controls",
                maximum: 100_000
            })
        ),
        "{error:?}"
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.undo_label(), undo.as_deref());
    assert_eq!(doc.redo_label(), Some("Point"));
    assert!(doc.is_selected(id));
    assert_eq!(doc.group(group).unwrap().members().len(), 1);
}
