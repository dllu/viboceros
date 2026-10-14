use crate::*;

fn pair() -> [NurbsSurface; 2] {
    let controls = (0..4)
        .flat_map(|v| {
            (0..3).map(move |u| {
                Point3::try_new(
                    u as f64 / 2.,
                    v as f64 / 3.,
                    f64::from(u == 2) + f64::from(v == 3),
                )
                .unwrap()
            })
        })
        .collect();
    [
        NurbsSurface::try_new(
            2,
            3,
            3,
            4,
            controls,
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap(),
        NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 1.).unwrap(),
            Point3::try_new(1., 0., 1.).unwrap(),
            Point3::try_new(1., 1., 0.).unwrap(),
            Point3::try_new(0., 1., 1.).unwrap(),
        ])
        .unwrap(),
    ]
}

#[test]
fn intersect_general_surfaces_and_natural_faces_preserves_operands_and_history() {
    let registry = CommandRegistry::with_builtins();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    for brep_count in 0..=2 {
        let mut document = Document::new(tolerance);
        let ids = pair()
            .into_iter()
            .enumerate()
            .map(|(i, surface)| {
                let geometry = if i < brep_count {
                    Geometry::Brep(Brep::try_surface_face(surface, tolerance).unwrap())
                } else {
                    Geometry::NurbsSurface(surface)
                };
                document.add_geometry(geometry).unwrap()
            })
            .collect::<Vec<_>>();
        document
            .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let inputs = ids
            .iter()
            .map(|id| document.object(*id).unwrap().clone())
            .collect::<Vec<_>>();
        registry.execute(&mut document, "Intersect").unwrap();
        let outputs = document.selected_objects().cloned().collect::<Vec<_>>();
        assert!(!outputs.is_empty());
        for object in &outputs {
            let Geometry::NurbsCurve(curve) = object.geometry() else {
                panic!("expected curve");
            };
            for station in 0..=32 {
                let p = curve
                    .evaluate(curve.parameter_at(station as f64 / 32.).unwrap())
                    .unwrap();
                assert!((p.x().powi(2) + p.y().powi(3) + p.x() * p.y() - 1.).abs() < 2e-6);
            }
        }
        for input in &inputs {
            assert_eq!(document.object(input.id()), Some(input));
            assert!(!document.is_selected(input.id()));
        }
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().count(), 2);
        assert_eq!(document.selected_objects().count(), 0);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().count(), inputs.len() + outputs.len());
        for output in &outputs {
            assert_eq!(document.object(output.id()), Some(output));
        }
    }
}

#[test]
fn intersect_unsupported_general_degree_rolls_back_atomically() {
    let registry = CommandRegistry::with_builtins();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let mut document = Document::new(tolerance);
    let [first, second] = pair();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                first.try_change_degree(17, 3, false).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::Brep(
                Brep::try_rectangular_surface_face(second, 0.25..=0.75, 0. ..=1., tolerance)
                    .unwrap(),
            ))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before = format!("{document:?}");
    assert!(registry.execute(&mut document, "Intersect").is_err());
    assert_eq!(format!("{document:?}"), before);
}

#[test]
fn intersect_general_partial_faces_preserves_metadata_selection_and_undo_redo() {
    let registry = CommandRegistry::with_builtins();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    for reverse in [false, true] {
        let mut document = Document::new(tolerance);
        let [first, second] = pair();
        let source = document
            .add_geometry_with_attributes(
                Geometry::NurbsSurface(first),
                ObjectAttributes::on_layer(document.current_layer_id())
                    .with_name("General surface source")
                    .with_object_color(ColorRgb::new(12, 34, 56)),
            )
            .unwrap();
        let cutter = document
            .add_geometry(Geometry::Brep(
                Brep::try_rectangular_surface_face(second, 0.25..=0.75, 0. ..=1., tolerance)
                    .unwrap(),
            ))
            .unwrap();
        let ids = if reverse {
            [cutter, source]
        } else {
            [source, cutter]
        };
        document
            .select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        let before = ids.map(|id| document.object(id).unwrap().clone());
        registry.execute(&mut document, "Intersect").unwrap();
        let outputs = document.selected_objects().cloned().collect::<Vec<_>>();
        assert_eq!(outputs.len(), 1);
        let Geometry::NurbsCurve(curve) = outputs[0].geometry() else {
            panic!("expected curve");
        };
        for i in 0..=32 {
            let p = curve
                .evaluate(curve.parameter_at(i as f64 / 32.).unwrap())
                .unwrap();
            assert!(p.x() >= 0.25 - 1e-6 && p.x() <= 0.75 + 1e-6);
            assert!((p.x().powi(2) + p.y().powi(3) + p.x() * p.y() - 1.).abs() < 2e-6);
        }
        for object in &before {
            assert_eq!(document.object(object.id()), Some(object));
            assert!(!document.is_selected(object.id()));
        }
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().count(), 2);
        for object in &before {
            assert_eq!(document.object(object.id()), Some(object));
        }
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.object(outputs[0].id()), Some(&outputs[0]));
    }
}
