use super::*;

#[test]
fn point_grid_commands_preserve_order_selection_attributes_and_atomic_history() {
    let registry = CommandRegistry::with_builtins();
    for name in ["SrfPtGrid", "SrfControlPtGrid"] {
        let mut document = Document::default();
        registry.execute(&mut document, "Point 93,97,101").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, "Group Existing").unwrap();
        registry.execute(&mut document, "Layer New Output").unwrap();
        registry
            .execute(&mut document, "Layer Current Output")
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        let selection = document.selected_object_ids().collect::<Vec<_>>();
        let options = if name == "SrfPtGrid" {
            "DegreeU=1 DegreeV=1 KeepPoints=Yes 2 3"
        } else {
            "KeepPoints=Yes Degree=1 2 Degree=1 3"
        };
        let text = format!("{name} {options} 0,0,0 0,1,1 0,2,0 3,0,0 3,1,2 3,2,0");
        registry.execute(&mut document, &text).unwrap();
        let after = document.objects().cloned().collect::<Vec<_>>();
        assert_eq!(after.len(), 3);
        assert_eq!(after[0], before[0]);
        for obj in &after[1..] {
            assert_eq!(
                obj.attributes(),
                &ObjectAttributes::on_layer(document.current_layer_id())
            );
            assert!(!document.is_selected(obj.id()));
            assert!(
                document
                    .groups()
                    .all(|g| !g.members().any(|id| id == obj.id()))
            );
        }
        let Geometry::PointCloud(cloud) = after[2].geometry() else {
            panic!("KeepPoints must create one point cloud")
        };
        assert_eq!(cloud.points()[1], Point3::try_new(0.0, 1.0, 1.0).unwrap());
        assert_eq!(cloud.points()[3], Point3::try_new(3.0, 0.0, 0.0).unwrap());
        let Geometry::Brep(brep) = after[1].geometry() else {
            panic!("point grid output must be a B-rep")
        };
        let s = brep.faces()[0].surface();
        assert_eq!(s.control_points()[1].point(), cloud.points()[3]);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            selection
        );
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn malformed_grid_commands_do_not_change_geometry_selection_or_history() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selection = doc.selected_object_ids().collect::<Vec<_>>();
    let history = doc.undo_label().map(str::to_owned);
    for command in [
        "SrfPtGrid",
        "SrfPtGrid 2 2 0,0 1,0 0,1",
        "SrfPtGrid 999999999999999 2",
        "SrfPtGrid DegreeU=0 2 2",
        "SrfPtGrid DegreeV=12 2 2",
        "SrfPtGrid ClosedU=Maybe 2 2",
        "SrfControlPtGrid ClosedU=Yes 2 2",
        "SrfControlPtGrid DegreeU=1 2 2",
        "SrfControlPtGrid Degree=0 2 2",
        "SrfControlPtGrid Degree=1 2 Degree=12 2",
        "SrfControlPtGrid Degree=1 2 Degree=1 2 Degree=1",
        "SrfPtGrid 2 2 0,0 0,1 1,0 1,1",
        "SrfPtGrid DegreeU=1 DegreeV=1 2 2 0,0 0,0 0,0 0,0",
        "SrfPtGrid DegreeU=1 DegreeV=1 2 2 0,0 0,1 1,0 1,1 extra",
        "SrfPtGrid DegreeU=1 DegreeV=1 2 2 0,0 0,1 1,0 NaN,1,0",
    ] {
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selection);
        assert_eq!(doc.undo_label(), history.as_deref());
    }
}
