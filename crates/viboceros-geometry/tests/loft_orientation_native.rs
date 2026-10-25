//! Closed, non-self-crossing rings compared with captured command outputs.
use serde_json::Value;
use std::path::Path;
use viboceros_geometry::{Brep, LoftOptions, LoftStyle, Tolerance};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

#[test]
fn closed_ring_lofts_match_native_geometry_and_face_sense() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let q: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/loft_splitting_closed_orientation_regular.json"))
            .unwrap(),
    )
    .unwrap();
    let r: Value = serde_json::from_slice(
        &std::fs::read(root.join("observations/loft_splitting_closed_orientation_regular.json"))
            .unwrap(),
    )
    .unwrap();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 8);
    assert_eq!(r["results"].as_array().unwrap().len(), 8);
    let mut count = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        assert_eq!(op["closed"], true);
        let source = op["curves"]
            .as_array()
            .unwrap()
            .iter()
            .map(definitions::curve)
            .collect::<Vec<_>>();
        let before = source.clone();
        let style = match op["style"].as_str().unwrap() {
            "straight" => LoftStyle::Straight,
            "normal" => LoftStyle::Normal,
            other => panic!("unexpected native fixture style: {other}"),
        };
        let brep = Brep::try_loft_with_surface_options(
            &source,
            style,
            true,
            LoftOptions {
                crease_splitting: false,
                split_at_tangents: false,
                natural_seams: false,
            },
            tolerance,
            &mut viboceros_geometry::try_loft_nurbs_curves,
        )
        .unwrap();
        let native = row["value"]["after"].as_array().unwrap().last().unwrap();
        assert!(brep.is_solid());
        assert_eq!(native["solid_orientation"], 1);
        assert_eq!(
            brep.faces().len(),
            native["face_count"].as_u64().unwrap() as usize
        );
        assert_eq!(
            brep.edges().len(),
            native["edge_count"].as_u64().unwrap() as usize
        );
        assert_eq!(
            brep.vertices().len(),
            native["vertex_count"].as_u64().unwrap() as usize
        );
        let bounds = brep.tight_bounds(tolerance).unwrap();
        for (actual, native) in [bounds.min(), bounds.max()]
            .iter()
            .zip(native["bounds"].as_array().unwrap())
        {
            for (a, b) in actual
                .to_array()
                .into_iter()
                .zip(native.as_array().unwrap())
            {
                assert!((a - b.as_f64().unwrap()).abs() < 1e-6);
            }
        }
        for (face, native) in brep.faces().iter().zip(native["faces"].as_array().unwrap()) {
            assert_eq!(
                face.is_reversed(),
                native["reversed"].as_bool().unwrap(),
                "{}",
                op["id"]
            );
            let surface = face.surface();
            let definition = &native["definition"];
            assert_eq!(
                serde_json::json!([
                    surface.control_point_count_u(),
                    surface.control_point_count_v()
                ]),
                definition["control_count"]
            );
            for (cp, native) in surface
                .control_points()
                .iter()
                .zip(definition["control_points"].as_array().unwrap())
            {
                for (a, b) in cp
                    .point()
                    .to_array()
                    .into_iter()
                    .zip(native["point"].as_array().unwrap())
                {
                    assert!((a - b.as_f64().unwrap()).abs() < 1e-9);
                }
                assert!((cp.weight() - native["weight"].as_f64().unwrap()).abs() < 1e-9);
            }
            for (knots, key) in [
                (surface.knots_u(), "knots_u"),
                (surface.knots_v(), "knots_v"),
            ] {
                let expected = definition[key].as_array().unwrap();
                assert_eq!(knots.len(), expected.len());
                // OpenNURBS omits the two artificial full-vector outer knots.
                for (a, b) in knots[1..knots.len() - 1]
                    .iter()
                    .zip(&expected[1..expected.len() - 1])
                {
                    assert!((a - b.as_f64().unwrap()).abs() < 1e-9);
                }
            }
            for i in 0..=4 {
                for j in 0..=4 {
                    let point = surface
                        .evaluate(
                            surface.parameter_at_u(i as f64 / 4.).unwrap(),
                            surface.parameter_at_v(j as f64 / 4.).unwrap(),
                        )
                        .unwrap();
                    for (a, b) in point
                        .to_array()
                        .into_iter()
                        .zip(native["samples"][i * 5 + j].as_array().unwrap())
                    {
                        assert!((a - b.as_f64().unwrap()).abs() < 1e-9, "{}", op["id"]);
                    }
                }
            }
        }
        assert_eq!(source, before);
        count += 1;
    }
    assert_eq!(count, 8);
}
