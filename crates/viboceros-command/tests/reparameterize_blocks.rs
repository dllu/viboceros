//! Independent public Rhino command records on the same native block graphs.
use serde_json::Value;
use viboceros_command::{
    CommandRegistry, ReparameterizeDomains, ReparameterizeReference, ReparameterizeSelection,
};
use viboceros_document::{Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::Point3;

fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn gap(geometry: &Geometry, expected: &Value) -> f64 {
    let expected = &expected["value"];
    let mut gap = 0_f64;
    let mut distance = |a: Point3, b: &Value| {
        gap = gap.max(a.distance_to(point(b)).unwrap());
    };
    match geometry {
        Geometry::Brep(brep) => {
            let faces = expected["faces"].as_array().unwrap();
            if brep.faces().len() != faces.len()
                || brep.vertices().len() != expected["vertices"].as_array().unwrap().len()
                || brep.edges().len() != expected["edges"].as_array().unwrap().len()
            {
                return f64::INFINITY;
            }
            for (vertex, reference) in brep
                .vertices()
                .iter()
                .zip(expected["vertices"].as_array().unwrap())
            {
                distance(vertex.point(), &reference["point"]);
            }
            for (edge, reference) in brep
                .edges()
                .iter()
                .zip(expected["edges"].as_array().unwrap())
            {
                let curve = edge.curve();
                let samples = reference["curve"]["samples"].as_array().unwrap();
                for (i, p) in samples.iter().enumerate() {
                    distance(
                        curve
                            .evaluate(
                                curve
                                    .parameter_at(i as f64 / (samples.len() - 1) as f64)
                                    .unwrap(),
                            )
                            .unwrap(),
                        p,
                    );
                }
            }
            for (face, reference) in brep.faces().iter().zip(faces) {
                let s = face.surface();
                for (i, p) in reference["samples"].as_array().unwrap().iter().enumerate() {
                    distance(
                        s.evaluate(
                            s.parameter_at_u((i % 9) as f64 / 8.).unwrap(),
                            s.parameter_at_v((i / 9) as f64 / 8.).unwrap(),
                        )
                        .unwrap(),
                        p,
                    );
                }
                if face.loops().len() != reference["loops"].as_array().unwrap().len() {
                    return f64::INFINITY;
                }
                for (a, b) in face
                    .loops()
                    .iter()
                    .zip(reference["loops"].as_array().unwrap())
                {
                    if a.trims().len() != b.as_array().unwrap().len() {
                        return f64::INFINITY;
                    }
                    for (trim, reference) in a.trims().iter().zip(b.as_array().unwrap()) {
                        for (i, p) in reference["lifted"].as_array().unwrap().iter().enumerate() {
                            let curve = trim.curve();
                            let uv = curve
                                .evaluate(curve.parameter_at(i as f64 / 32.).unwrap())
                                .unwrap();
                            distance(s.evaluate(uv.x(), uv.y()).unwrap(), p);
                        }
                    }
                }
            }
            for (face, reference) in brep.faces().iter().zip(faces) {
                let s = face.surface();
                for (domain, key) in [(s.domain_u(), "domain_u"), (s.domain_v(), "domain_v")] {
                    gap = gap
                        .max(
                            (domain.start() - reference["definition"][key][0].as_f64().unwrap())
                                .abs(),
                        )
                        .max(
                            (domain.end() - reference["definition"][key][1].as_f64().unwrap())
                                .abs(),
                        );
                }
            }
        }
        geometry => {
            let Some(curve) = geometry.curve_ref() else {
                return f64::INFINITY;
            };
            let samples = expected["samples"].as_array().unwrap();
            for (i, p) in samples.iter().enumerate() {
                distance(
                    curve
                        .evaluate(
                            curve
                                .parameter_at(i as f64 / (samples.len() - 1) as f64)
                                .unwrap(),
                        )
                        .unwrap(),
                    p,
                );
            }
            let domain = curve.domain();
            gap = gap
                .max((domain.start() - expected["domain"][0].as_f64().unwrap()).abs())
                .max((domain.end() - expected["domain"][1].as_f64().unwrap()).abs());
        }
    }
    gap
}
#[test]
fn placed_block_copies_match_native_geometry_attributes_and_atomic_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/reparameterize_blocks.json"
    ))
    .unwrap();
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/reparameterize_blocks.request.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for (case, op) in capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(request["operations"].as_array().unwrap())
    {
        let name = case["id"].as_str().unwrap();
        assert_eq!(op["id"], name);
        let path = root.join(format!(
            "tools/rhino_oracle/fixtures/reparameterize_blocks/{name}.3dm"
        ));
        let (mut doc, _, _, _) =
            viboceros_command::open_3dm_with_views(path.to_str().unwrap()).unwrap();
        let id = case["root_id"]
            .as_str()
            .unwrap()
            .parse::<ObjectId>()
            .unwrap();
        assert!(
            doc.object(id).is_some(),
            "{name}: source UUID must come from shared 3dm"
        );
        doc.clear_selection();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let definitions = doc.block_definitions().cloned().collect::<Vec<_>>();
        let expected = case["outputs"].as_array().unwrap();
        if name == "mixed_rejected" {
            let refs = [
                ReparameterizeReference {
                    object: id,
                    member: Some(vec![0]),
                    face: None,
                },
                ReparameterizeReference {
                    object: id,
                    member: Some(vec![1]),
                    face: Some(5),
                },
            ];
            assert!(matches!(
                ReparameterizeSelection::prepare(&doc, refs, ReparameterizeDomains::Automatic),
                Err(viboceros_command::CommandError::ReparameterizeMixedGeometry)
            ));
            assert!(expected.is_empty());
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            continue;
        }
        let base_path: Vec<usize> = serde_json::from_value(case["member_path"].clone()).unwrap();
        let automatic = op["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "Automatic");
        let numeric = op["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_f64)
            .collect::<Vec<_>>();
        if name == "block_cancel" {
            let plan = ReparameterizeSelection::prepare(
                &doc,
                [ReparameterizeReference {
                    object: id,
                    member: Some(base_path),
                    face: None,
                }],
                ReparameterizeDomains::Automatic,
            )
            .unwrap();
            drop(plan);
            assert!(expected.is_empty());
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!doc.can_undo());
            continue;
        }
        let domains = if automatic {
            ReparameterizeDomains::Automatic
        } else if numeric.is_empty() {
            ReparameterizeDomains::Explicit(
                serde_json::from_value(expected[0]["geometry"]["value"]["domain"].clone()).unwrap(),
            )
        } else {
            ReparameterizeDomains::Explicit(numeric.clone())
        };
        let references = expected
            .iter()
            .map(|output| {
                let mut path = base_path.clone();
                if output["geometry_text"]["GeometryCode"] == "alternate" {
                    *path.last_mut().unwrap() = 1;
                }
                let face = if output["geometry"]["type"] == "brep" {
                    let faces = output["geometry"]["value"]["faces"].as_array().unwrap();
                    Some(if automatic {
                        0
                    } else {
                        faces
                            .iter()
                            .position(|f| {
                                f["definition"]["domain_u"][0].as_f64() == numeric.first().copied()
                            })
                            .unwrap()
                    })
                } else {
                    None
                };
                ReparameterizeReference {
                    object: id,
                    member: Some(path),
                    face,
                }
            })
            .collect::<Vec<_>>();
        let plan = ReparameterizeSelection::prepare(&doc, references, domains)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            doc.objects().cloned().collect::<Vec<_>>(),
            before,
            "{name}: staging"
        );
        plan.commit(&mut doc)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            doc.block_definitions().cloned().collect::<Vec<_>>(),
            definitions,
            "{name}: definitions"
        );
        for source in &before {
            assert_eq!(doc.object(source.id()), Some(source), "{name}: source");
        }
        let actual = doc
            .objects()
            .filter(|o| !before.iter().any(|s| s.id() == o.id()))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(actual.len(), expected.len(), "{name}: copy count");
        let mut remaining = expected.iter().collect::<Vec<_>>();
        let mut maximum = 0_f64;
        for copy in &actual {
            assert_eq!(
                copy.attributes(),
                &ObjectAttributes::on_layer(doc.current_layer_id()),
                "{name}: fresh attributes"
            );
            assert!(copy.group_ids().is_empty());
            assert!(!doc.is_selected(copy.id()));
            let (best, error) = remaining
                .iter()
                .enumerate()
                .map(|(i, e)| (i, gap(copy.geometry(), &e["geometry"])))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            let reference = remaining.remove(best);
            maximum = maximum.max(error);
            assert!(
                error <= 1e-8,
                "{name}: independent domain/image error {error}"
            );
            assert_eq!(
                serde_json::to_value(copy.geometry_user_text()).unwrap(),
                reference["geometry_text"],
                "{name}: geometry text"
            );
            assert_eq!(reference["attributes"]["layer"], "Current");
            assert_eq!(reference["attributes"]["color_source"], "ColorFromLayer");
            if let Geometry::Brep(b) = copy.geometry() {
                for (face, reference) in b.faces().iter().zip(
                    reference["geometry"]["value"]["topology"]["faces"]
                        .as_array()
                        .unwrap(),
                ) {
                    assert_eq!(
                        face.is_reversed(),
                        reference["reversed"].as_bool().unwrap(),
                        "{name}: face orientation"
                    );
                }
                assert_eq!(
                    b.is_solid(),
                    reference["geometry"]["value"]["topology"]["solid"]
                        .as_bool()
                        .unwrap()
                );
            }
        }
        assert_eq!(doc.undo_label(), Some("Reparameterize"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(
            doc.objects().cloned().collect::<Vec<_>>(),
            before,
            "{name}: undo"
        );
        assert!(!doc.can_undo());
        registry.execute(&mut doc, "Redo").unwrap();
        for copy in &actual {
            assert_eq!(doc.object(copy.id()), Some(copy), "{name}: redo");
        }
        eprintln!(
            "BLOCK_REPARAMETERIZE_AUDIT {}",
            serde_json::json!({"id":name,"copies":actual.len(),"maximum_domain_or_image_error":maximum})
        );
    }
}
