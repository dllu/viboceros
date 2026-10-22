#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod mesh_native;
#[allow(dead_code)]
#[path = "support/crease.rs"]
mod native;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use viboceros_document::{Geometry, GripMode};
use viboceros_geometry::{AffineTransform3, Vector3};

#[test]
fn surface_grips_match_native_restricted_edits_and_keep_three_explicit_native_gaps() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/component_seams_subd_grips.json"
    ))
    .unwrap();
    let gaps: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/subd_surface_grip_gaps.json"
    ))
    .unwrap();
    assert_eq!(capture["results"].as_array().unwrap().len(), 9);
    let transform = AffineTransform3::from_translation(Vector3::try_new(0.1, 0.2, 0.3).unwrap());
    let mut matched = 0;
    for row in capture["results"].as_array().unwrap() {
        let value = &row["value"];
        assert_eq!(value["success"], true);
        let (doc, ids) = native::setup_from_family(row, "component_seams_subd_grips");
        let id = ids[0];
        assert_eq!(doc.grip_mode(id), Some(GripMode::Edit));
        let Geometry::SubD(source) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        let indices = value["input"]["sources"][0]["selected_vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i.as_u64().unwrap() as usize)
            .collect::<BTreeSet<_>>();
        let Geometry::SubD(edited) = doc
            .object(id)
            .unwrap()
            .geometry()
            .with_transformed_grips_in_mode(&indices, transform, doc.tolerance(), GripMode::Edit)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(edited.faces(), source.faces());
        assert_eq!(edited.vertex_tags(), source.vertex_tags());
        assert_eq!(edited.edge_tags(), source.edge_tags());
        assert_eq!(edited.edge_sharpness(), source.edge_sharpness());
        for (i, &p) in source.control_points().iter().enumerate() {
            if !indices.contains(&i) {
                assert_eq!(edited.control_points()[i], p);
            }
        }
        let expected = &value["after"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o.get("subd").is_some())
            .unwrap()["subd"];
        let mut selected_error = 0f64;
        let mut native_target_error = 0f64;
        let mut maximum = 0f64;
        for &i in &indices {
            let target = transform
                .transform_point(source.surface_points()[i])
                .unwrap();
            selected_error =
                selected_error.max(target.distance_to(edited.surface_points()[i]).unwrap());
            native_target_error = native_target_error.max(
                target
                    .distance_to(native::point(&expected["surface_points"][i]))
                    .unwrap(),
            );
        }
        assert!(selected_error <= 1e-6);
        for (points, key) in [
            (edited.control_points(), "points"),
            (edited.surface_points(), "surface_points"),
        ] {
            for (p, v) in points.iter().zip(expected[key].as_array().unwrap()) {
                maximum = maximum.max(p.distance_to(native::point(v)).unwrap());
            }
        }
        let native_match = gaps.get(row["id"].as_str().unwrap()).is_none();
        if native_match {
            let result = subd_native::compare(&edited, expected);
            maximum = maximum.max(result["maximum_sampled_surface_error"].as_f64().unwrap());
            assert!(maximum <= 1e-6);
            assert!(native_target_error <= 1e-6);
            matched += 1;
        } else {
            assert!(maximum > 1e-6);
            assert!(native_target_error > 1e-6);
        }
        println!(
            "SUBD_SURFACE_GRIP {}",
            json!({"id":row["id"],"native_match":native_match,"maximum_error":maximum,"selected_station_error":selected_error,"native_selected_station_error":native_target_error,"untouched_controls":true,"tags_and_topology":true})
        );
    }
    assert_eq!(matched, 6);
}
