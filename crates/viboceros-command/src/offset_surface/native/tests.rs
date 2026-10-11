use super::*;
use serde_json::Value;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn warped_offset_faces_match_ideal_normals_and_public_rhino_outcomes() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/offset_warped_face_geometry.json"
    ))
    .unwrap();
    let source =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(4., 0., 0.), p(4., 3., 1.), p(0., 3., 0.)])
            .unwrap();
    for row in capture["results"].as_array().unwrap() {
        let case = row["id"].as_str().unwrap();
        let distance = if case == "warped_negative" {
            -0.25
        } else {
            0.25
        };
        let options = Options {
            distance,
            solid: case.contains("wall"),
            both_sides: case.contains("two_side"),
            delete_input: false,
        };
        let output = offset(
            &Geometry::NurbsSurface(source.clone()),
            options,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let brep = match output {
            Geometry::Brep(b) => b,
            Geometry::NurbsSurface(s) => Brep::try_surface_face(s, Tolerance::DEFAULT).unwrap(),
            _ => panic!(),
        };
        let expected = &row["value"];
        assert_eq!(brep.is_solid(), expected["solid"] == true, "{case}");
        assert_eq!(
            brep.faces().len(),
            expected["faces"].as_u64().unwrap() as usize,
            "{case}"
        );
        for d in if options.both_sides {
            vec![distance, -distance]
        } else {
            vec![distance]
        } {
            for u in [0., 0.25, 0.5, 0.75, 1.] {
                for v in [0., 0.25, 0.5, 0.75, 1.] {
                    let point = source
                        .evaluate(u, v)
                        .unwrap()
                        .translated(
                            source
                                .normal_at(u, v)
                                .unwrap()
                                .as_vector()
                                .scaled(d)
                                .unwrap(),
                        )
                        .unwrap();
                    let error = brep
                        .faces()
                        .iter()
                        .map(|f| {
                            let (a, b) = f
                                .surface()
                                .closest_parameters(point, Tolerance::NUMERICAL_VALIDATION)
                                .unwrap();
                            f.surface()
                                .evaluate(a, b)
                                .unwrap()
                                .distance_to(point)
                                .unwrap()
                        })
                        .fold(f64::INFINITY, f64::min);
                    assert!(
                        error < 1e-8,
                        "{case} ({u},{v}) normal offset error: {error}"
                    );
                }
            }
        }
        if options.solid {
            let volume = brep.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (volume - expected["admitted_volume"].as_f64().unwrap()).abs() < 2e-6,
                "{case} volume {volume} != {}",
                expected["admitted_volume"]
            );
        }
    }
}

#[test]
fn closed_box_offsets_create_shifted_solid_and_wall_with_atomic_history() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Box 0,0 2,2 2").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    doc.clear_history().unwrap();
    for (solid, volume) in [(false, 15.625), (true, 7.625)] {
        registry
            .execute(
                &mut doc,
                &format!(
                    "OffsetSrf 0.25 Solid={} DeleteInput=Yes",
                    if solid { "Yes" } else { "No" }
                ),
            )
            .unwrap();
        assert_eq!(doc.objects().count(), 1);
        let Geometry::Brep(brep) = doc.objects().next().unwrap().geometry() else {
            panic!("native polysurface")
        };
        assert!(brep.is_solid());
        assert!((brep.signed_volume(doc.tolerance()).unwrap() - volume).abs() < 1e-8);
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn box_offsets_match_public_sdk_and_retain_its_empty_wall_observations() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/offset_box_geometry.json"
    ))
    .unwrap();
    let mut partitions = Vec::new();
    for row in capture["results"].as_array().unwrap() {
        let case = row["id"].as_str().unwrap();
        let expected = &row["value"];
        if case.contains("wall") {
            assert_eq!(expected["created"], false);
            assert!(expected["bodies"].as_array().unwrap().is_empty());
            continue;
        }
        let source = Brep::try_box(
            CommandContext::default().construction_plane,
            [[0., 2.]; 3],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let options = Options {
            distance: if case.contains("negative") {
                -0.25
            } else {
                0.25
            },
            solid: false,
            both_sides: false,
            delete_input: false,
        };
        let Geometry::Brep(result) =
            offset(&Geometry::Brep(source), options, Tolerance::DEFAULT).unwrap()
        else {
            panic!()
        };
        let body = &expected["bodies"][0];
        assert_eq!(
            result.faces().len(),
            body["faces"].as_u64().unwrap() as usize
        );
        if result.edges().len() != body["edges"].as_u64().unwrap() as usize {
            partitions.push(serde_json::json!({"case":case,"actual_edges":result.edges().len(),"rhino_edges":body["edges"]}));
        }
        assert!(
            (result.signed_volume(Tolerance::DEFAULT).unwrap()
                - body["admitted_volume"].as_f64().unwrap())
            .abs()
                < 1e-8
        );
        for i in 0..3 {
            let low = result
                .vertices()
                .iter()
                .map(|v| v.point().to_array()[i])
                .fold(f64::INFINITY, f64::min);
            let high = result
                .vertices()
                .iter()
                .map(|v| v.point().to_array()[i])
                .fold(f64::NEG_INFINITY, f64::max);
            assert!((low - body["bbox_min"][i].as_f64().unwrap()).abs() < 1e-8);
            assert!((high - body["bbox_max"][i].as_f64().unwrap()).abs() < 1e-8);
        }
    }
    eprintln!(
        "Native offset partitions: {}",
        serde_json::to_string_pretty(&partitions).unwrap()
    );
    let recorded: Value = serde_json::from_str(include_str!(
        "../../../../../docs/smlib-offset-partitions.json"
    ))
    .unwrap();
    assert_eq!(serde_json::json!(partitions), recorded);
}
