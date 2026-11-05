use viboceros_geometry::*;

fn point(v: &serde_json::Value) -> Point3 {
    Point3::try_from(std::array::from_fn(|i| v[i].as_f64().unwrap())).unwrap()
}
fn profile(v: &serde_json::Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        v["knots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_f64().unwrap())
            .collect(),
    )
    .unwrap()
}

#[test]
fn microscopic_spans_preserve_meshes_normals_reversal_and_native_geometry() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/short_span_extrusions.json"
    ))
    .unwrap();
    let operations = request["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 16);
    for op in operations {
        let source = profile(&op["curve"]);
        let height = op["height"].as_f64().unwrap();
        let xy = op["id"].as_str().unwrap().starts_with("xy_");
        let end = Vector3::try_from(if xy {
            [0., 0., height]
        } else {
            [0., height, 0.]
        })
        .unwrap();
        let start = if op["both_sides"] == true {
            end.scaled(-1.).unwrap()
        } else {
            Vector3::try_new(0., 0., 0.).unwrap()
        };
        for reversed in [false, true] {
            let curve = if reversed {
                source.reversed().unwrap()
            } else {
                source.clone()
            };
            let typed =
                Brep::try_command_curve_extrusion(&curve, start, end, false, Tolerance::DEFAULT)
                    .unwrap()
                    .unwrap();
            let before = typed.clone();
            let archive = typed.extrusion().unwrap().archive().to_vec();
            for explicit in [false, true] {
                let brep = if explicit {
                    typed.try_extrusion_brep_form(Tolerance::DEFAULT).unwrap()
                } else {
                    typed.clone()
                };
                let label = format!("{} reversed={reversed} explicit={explicit}", op["id"]);
                let mesh = brep
                    .tessellate(8, Tolerance::DEFAULT)
                    .unwrap_or_else(|e| panic!("{label}: {e}"));
                let peak = if xy {
                    mesh.bounds().max().y()
                } else {
                    mesh.bounds().max().z()
                };
                assert!((peak - 10. / 3.).abs() < 1e-12, "{label}: peak {peak}");
                assert!(mesh.topology().is_oriented(), "{label}");
                let rendered = brep
                    .tessellate_with_normals(8, Tolerance::DEFAULT)
                    .unwrap_or_else(|e| panic!("{label}: {e}"));
                assert_eq!(rendered.mesh.bounds(), mesh.bounds(), "{label}");
                assert!(
                    rendered
                        .corners
                        .iter()
                        .flatten()
                        .all(|c| c.normal.is_some()),
                    "{label}"
                );
                let display = brep.display_mesh(8, Tolerance::DEFAULT).unwrap();
                assert_eq!(display.bounds(), mesh.bounds(), "{label}");
            }
            assert_eq!(typed, before);
            assert_eq!(typed.extrusion().unwrap().archive(), archive);
        }
    }
}
