use super::*;
use crate::morph_test_support::{curve, surface};
use serde_json::Value;

fn distance(a: &NurbsCurve, b: &NurbsCurve, tolerance: Tolerance) -> Real {
    let mut maximum = 0_f64;
    for (a, b) in [(a, b), (b, a)] {
        for i in 0..=128 {
            let p = a
                .evaluate(a.parameter_at(i as Real / 128.).unwrap())
                .unwrap();
            let t = b.closest_parameter(p, tolerance).unwrap();
            maximum = maximum.max(p.distance_to(b.evaluate(t).unwrap()).unwrap());
        }
    }
    maximum
}

fn replay(wanted: &str) {
    let sdk:Value=serde_json::from_str(include_str!("../../../../../tools/rhino_oracle/observations/surface_curve_offset_general_extensions.json")).unwrap();
    let commands: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/surface_offset_general_ends.json"
    ))
    .unwrap();
    for (sdk, row) in sdk["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(commands["results"].as_array().unwrap())
    {
        assert_eq!(sdk["id"], row["id"]);
        if sdk["id"] != wanted {
            continue;
        }
        let id = sdk["id"].as_str().unwrap();
        let captured = &row["value"];
        let source = curve(&sdk["value"]["curve"]);
        assert_eq!(source, curve(&captured["before"][0]["curve"]));
        let surface = surface(&sdk["value"]["surface"]);
        let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
        let default = doc.current_layer_id();
        let source_layer = doc
            .add_layer(
                "Surface offset source",
                viboceros_document::ColorRgb::new(10, 20, 30),
            )
            .unwrap();
        let output_layer = doc
            .add_layer(
                "Surface offset output",
                viboceros_document::ColorRgb::new(40, 50, 60),
            )
            .unwrap();
        doc.set_current_layer(source_layer).unwrap();
        let source_id = doc.add_geometry(Geometry::NurbsCurve(source)).unwrap();
        doc.set_current_layer(default).unwrap();
        let target_id = doc
            .add_geometry(Geometry::NurbsSurface(surface.clone()))
            .unwrap();
        doc.set_object_names([
            (source_id, Some("Source".into())),
            (target_id, Some("Target".into())),
        ])
        .unwrap();
        doc.set_current_layer(output_layer).unwrap();
        let parents = [
            doc.object(source_id).unwrap().geometry_snapshot().clone(),
            doc.object(target_id).unwrap().geometry_snapshot().clone(),
        ];
        doc.select_object(source_id, SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let source = Source::capture(&doc, source_id, None, None).unwrap();
        let target = Target::capture(&doc, target_id, None).unwrap();
        let before = format!("{doc:?}");
        let started = std::time::Instant::now();
        let prepared = source
            .extended_on_surface(&target.surface, doc.tolerance())
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        let source_preparation_ms = started.elapsed().as_millis();
        let native = curve(&sdk["value"]["extended_source"]);
        let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-6).unwrap();
        let source_error = distance(&prepared, &native, tolerance);
        let endpoints = [
            (*prepared.domain().start(), *native.domain().start()),
            (*prepared.domain().end(), *native.domain().end()),
        ]
        .into_iter()
        .map(|(a, b)| {
            prepared
                .evaluate(a)
                .unwrap()
                .distance_to(native.evaluate(b).unwrap())
                .unwrap()
        })
        .fold(0_f64, Real::max);
        let native_surface_error = (0..=128)
            .map(|i| {
                let p = native
                    .evaluate(native.parameter_at(i as Real / 128.).unwrap())
                    .unwrap();
                let (u, v) = surface.closest_parameters(p, tolerance).unwrap();
                p.distance_to(surface.evaluate(u, v).unwrap()).unwrap()
            })
            .fold(0_f64, Real::max);
        println!(
            "SURFACE_NORMAL_GENERAL_EXTENSION {}",
            serde_json::json!({"id":id,"maximum_bidirectional_sample_error":source_error,"maximum_endpoint_difference":endpoints,"native_maximum_sample_surface_distance":native_surface_error,"qualified":source_error<1e-6,"controls":prepared.control_points().len(),"source_preparation_ms":source_preparation_ms})
        );
        assert!(endpoints < 1e-6, "{id}: source endpoints {endpoints:e}");
        assert!(
            source_error
                < if id == "general_rational" {
                    3e-5
                } else if id == "general_iso" {
                    1e-6
                } else {
                    2e-5
                },
            "{id}: source {source_error:e}"
        );
        let started = std::time::Instant::now();
        let options = Options {
            distance: if id == "general_iso" { -0.05 } else { 0.05 },
            number: 1,
            flip: false,
        };
        let plan = Plan::prepare(&doc, source.clone(), target, None, options)
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        let preparation_ms = started.elapsed().as_millis();
        assert!(Arc::ptr_eq(
            &prepared,
            source.prepared.get().unwrap().curve.as_ref().unwrap()
        ));
        assert_eq!(format!("{doc:?}"), before);
        let outputs = captured["after_script"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["role"] == "output")
            .map(|r| curve(&r["curve"]))
            .collect::<Vec<_>>();
        assert_eq!(plan.curves.len(), outputs.len(), "{id}");
        let maximum = plan
            .curves
            .iter()
            .zip(&outputs)
            .map(|(a, b)| distance(a, b, tolerance))
            .fold(0_f64, Real::max);
        let independent = if id == "general_iso" {
            let request:Value=serde_json::from_str(include_str!("../../../../../tools/rhino_oracle/fixtures/surface_curve_offset_general_verify.json")).unwrap();
            let response:Value=serde_json::from_str(include_str!("../../../../../tools/rhino_oracle/observations/surface_curve_offset_general_verify.json")).unwrap();
            let p = Point3::try_from(
                serde_json::from_value::<[Real; 3]>(request["operations"][0]["samples"][0].clone())
                    .unwrap(),
            )
            .unwrap();
            let current = &plan.curves[0];
            let t = current.closest_parameter(p, tolerance).unwrap();
            let error = current.evaluate(t).unwrap().distance_to(p).unwrap();
            let native = curve(&sdk["value"]["offset_extended_source"][0]);
            let t = native.closest_parameter(p, tolerance).unwrap();
            let distance = native.evaluate(t).unwrap().distance_to(p).unwrap();
            let official = response["results"][0]["value"]["source_first_sample_distances"][0]
                .as_f64()
                .unwrap();
            assert!(error < 1e-6);
            assert!(official > 1e-5 && official < 1.3e-5);
            assert!((distance - official).abs() < 1e-9);
            Some(
                serde_json::json!({"point":p.to_array(),"current_distance":error,"native_sdk_distance":official,"sdk_closest_solver_difference":(distance-official).abs()}),
            )
        } else {
            None
        };
        println!(
            "SURFACE_OFFSET_GENERAL_ENDS {}",
            serde_json::json!({"id":id,"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6,"outputs":plan.curves.len(),"native_outputs":outputs.len(),"preparation_ms":preparation_ms,"independent_geodesic":independent})
        );
        // Diagnostic ceilings remain separate from the unchanged model gate.
        let ceiling = match id {
            "general_iso" => 2.2e-5,
            "general_reversed" => 1.1e-6,
            _ => 1e-6,
        };
        assert!(maximum < ceiling, "{id}: command {maximum:e}");
        let mut group = doc.begin_history_group("OffsetCrvOnSrf").unwrap();
        plan.commit(&mut doc, &mut group).unwrap();
        for (a, b) in doc
            .objects()
            .zip(captured["after_script"].as_array().unwrap())
        {
            assert_eq!(doc.is_selected(a.id()), b["selected"].as_bool().unwrap());
            assert_eq!(a.attributes().name(), b["name"].as_str());
            assert_eq!(
                doc.layer(a.attributes().layer_id()).unwrap().name(),
                b["layer"].as_str().unwrap()
            );
        }
        assert!(doc.is_selected(source_id));
        for (id, snapshot) in [(source_id, &parents[0]), (target_id, &parents[1])] {
            assert!(snapshot.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
        }
        let snapshots = doc
            .objects()
            .map(|o| (o.id(), o.geometry_snapshot().clone()))
            .collect::<Vec<_>>();
        for output in doc.objects().skip(2) {
            assert!(!doc.is_selected(output.id()));
            assert!(output.attributes().name().is_none());
            assert_eq!(output.attributes().layer_id(), doc.current_layer_id());
        }
        doc.undo().unwrap();
        assert_eq!(doc.objects().count(), 2);
        assert!(doc.undo().unwrap().is_none());
        doc.redo().unwrap();
        for ((id, geometry), object) in snapshots.iter().zip(doc.objects()) {
            assert_eq!(*id, object.id());
            assert!(geometry.shares_storage_with(object.geometry_snapshot()));
        }
    }
}

#[test]
fn native_general_diagonal_sections_and_history() {
    replay("general_diagonal");
}
#[test]
fn native_general_reversed_sections_and_history() {
    replay("general_reversed");
}
#[test]
fn native_general_isocurve_sections_and_history() {
    replay("general_iso");
}
#[test]
fn native_general_rational_sections_and_history() {
    replay("general_rational");
}
