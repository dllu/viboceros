use super::*;
use viboceros_geometry::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    CircularArc3, CurveSegment3, LineSegment, NurbsCurve2, NurbsSurface, Point2, PolyCurve3,
    SurfaceIso, WeightedPoint2, WeightedPoint3,
};

fn numbers(value: &Value) -> Vec<f64> {
    serde_json::from_value(value.clone()).unwrap()
}
fn curve(value: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        value["degree"].as_u64().unwrap() as usize,
        value["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(point_value(&p["point"]), p["weight"].as_f64().unwrap())
                    .unwrap()
            })
            .collect(),
        numbers(&value["knots"]),
    )
    .unwrap()
}
fn surface(value: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        value["degree"][0].as_u64().unwrap() as usize,
        value["degree"][1].as_u64().unwrap() as usize,
        value["control_count"][0].as_u64().unwrap() as usize,
        value["control_count"][1].as_u64().unwrap() as usize,
        value["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(point_value(&p["point"]), p["weight"].as_f64().unwrap())
                    .unwrap()
            })
            .collect(),
        numbers(&value["knots_u"]),
        numbers(&value["knots_v"]),
    )
    .unwrap()
}
fn brep(value: &Value, tolerance: Tolerance) -> Brep {
    let vertices = value["vertices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            BrepVertex::try_new(point_value(&v["point"]), v["tolerance"].as_f64().unwrap()).unwrap()
        })
        .collect();
    let edges = value["edges"]
        .as_array()
        .unwrap()
        .iter()
        .zip(value["topology"]["edges"].as_array().unwrap())
        .map(|(e, v)| {
            BrepEdge::try_new(
                serde_json::from_value(v.clone()).unwrap(),
                curve(&e["curve"]["definition"]),
                e["tolerance"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    let faces = value["faces"]
        .as_array()
        .unwrap()
        .iter()
        .zip(value["topology"]["faces"].as_array().unwrap())
        .map(|(f, t)| {
            let loops = f["loops"]
                .as_array()
                .unwrap()
                .iter()
                .zip(t["loops"].as_array().unwrap())
                .map(|(l, tl)| {
                    let trims = l
                        .as_array()
                        .unwrap()
                        .iter()
                        .zip(tl["trims"].as_array().unwrap())
                        .map(|(r, tr)| {
                            let d = &r["definition"];
                            let uv = NurbsCurve2::try_new_rational(
                                d["degree"].as_u64().unwrap() as usize,
                                d["control_points"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .map(|p| {
                                        WeightedPoint2::try_new(
                                            Point2::try_from(
                                                serde_json::from_value::<[f64; 2]>(
                                                    p["point"].clone(),
                                                )
                                                .unwrap(),
                                            )
                                            .unwrap(),
                                            p["weight"].as_f64().unwrap(),
                                        )
                                        .unwrap()
                                    })
                                    .collect(),
                                numbers(&d["knots"]),
                            )
                            .unwrap();
                            let iso = match r["iso"].as_u64().unwrap() {
                                0 => SurfaceIso::NotIso,
                                1 => SurfaceIso::InteriorUConstant,
                                2 => SurfaceIso::InteriorVConstant,
                                3 => SurfaceIso::West,
                                4 => SurfaceIso::South,
                                5 => SurfaceIso::East,
                                6 => SurfaceIso::North,
                                _ => panic!(),
                            };
                            let kind = match tr["type"].as_str().unwrap() {
                                "Boundary" => BrepTrimType::Boundary,
                                "Mated" => BrepTrimType::Mated,
                                "Seam" => BrepTrimType::Seam,
                                "Singular" => BrepTrimType::Singular,
                                _ => panic!(),
                            };
                            BrepTrim::try_new(
                                serde_json::from_value(tr["vertices"].clone()).unwrap(),
                                tr["edge"].as_u64().map(|i| i as usize),
                                tr["reversed"].as_bool().unwrap(),
                                uv,
                                kind,
                                iso,
                                serde_json::from_value(r["tolerance"].clone()).unwrap(),
                            )
                            .unwrap()
                        })
                        .collect();
                    BrepLoop::try_new(
                        if tl["outer"] == true {
                            BrepLoopType::Outer
                        } else {
                            BrepLoopType::Inner
                        },
                        trims,
                    )
                    .unwrap()
                })
                .collect();
            BrepFace::try_new(
                surface(&f["definition"]),
                t["reversed"].as_bool().unwrap(),
                loops,
            )
            .unwrap()
        })
        .collect();
    Brep::try_new(vertices, edges, faces, tolerance).unwrap()
}
fn source(row: &Value, tolerance: Tolerance) -> Geometry {
    match row["kind"].as_str().unwrap() {
        "Point" => Geometry::Point(point_value(&row["point"])),
        "Brep" => Geometry::Brep(brep(&row["brep"], tolerance)),
        "Mesh" => super::super::grip_transform::geometry(&json!({"source":"mesh"}), row),
        "LineCurve" => Geometry::Line(
            LineSegment::try_new(
                point_value(&row["curve"]["control_points"][0]["point"]),
                point_value(&row["curve"]["control_points"][1]["point"]),
                tolerance,
            )
            .unwrap(),
        ),
        "PolyCurve" => Geometry::PolyCurve(
            PolyCurve3::try_new(vec![
                CurveSegment3::Line(
                    LineSegment::try_new(point(-2., 0., 0.), point(0., 0., 0.), tolerance).unwrap(),
                ),
                CurveSegment3::Arc(
                    CircularArc3::try_from_three_points(
                        point(0., 0., 0.),
                        point(1., 1., 0.),
                        point(2., 0., 0.),
                        tolerance,
                    )
                    .unwrap(),
                ),
            ])
            .unwrap(),
        ),
        "NurbsCurve" => Geometry::NurbsCurve(curve(&row["curve"])),
        other => panic!("{other}"),
    }
}
fn curve_json(c: &NurbsCurve) -> Value {
    json!({"degree":c.degree(),"control_points":c.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>(),
        "knots":c.knots(),"domain":[*c.domain().start(),*c.domain().end()]})
}
fn check(app: &VibocerosApp, ids: &[ObjectId], expected: &Value, label: &str) {
    let rows = expected.as_array().unwrap();
    assert_eq!(app.document.objects().len(), rows.len(), "{label}");
    for (o, row) in app.document.objects().zip(rows) {
        assert_eq!(
            Some(row["source"].as_u64().unwrap() as usize),
            ids.iter().position(|id| *id == o.id()),
            "{label}"
        );
        assert_eq!(o.attributes().name(), row["name"].as_str(), "{label}");
        assert_eq!(
            o.attributes().user_text().get("Code").map(String::as_str),
            row["attribute_text"].as_str(),
            "{label}"
        );
        assert_eq!(
            o.geometry_user_text().get("Code").map(String::as_str),
            row["geometry_text"].as_str(),
            "{label}"
        );
        assert_eq!(
            app.document.is_selected(o.id()),
            row["selected"].as_bool().unwrap(),
            "{label}"
        );
        assert_eq!(
            app.document.control_point_locations(o.id()).is_some(),
            row["grips_on"].as_bool().unwrap(),
            "{label}"
        );
        let grips=json!(app.document.control_points().filter(|(id,_,_)|id.object==o.id()).map(|(id,p,selected)|json!({"index":id.index,"point":p.to_array(),"selected":selected})).collect::<Vec<_>>());
        compare(&grips, &row["grips"], label);
        match o.geometry() {
            Geometry::Brep(b) => {
                let native = &row["brep"];
                assert_eq!(
                    b.is_solid(),
                    native["topology"]["solid"].as_bool().unwrap(),
                    "{label}"
                );
                assert_eq!(
                    b.vertices().len(),
                    native["vertices"].as_array().unwrap().len(),
                    "{label}"
                );
                assert_eq!(
                    b.edges().len(),
                    native["edges"].as_array().unwrap().len(),
                    "{label}"
                );
                for (a, n) in b
                    .vertices()
                    .iter()
                    .zip(native["vertices"].as_array().unwrap())
                {
                    compare(&json!(a.point().to_array()), &n["point"], label);
                }
                for (a, n) in b.edges().iter().zip(native["edges"].as_array().unwrap()) {
                    let native_curve = curve(&n["curve"]["definition"]);
                    let d = a.curve().domain();
                    for (i, p) in n["curve"]["samples"].as_array().unwrap().iter().enumerate() {
                        let target = point_value(p);
                        let closest = a
                            .curve()
                            .closest_parameter(target, app.document.tolerance())
                            .unwrap();
                        let error = a
                            .curve()
                            .evaluate(closest)
                            .unwrap()
                            .distance_to(target)
                            .unwrap();
                        assert!(error <= 1e-7, "{label}/native edge sample {i}: {error}");
                        let t = d.start() + (d.end() - d.start()) * i as f64 / 32.;
                        let target = a.curve().evaluate(t).unwrap();
                        let closest = native_curve
                            .closest_parameter(target, app.document.tolerance())
                            .unwrap();
                        let error = native_curve
                            .evaluate(closest)
                            .unwrap()
                            .distance_to(target)
                            .unwrap();
                        assert!(error <= 1e-7, "{label}/edited edge sample {i}: {error}");
                    }
                }
                for (a, n) in b.faces().iter().zip(native["faces"].as_array().unwrap()) {
                    let s = a.surface();
                    compare(
                        &json!({"degree":[s.degree_u(),s.degree_v()],"control_count":[s.control_point_count_u(),s.control_point_count_v()],
                        "control_points":s.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>(),
                        "knots_u":s.knots_u(),"knots_v":s.knots_v(),"domain_u":[*s.domain_u().start(),*s.domain_u().end()],"domain_v":[*s.domain_v().start(),*s.domain_v().end()]}),
                        &n["definition"],
                        label,
                    );
                    for (l, nl) in a.loops().iter().zip(n["loops"].as_array().unwrap()) {
                        for (t, nt) in l.trims().iter().zip(nl.as_array().unwrap()) {
                            let c = t.curve();
                            compare(
                                &json!({"degree":c.degree(),"control_points":c.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>(),"knots":c.knots(),"domain":[*c.domain().start(),*c.domain().end()]}),
                                &nt["definition"],
                                label,
                            );
                        }
                    }
                }
            }
            Geometry::Mesh(m) => compare(
                &json!({"vertices":m.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":m.faces().iter().map(|f|f.indices().to_vec()).collect::<Vec<_>>()}),
                &row["mesh"],
                label,
            ),
            Geometry::Point(p) => compare(&json!(p.to_array()), &row["point"], label),
            g => {
                let kind = match g {
                    Geometry::PolyCurve(_) => "PolyCurve",
                    Geometry::Line(_) => "LineCurve",
                    _ => "NurbsCurve",
                };
                assert_eq!(kind, row["kind"].as_str().unwrap(), "{label}");
                compare(
                    &curve_json(&g.nurbs_curve_representation().unwrap().unwrap()),
                    &row["curve"],
                    label,
                );
            }
        }
    }
}

#[test]
fn native_workflows_replay_admission_metadata_preferences_cancellation_and_trim_rebuilding() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/smooth_workflow.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/smooth_workflow.json"
    ))
    .unwrap();
    let mut verified = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let label = op["id"].as_str().unwrap();
        let case = op["case"].as_str().unwrap();
        let n = &row["value"];
        let mut app = test_app();
        app.document = Document::new(Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap());
        let plane = &n["plane"];
        app.viewports[0].set_construction_plane(
            Frame3::try_from_directions(
                point_value(&plane["origin"]),
                Vector3::try_from(
                    serde_json::from_value::<[f64; 3]>(plane["x_axis"].clone()).unwrap(),
                )
                .unwrap(),
                Vector3::try_from(
                    serde_json::from_value::<[f64; 3]>(plane["y_axis"].clone()).unwrap(),
                )
                .unwrap(),
                app.document.tolerance(),
            )
            .unwrap(),
        );
        let mut ids = vec![];
        for row in n["before"].as_array().unwrap() {
            let id = app
                .document
                .add_geometry(source(row, app.document.tolerance()))
                .unwrap();
            ids.push(id);
            app.document
                .set_object_names([(id, row["name"].as_str().map(str::to_owned))])
                .unwrap();
            app.document
                .set_object_user_text([id], "Code", row["attribute_text"].as_str())
                .unwrap();
            app.document
                .set_object_geometry_user_text([id], "Code", row["geometry_text"].as_str())
                .unwrap();
            if row["grips_on"] == true {
                app.document.enable_control_points([id]).unwrap();
                app.document
                    .select_control_points(
                        row["grips"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .filter(|g| g["selected"] == true)
                            .map(|g| ControlPointId {
                                object: id,
                                index: g["index"].as_u64().unwrap() as usize,
                            }),
                        SelectionMode::Add,
                    )
                    .unwrap();
            }
            if row["selected"] == true {
                app.document
                    .select_objects_direct([id], SelectionMode::Add)
                    .unwrap();
            }
        }
        app.document.clear_history().unwrap();
        check(&app, &ids, &n["before"], label);
        enter(&mut app, "Smooth");
        match case {
            "unsupported" | "keyboard_selection" => app.cancel_current_prompt_or_selection(),
            "cancel_factor" | "cancel_steps" | "cancel_coordinates" => {
                enter(
                    &mut app,
                    match case {
                        "cancel_factor" => "SmoothFactor",
                        "cancel_steps" => "Steps",
                        _ => "CoordinateSystem",
                    },
                );
                enter(&mut app, "_Cancel");
            }
            "partial_cancel" => {
                enter(&mut app, "SmoothFactor=.4 X");
                enter(&mut app, "Steps");
                enter(&mut app, "_Cancel");
            }
            "replace" | "keyboard_escape" => {
                enter(&mut app, "SmoothFactor=.4");
                app.cancel_current_prompt_or_selection();
            }
            "keyboard_factor" => {
                enter(&mut app, "SmoothFactor");
                app.cancel_current_prompt_or_selection();
            }
            "numeric_prompt" => {
                enter(&mut app, "SmoothFactor");
                enter(&mut app, ".4");
                enter(&mut app, "Steps");
                enter(&mut app, "2");
                enter(&mut app, "");
            }
            "toggles" => {
                for option in ["X", "Y", "Z", "FixBoundaries"] {
                    enter(&mut app, option);
                }
                enter(&mut app, "");
            }
            _ => {
                let tokens = n["macro"]
                    .as_str()
                    .unwrap()
                    .split_whitespace()
                    .skip(1)
                    .take_while(|s| !["_Enter", "_Cancel"].contains(s))
                    .collect::<Vec<_>>()
                    .join(" ");
                enter(&mut app, &tokens);
                enter(&mut app, if case == "cancelled" { "_Cancel" } else { "" });
            }
        }
        assert!(
            app.object_prompt.is_none(),
            "{label}: {:?}",
            app.command_log
        );
        check(&app, &ids, &n["after"], label);
        if n["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "Smooth" && e["result"] == "Success")
        {
            enter(&mut app, "_Cancel");
            enter(&mut app, "_Cancel");
        }
        check(&app, &ids, &n["after_script"], label);
        if !n["undo"].is_null() {
            enter(&mut app, "Undo");
            check(&app, &ids, &n["undo"]["after"], label);
        }
        if !n["redo"].is_null() {
            enter(&mut app, "Redo");
            check(&app, &ids, &n["redo"]["after"], label);
        }
        if !n["followup_cancel"].is_null() {
            app.document.clear_selection();
            app.document
                .select_objects_direct(ids.iter().copied(), SelectionMode::Add)
                .unwrap();
            check(&app, &ids, &n["followup_cancel"]["before"], label);
            enter(&mut app, "Smooth");
            enter(&mut app, "_Cancel");
            check(&app, &ids, &n["followup_cancel"]["after"], label);
        }
        verified += 1;
    }
    assert_eq!(verified, 26);
}
