#[path = "support/shell.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
use viboceros_command::CommandRegistry;

#[test]
fn imported_native_shell_ring_contains_points_past_the_inner_cavity_corner() {
    let value = native::read("shell_getter_holdouts")["results"][0]["value"].clone();
    let b = inputs::native_brep(
        value["after"].as_array().unwrap().last().unwrap(),
        viboceros_geometry::Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap(),
    );
    let p = viboceros_geometry::Point3::try_new(8.75, 2., 6.).unwrap();
    let t = viboceros_geometry::Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (i, f) in b.faces().iter().enumerate() {
        let (u, v) = f.surface().closest_parameters(p, t).unwrap();
        let n = f
            .surface()
            .normal_at(u, v)
            .unwrap()
            .as_vector()
            .scaled(if f.is_reversed() { -1. } else { 1. })
            .unwrap();
        if n.z() > 0.9 && f.surface().evaluate(u, v).unwrap().distance_to(p).unwrap() < 1e-6 {
            assert!(f.contains_parameters(u, v, t).unwrap());
            let (a, c) = b.closest_parameters_on_face(i, p, t).unwrap().unwrap();
            assert!(f.surface().evaluate(a, c).unwrap().distance_to(p).unwrap() < 1e-6);
            return;
        }
    }
    panic!("native shell ring surface must contain witness");
}

#[test]
fn shell_matches_native_geometry_metadata_identity_history_and_archives() {
    let mut count = 0;
    for (id, value) in inputs::rows() {
        if inputs::diagnostic(&value) {
            continue;
        }
        println!("SHELL_COMMAND_CASE {id}");
        let mut doc = inputs::setup(&value);
        if !inputs::supported(&doc, &value) {
            continue;
        }
        let state = inputs::State::capture(&doc);
        inputs::check(&doc, &value["before"], false);
        if inputs::success(&value) {
            let command = inputs::command(&doc, &value);
            CommandRegistry::with_builtins()
                .execute(&mut doc, &command)
                .unwrap();
        }
        let mut record = state.verify(&mut doc, &value);
        record["id"] = id.into();
        println!("SHELL_COMMAND {record}");
        count += 1;
    }
    println!("SHELL_COMMAND_COUNT {count}");
}

#[cfg(feature = "native-smlib")]
#[test]
fn shell_sdk_box_and_circular_openings_match_native_geometry_and_mass() {
    use viboceros_document::{Document, Geometry};
    use viboceros_geometry::Tolerance;
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/shell_sdk_geometry.json"
    ))
    .unwrap();
    let capture = native::read("shell_sdk_geometry");
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (op, row) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(capture["results"].as_array().unwrap())
    {
        let id = op["id"].as_str().unwrap();
        assert_eq!(row["id"], id);
        println!("SHELL_SDK_CASE {id}");
        let mut doc = Document::new(tolerance);
        let source = doc
            .add_geometry(inputs::source_geometry(&op["source"], tolerance))
            .unwrap();
        let faces = op["opening_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| format!("{source}:{}", inputs::face(&doc, source, p)))
            .collect::<Vec<_>>();
        doc.clear_history().unwrap();
        CommandRegistry::with_builtins()
            .execute(
                &mut doc,
                &format!(
                    "Shell Thickness={} FaceRefs={}",
                    op["thickness"],
                    faces.join(",")
                ),
            )
            .unwrap();
        let targets = row["value"]["breps"].as_array().unwrap();
        assert_eq!(doc.objects().len(), targets.len());
        let mut actual = doc.objects().collect::<Vec<_>>();
        actual.sort_by(|a, b| {
            a.geometry()
                .tight_bounds(tolerance)
                .unwrap()
                .min()
                .x()
                .total_cmp(&b.geometry().tight_bounds(tolerance).unwrap().min().x())
        });
        let mut error = 0_f64;
        let mut mass_error = 0_f64;
        let mut volume_gate = 0_f64;
        let mut coordinate_scale = 1_f64;
        let mut actual_topology = Vec::new();
        let mut native_topology = Vec::new();
        for (object, target) in actual.iter().zip(targets) {
            let Geometry::Brep(b) = object.geometry() else {
                panic!("shell walls")
            };
            actual_topology.push(serde_json::json!([
                b.vertices().len(),
                b.edges().len(),
                b.faces().len()
            ]));
            let g = &target["geometry"];
            native_topology.push(serde_json::json!([
                g["vertices"].as_array().unwrap().len(),
                g["edges"].as_array().unwrap().len(),
                g["faces"].as_array().unwrap().len()
            ]));
            coordinate_scale = coordinate_scale.max(
                b.vertices()
                    .iter()
                    .flat_map(|v| v.point().to_array())
                    .map(f64::abs)
                    .fold(1_f64, f64::max),
            );
            assert!(b.is_solid());
            assert_eq!(target["is_solid"], true);
            error = error.max(if op["source"]["type"] == "box_brep" {
                inputs::sdk_error(b, &target["geometry"], tolerance)
            } else {
                inputs::sdk_set_error(b, &target["geometry"], tolerance)
            });
            let volume = b.signed_volume(tolerance).unwrap().abs();
            let difference = (volume - target["volume"].as_f64().unwrap()).abs();
            volume_gate = volume_gate.max(b.area(tolerance).unwrap() * tolerance.absolute());
            assert!(
                difference <= b.area(tolerance).unwrap() * tolerance.absolute(),
                "{id}: volume error {difference:e}"
            );
            mass_error = mass_error.max(difference);
        }
        assert!(
            error <= 2e-6 + 64. * f64::EPSILON * coordinate_scale,
            "{id}: geometry {error:e}"
        );
        println!(
            "SHELL_SDK {}",
            serde_json::json!({"id":id,"maximum_geometry_error":error,"volume_error":mass_error,"volume_gate":volume_gate,"geometry_gate":2e-6+64.*f64::EPSILON*coordinate_scale,"native_topology":native_topology,"actual_topology":actual_topology})
        );
    }
    println!("SHELL_SDK_COUNT 9");
}
#[path = "support/brep_comparison.rs"]
#[allow(dead_code)]
mod brep_comparison;
