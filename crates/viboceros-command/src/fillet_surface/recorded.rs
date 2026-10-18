//! Exercise command ownership and picks against the existing public SDK captures.
use super::*;
use serde_json::Value;
fn surface(s: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        s["degree_u"].as_u64().unwrap() as usize,
        s["degree_v"].as_u64().unwrap() as usize,
        s["control_point_count_u"].as_u64().unwrap() as usize,
        s["control_point_count_v"].as_u64().unwrap() as usize,
        s["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[Real; 3]>(p["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    p["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(s["knots_u"].clone()).unwrap(),
        serde_json::from_value(s["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
fn bounds(b: &Brep) -> [[Real; 3]; 2] {
    // The SDK capture measures the trimmed image. Brep::bounds is a
    // conservative control-geometry box, so use the same tight native query
    // as the existing oracle rather than comparing different box policies.
    let tolerance = Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap();
    let bounds = viboceros_smlib::Shape::from_brep(b, tolerance)
        .unwrap()
        .tight_bounds()
        .unwrap();
    [bounds.min().to_array(), bounds.max().to_array()]
}
fn close(a: [[Real; 3]; 2], e: &Value, context: &str) {
    let e: [[Real; 3]; 2] = serde_json::from_value(e.clone()).unwrap();
    for (x, y) in a.into_iter().flatten().zip(e.into_iter().flatten()) {
        assert!((x - y).abs() <= 1e-8, "{context}: {x} vs {y}");
    }
}
#[test]
fn sdk_parent_recipes_keep_the_expected_pieces_and_source_identity() {
    check_command_fixture("surface_fillet_parents");
    check_command_fixture("surface_fillet_charts");
}
fn check_command_fixture(name: &str) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures").join(format!("{name}.json"))).unwrap(),
    )
    .unwrap();
    let reference: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations").join(format!("{name}.json"))).unwrap(),
    )
    .unwrap();
    let canonical_request: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/surface_fillet_parents.json")).unwrap(),
    )
    .unwrap();
    let canonical_reference: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/surface_fillet_parents.json")).unwrap(),
    )
    .unwrap();
    let mut inaccurate_sdk = 0;
    let registry = CommandRegistry::with_builtins();
    for (op, record) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], record["id"]);
        let context = op["id"].as_str().unwrap();
        let expected = &record["value"];
        let mut component_tolerance = 0_f64;
        for body in expected["outputs"].as_array().unwrap().iter().chain(
            expected["parents"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|p| p.as_array().unwrap()),
        ) {
            for table in ["vertices", "edges"] {
                for part in body["geometry"][table].as_array().unwrap() {
                    component_tolerance =
                        component_tolerance.max(part["tolerance"].as_f64().unwrap());
                }
            }
        }
        let sdk_surface = &expected["outputs"][0]["geometry"]["faces"][0]["definition"];
        let coarse_sdk = name == "surface_fillet_charts"
            && sdk_surface["degree"][0] == 3
            && sdk_surface["control_count"][0] == 7;
        let geometric = if coarse_sdk {
            assert!(component_tolerance > 1e-8);
            // The oracle's independent radius witness identifies these eight
            // coarse SDK fits. Keep their literal domains, counts and raw
            // observations; compare geometry to identical canonical inputs.
            assert_eq!(name, "surface_fillet_charts");
            inaccurate_sdk += 1;
            let id = context.rsplit_once('-').unwrap().0;
            let original = canonical_request["operations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o["id"] == id)
                .unwrap();
            for i in 0..2 {
                assert_eq!(
                    surface(&op["surfaces"][i])
                        .try_reparameterized(0. ..=1., 0. ..=1.)
                        .unwrap(),
                    surface(&original["surfaces"][i])
                        .try_reparameterized(0. ..=1., 0. ..=1.)
                        .unwrap()
                );
            }
            &canonical_reference["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == id)
                .unwrap()["value"]
        } else {
            expected
        };
        let mut doc = Document::new(Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap());
        let ids = [
            doc.add_geometry(Geometry::NurbsSurface(surface(&op["surfaces"][0])))
                .unwrap(),
            doc.add_geometry(Geometry::NurbsSurface(surface(&op["surfaces"][1])))
                .unwrap(),
        ];
        for (i, id) in ids.iter().enumerate() {
            doc.set_object_names([(*id, Some(format!("parent{i}")))])
                .unwrap();
        }
        let originals = ids.map(|id| doc.object(id).unwrap().geometry_snapshot().clone());
        doc.clear_history().unwrap();
        let picks: Vec<[Real; 2]> = serde_json::from_value(op["picks"].clone()).unwrap();
        registry
            .execute(
                &mut doc,
                &format!(
                    "FilletSrf Sources={},{} Pick1={},{} Pick2={},{} Radius={} Extend=No Trim={}",
                    ids[0],
                    ids[1],
                    picks[0][0],
                    picks[0][1],
                    picks[1][0],
                    picks[1][1],
                    op["radius"],
                    if op["trim"] == true { "Yes" } else { "Split" }
                ),
            )
            .unwrap_or_else(|e| panic!("{context}: {e}"));
        let selected = doc.selected_objects().next().unwrap();
        let Geometry::Brep(fillet) = selected.geometry() else {
            panic!()
        };
        close(bounds(fillet), &geometric["outputs"][0]["bounds"], context);
        for (i, id) in ids.iter().enumerate() {
            let pieces = expected["parents"][i].as_array().unwrap();
            let name = format!("parent{i}");
            let actual = doc
                .objects()
                .filter(|o| o.attributes().name() == Some(name.as_str()))
                .collect::<Vec<_>>();
            assert!(doc.object(*id).is_some());
            assert_eq!(actual.len(), pieces.len().max(1), "{context}");
            if pieces.is_empty() {
                assert!(
                    doc.object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .shares_storage_with(&originals[i])
                );
            } else {
                for piece in pieces {
                    let expected = &piece["geometry"]["faces"][0]["definition"];
                    assert!(
                        actual.iter().all(|object| {
                            let Geometry::Brep(b) = object.geometry() else {
                                return false;
                            };
                            let s = b.faces()[0].surface();
                            [*s.domain_u().start(), *s.domain_u().end()]
                                == serde_json::from_value::<[Real; 2]>(expected["domain_u"].clone())
                                    .unwrap()
                                && [*s.domain_v().start(), *s.domain_v().end()]
                                    == serde_json::from_value::<[Real; 2]>(
                                        expected["domain_v"].clone(),
                                    )
                                    .unwrap()
                        }),
                        "{context}: parent {i} chart changed"
                    );
                }
                let regions = geometric["parents"][i].as_array().unwrap();
                assert_eq!(pieces.len(), regions.len());
                for piece in regions {
                    assert!(
                        actual.iter().any(|o| {
                            let Geometry::Brep(b) = o.geometry() else {
                                return false;
                            };
                            let e: [[Real; 3]; 2] =
                                serde_json::from_value(piece["bounds"].clone()).unwrap();
                            bounds(b)
                                .into_iter()
                                .flatten()
                                .zip(e.into_iter().flatten())
                                .all(|(a, b)| (a - b).abs() <= 1e-8)
                        }),
                        "{context}: missing parent {i} piece"
                    );
                }
            }
        }
    }
    assert_eq!(
        inaccurate_sdk,
        if name == "surface_fillet_charts" {
            8
        } else {
            0
        }
    );
}
#[test]
fn rectangular_trimmed_parents_use_original_uv_picks_and_preserve_orientation() {
    let (_, mut doc, ids) = sources();
    for id in ids {
        let Geometry::NurbsSurface(s) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        let surface = s.try_reparameterized(-2. ..=8., 10. ..=30.).unwrap();
        let b = Brep::try_rectangular_surface_face_with_orientation(
            surface,
            -1. ..=7.,
            14. ..=26.,
            true,
            doc.tolerance(),
        )
        .unwrap();
        doc.replace_object_geometries([(id, Geometry::Brep(b))])
            .unwrap();
        let clipped = source_surface(&doc, id).unwrap();
        assert_eq!(clipped.domain_u(), -1. ..=7.);
        assert_eq!(clipped.domain_v(), 14. ..=26.);
    }
    let supports = ids.map(|id| {
        let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        b.faces()[0].surface().clone()
    });
    let options = parse(&["Pick1=0,20", "Pick2=0,20", "Extend=No"]).unwrap();
    prepare(&doc, ids, &options)
        .unwrap()
        .commit(&mut doc)
        .unwrap();
    for (i, id) in ids.into_iter().enumerate() {
        let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(b.faces()[0].surface(), &supports[i]);
        assert!(
            b.faces()[0]
                .contains_parameters(0., 20., doc.tolerance())
                .unwrap()
        );
        assert!(b.faces()[0].is_reversed());
        assert_eq!(b.faces()[0].surface().domain_u(), -2. ..=8.);
        assert_eq!(b.faces()[0].surface().domain_v(), 10. ..=30.);
    }
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!((b.bounds().min().y() - 2.).abs() < 1e-8);
    assert!((b.bounds().max().y() - 8.).abs() < 1e-8);
}

#[test]
fn curved_extension_is_declined_and_overlap_join_preserves_unchanged_parent() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/surface_fillet_picks.json"
    ))
    .unwrap();
    let op = &request["operations"][4];
    let mut doc = Document::new(Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap());
    let ids = [
        doc.add_geometry(Geometry::NurbsSurface(surface(&op["surfaces"][0])))
            .unwrap(),
        doc.add_geometry(Geometry::NurbsSurface(surface(&op["surfaces"][1])))
            .unwrap(),
    ];
    let options = parse(&["Pick1=0.6,0.6", "Pick2=0.5,0.2", "Radius=0.75", "Join=Yes"]).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    doc.clear_history().unwrap();
    assert!(
        prepare(&doc, ids, &options)
            .unwrap_err()
            .to_string()
            .contains("curved Extend=Yes")
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
    let options = options.updated(&["Extend=No"]).unwrap();
    prepare(&doc, ids, &options)
        .unwrap()
        .commit(&mut doc)
        .unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert!(
        doc.object(ids[0])
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(before[0].geometry_snapshot())
    );
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(b.faces().len(), 2);
    assert_eq!(doc.selected_objects().next().unwrap().id(), ids[1]);
}

#[test]
fn joined_parent_faces_keep_their_nonunit_original_uv_ranges() {
    let (_, mut doc, ids) = sources();
    for (i, id) in ids.into_iter().enumerate() {
        let Geometry::NurbsSurface(s) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        let s = s
            .try_reparameterized(
                if i == 0 { -2. ..=8. } else { 2. ..=7. },
                if i == 0 { 10. ..=30. } else { -8. ..=3. },
            )
            .unwrap();
        doc.replace_object_geometries([(id, Geometry::NurbsSurface(s))])
            .unwrap();
    }
    let options = parse(&["Pick1=0,20", "Pick2=3,-2.5", "Extend=No", "Join=Yes"]).unwrap();
    prepare(&doc, ids, &options)
        .unwrap()
        .commit(&mut doc)
        .unwrap();
    assert_eq!(doc.objects().len(), 1);
    let Geometry::Brep(b) = doc.object(ids[0]).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(b.faces().len(), 3);
    for domains in [[-2. ..=8., 10. ..=30.], [2. ..=7., -8. ..=3.]] {
        assert!(
            b.faces()
                .iter()
                .any(|f| [f.surface().domain_u(), f.surface().domain_v()] == domains)
        );
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fillet-charts.3dm");
    CommandRegistry::with_builtins()
        .execute(&mut doc, &format!("SaveAs {}", path.display()))
        .unwrap();
    let restored = viboceros_io::read_3dm_file_with_model_tolerance(&path).unwrap();
    assert_eq!(restored.unsupported_object_count(), 0);
    assert_eq!(restored.objects.len(), 1);
    let viboceros_io::ThreeDmGeometry::Brep(b) = &restored.objects[0].geometry else {
        panic!()
    };
    for domains in [[-2. ..=8., 10. ..=30.], [2. ..=7., -8. ..=3.]] {
        assert!(
            b.faces()
                .iter()
                .any(|f| [f.surface().domain_u(), f.surface().domain_v()] == domains)
        );
    }
}
