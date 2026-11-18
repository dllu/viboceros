use super::*;
use serde_json::Value;

#[test]
fn solid_axial_tube_matches_rhino_profiles_and_path() {
    // Rhino 8.32.26160.13001 command file and live viewport captures.
    let captures: Vec<Value> = serde_json::from_str(include_str!("rhino_tube.json")).unwrap();
    for capture in captures {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let mut command = capture["command"].as_str().unwrap().to_owned();
        if capture["polysurface"].as_bool().unwrap_or(false) {
            registry
                .execute(&mut doc, "UseExtrusions Polysurface")
                .unwrap();
        }
        if let Some(points) = capture["points"].as_array() {
            let ids = points
                .iter()
                .map(|point| {
                    let point: [f64; 3] = serde_json::from_value(point.clone()).unwrap();
                    doc.add_geometry(Geometry::Point(Point3::try_from(point).unwrap()))
                        .unwrap()
                })
                .collect::<Vec<_>>();
            doc.select_objects_direct(ids, SelectionMode::Replace)
                .unwrap();
        }
        if let Some(guide) = capture.get("guide") {
            let [from, to]: [[f64; 3]; 2] = serde_json::from_value(guide.clone()).unwrap();
            let token = |[x, y, z]: [f64; 3]| format!("w{x},{y},{z}");
            registry
                .execute(&mut doc, &format!("Line {} {}", token(from), token(to)))
                .unwrap();
            command = command.replace("{guide}", &doc.objects().next().unwrap().id().to_string());
        }
        let source_count = doc.objects().len();
        registry
            .execute(&mut doc, &command)
            .unwrap_or_else(|error| panic!("{command}: {error}"));
        let outputs = capture["outputs"].as_array().unwrap();
        assert_eq!(
            doc.objects().len(),
            source_count + outputs.len(),
            "{command}"
        );
        for (object, expected) in doc.objects().skip(source_count).zip(outputs) {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("{command}: expected a B-rep");
            };
            assert_tube_matches(&command, brep, expected);
        }
        assert_eq!(doc.undo_label(), Some("Tube"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), source_count, "{command}");
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(
            doc.objects().len(),
            source_count + outputs.len(),
            "{command}"
        );
    }
}

fn assert_tube_matches(command: &str, brep: &Brep, expected: &Value) {
    let native_extrusion = &expected["extrusion"];
    let surfaces = expected["surfaces"].as_array().unwrap();
    if native_extrusion.is_null() {
        assert!(!brep.is_extrusion(), "{command}");
        assert_eq!(brep.faces().len(), surfaces.len(), "{command}");
        for (face, native) in brep.faces().iter().zip(surfaces) {
            assert_surface_matches(command, face.surface(), native);
        }
        assert_eq!(topology(brep), expected["topology"], "{command}");
    } else {
        let native = brep
            .extrusion()
            .unwrap_or_else(|| panic!("{command}: expected an extrusion"))
            .nurbs_form()
            .unwrap();
        assert_eq!(json!(native.caps), native_extrusion["caps"], "{command}");
        assert_eq!(native.path_axis, 1, "{command}");
        assert_surface_matches(command, &native.surface, &surfaces[0]);
        for (row, name) in ["path_start", "path_end"].into_iter().enumerate() {
            let corner = |i| native.surface.control_point(i, row).unwrap().point();
            let center = corner(0).midpoint(corner(4)).unwrap();
            let expected: [f64; 3] =
                serde_json::from_value(native_extrusion[name].clone()).unwrap();
            assert!(
                center
                    .distance_to(Point3::try_from(expected).unwrap())
                    .unwrap()
                    < 1e-9
            );
        }
    }
}
