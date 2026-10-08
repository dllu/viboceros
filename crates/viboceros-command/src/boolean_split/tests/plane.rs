use super::*;
use crate::boolean_union::tests::box_brep;
use serde_json::json;
use viboceros_document::{GroupId, LayerId};

pub(super) fn setup(v: &Value) -> (Document, Vec<ObjectId>, Vec<LayerId>, Vec<GroupId>) {
    let mut doc = Document::default();
    let mut ids = Vec::new();
    let mut layers = Vec::new();
    for (i, shape) in v["shapes"].as_array().unwrap().iter().enumerate() {
        let layer = doc
            .add_layer(format!("Source {i}"), ColorRgb::BLACK)
            .unwrap();
        let geometry = Geometry::Brep(shape_brep(shape, doc.tolerance()));
        let attrs = ObjectAttributes::on_layer(layer)
            .with_name(format!("source-{i}"))
            .with_object_color(ColorRgb::new(20 + i as u8, 40, 60))
            .try_with_user_text("Code", format!("attribute-{i}"))
            .unwrap();
        let id = doc.add_geometry_with_attributes(geometry, attrs).unwrap();
        doc.set_object_geometry_user_text([id], "Code", Some(&format!("geometry-{i}")))
            .unwrap();
        ids.push(id);
        layers.push(layer);
    }
    let mut groups = ids
        .iter()
        .map(|&id| doc.add_group(None, [id]).unwrap())
        .collect::<Vec<_>>();
    groups.push(doc.add_group(None, ids.iter().copied()).unwrap());
    doc.clear_history().unwrap();
    (doc, ids, layers, groups)
}

fn shape_brep(shape: &Value, tolerance: Tolerance) -> Brep {
    match shape["kind"].as_str().unwrap() {
        "box" => box_brep(serde_json::from_value(shape["bounds"].clone()).unwrap()),
        "compound" => Brep::try_disjoint_union(
            shape["parts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| shape_brep(p, tolerance))
                .collect(),
            tolerance,
        )
        .unwrap(),
        "sheet_hole" => {
            use viboceros_geometry::{
                BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
                NurbsCurve2, Point2, SurfaceIso,
            };
            let outer = serde_json::from_value::<[[f64; 3]; 4]>(shape["outer"].clone())
                .unwrap()
                .map(|p| Point3::try_from(p).unwrap());
            let mut hole = serde_json::from_value::<[[f64; 3]; 4]>(shape["hole"].clone())
                .unwrap()
                .map(|p| Point3::try_from(p).unwrap());
            hole.reverse();
            let surface = NurbsSurface::try_bilinear(outer).unwrap();
            let mut vertices = Vec::new();
            let mut edges = Vec::new();
            let mut loops = Vec::new();
            for (index, points) in [outer, hole].into_iter().enumerate() {
                let offset = vertices.len();
                vertices.extend(points.map(|p| BrepVertex::try_new(p, 0.).unwrap()));
                let mut trims = Vec::new();
                for i in 0..4 {
                    let ids = [offset + i, offset + (i + 1) % 4];
                    let e = edges.len();
                    edges.push(
                        BrepEdge::try_new(
                            ids,
                            NurbsCurve::try_new(
                                1,
                                ids.map(|i| vertices[i].point()).to_vec(),
                                vec![0., 0., 1., 1.],
                            )
                            .unwrap(),
                            0.,
                        )
                        .unwrap(),
                    );
                    let uv =
                        |p: Point3| Point2::try_new((p.y() + 1.) / 4., (p.z() + 1.) / 4.).unwrap();
                    trims.push(
                        BrepTrim::try_new(
                            ids,
                            Some(e),
                            false,
                            NurbsCurve2::try_line(
                                uv(vertices[ids[0]].point()),
                                uv(vertices[ids[1]].point()),
                            )
                            .unwrap(),
                            BrepTrimType::Boundary,
                            SurfaceIso::NotIso,
                            [0., 0.],
                        )
                        .unwrap(),
                    );
                }
                loops.push(
                    BrepLoop::try_new(
                        if index == 0 {
                            BrepLoopType::Outer
                        } else {
                            BrepLoopType::Inner
                        },
                        trims,
                    )
                    .unwrap(),
                );
            }
            Brep::try_new(
                vertices,
                edges,
                vec![BrepFace::try_new(surface, false, loops).unwrap()],
                tolerance,
            )
            .unwrap()
        }
        _ => {
            let points = serde_json::from_value::<[[f64; 3]; 4]>(shape["points"].clone())
                .unwrap()
                .map(|p| Point3::try_from(p).unwrap());
            let b = Brep::try_surface_face(NurbsSurface::try_bilinear(points).unwrap(), tolerance)
                .unwrap();
            if shape["reverse"] == true {
                b.reversed()
            } else {
                b
            }
        }
    }
}

#[test]
fn boolean_split_plane_replays_finite_coverage_order_caps_metadata_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/boolean_split_plane.json"
    ))
    .unwrap();
    let mut count = 0;
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let case = v["case"].as_str().unwrap();
        let (mut doc, ids, layers, groups) = setup(v);
        let original = ids
            .iter()
            .map(|id| doc.object(*id).unwrap().geometry_snapshot().clone())
            .collect::<Vec<_>>();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let cuts = ids
            .iter()
            .skip(1)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let command = format!(
            "BooleanSplit FirstSet={} SecondSet={cuts} DeleteInput={}",
            ids[0],
            if v["delete"] == true { "Yes" } else { "No" }
        );
        let registry = CommandRegistry::with_builtins();
        let result = if v["pre"] == true {
            registry.execute(&mut doc, &command)
        } else {
            registry.execute_postselected(&mut doc, &command, Default::default())
        };
        assert_eq!(
            result.is_ok(),
            v["command"]["success"] == true,
            "{case}: {result:?}"
        );
        let a = snapshot(&doc, &ids, &layers, &groups);
        let (expected, _) = align(&a, &v["command"]["after_script"]);
        compare(&a, &expected, case);
        for (i, &id) in ids.iter().enumerate().skip(1) {
            assert!(
                original[i].shares_storage_with(doc.object(id).unwrap().geometry_snapshot()),
                "{case}"
            );
        }
        if result.is_ok() {
            count += 1;
            for (object, expected) in doc.objects().zip(expected.as_array().unwrap()) {
                if !expected["source"].is_null() {
                    continue;
                }
                let Geometry::Brep(b) = object.geometry() else {
                    panic!()
                };
                let a = regions(b);
                let b =
                    serde_json::from_value::<Boundary>(expected["face_regions"].clone()).unwrap();
                witnesses(&a, &b, case);
                witnesses(&b, &a, case);
            }
            registry.execute(&mut doc, "Undo").unwrap();
            compare(
                &snapshot(&doc, &ids, &layers, &groups),
                &v["undo"]["after_script"],
                &format!("{case}/undo"),
            );
            registry.execute(&mut doc, "Redo").unwrap();
            let a = snapshot(&doc, &ids, &layers, &groups);
            compare(
                &a,
                &align(&a, &v["redo"]["after_script"]).0,
                &format!("{case}/redo"),
            );
        } else {
            assert!(!doc.can_undo(), "{case}");
        }
    }
    assert_eq!(count, 14);
}

#[test]
fn boolean_split_plane_bad_surface_and_mixed_target_failure_do_not_commit_other_outputs() {
    let v = json!({"shapes":[{"kind":"box","bounds":[[0,2],[0,2],[0,2]]},
        {"kind":"plane","points":[[1,-1,-1],[1,3,-1],[1,3,3],[1,-1,3]]}]});
    let (mut doc, ids, _, _) = setup(&v);
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Sphere 5,0 2").unwrap();
    let sphere = doc.objects().last().unwrap().id();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(
        registry
            .execute(
                &mut doc,
                &format!(
                    "BooleanSplit FirstSet={},{} SecondSet={}",
                    ids[0], sphere, ids[1]
                )
            )
            .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
}
