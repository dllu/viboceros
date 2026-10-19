//! Independent Rhino sources and Rhino readback of Viboceros rewrites.
use serde_json::{Value, json};
use viboceros_geometry::{
    BrepLoopType, CurveRef, NurbsCurve, NurbsCurve2, NurbsSurface, Tolerance,
};
use viboceros_io::{ThreeDmGeometry, read_3dm_file, write_3dm_file};

fn interval(d: std::ops::RangeInclusive<f64>) -> [f64; 2] {
    [*d.start(), *d.end()]
}
fn definition(c: &NurbsCurve) -> Value {
    json!({"degree":c.degree(),"knots":c.knots(),"domain":interval(c.domain()),
        "control_points":c.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>()})
}
fn definition2(c: &NurbsCurve2) -> Value {
    json!({"degree":c.degree(),"knots":c.knots(),"domain":interval(c.domain()),
        "control_points":c.control_points().iter().map(|p|json!({"point":[p.point().x(),p.point().y(),0.],"weight":p.weight()})).collect::<Vec<_>>()})
}
fn surface(s: &NurbsSurface) -> Value {
    json!({"degree":[s.degree_u(),s.degree_v()],"knots_u":s.knots_u(),"knots_v":s.knots_v(),
        "domain_u":interval(s.domain_u()),"domain_v":interval(s.domain_v()),
        "control_count":[s.control_point_count_u(),s.control_point_count_v()],
        "control_points":s.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>()})
}
fn curve(c: CurveRef<'_>) -> Value {
    let mut v = json!({"domain":interval(c.domain()),"closed":c.is_closed().unwrap(),
        "samples":(0..=32).map(|i|c.evaluate(c.parameter_at(i as f64/32.).unwrap()).unwrap().to_array()).collect::<Vec<_>>()});
    match c {
        CurveRef::NurbsCurve(n) => {
            v["type"] = json!("nurbs");
            v["definition"] = definition(n);
            v["rational"] = json!(n.has_rational_representation());
        }
        CurveRef::Line(_) => v["type"] = json!("line"),
        CurveRef::PolyCurve(p) => {
            v["type"] = json!("polycurve");
            v["parameters"] = json!(p.parameters());
            v["segments"] = json!(
                p.segments()
                    .iter()
                    .map(|s| curve(s.as_ref()))
                    .collect::<Vec<_>>()
            );
        }
        _ => panic!("unexpected captured curve: {c:?}"),
    }
    v
}
fn geometry(g: &ThreeDmGeometry) -> Value {
    match g {
        ThreeDmGeometry::PolyCurve(c) => {
            json!({"kind":"curve","geometry":curve(CurveRef::PolyCurve(c))})
        }
        ThreeDmGeometry::Brep(b) => json!({"kind":"brep","valid":true,"solid":b.is_solid(),
            "vertices":b.vertices().iter().map(|v|v.point().to_array()).collect::<Vec<_>>(),
            "edges":b.edges().iter().map(|e|json!({"vertices":e.vertices(),"curve":curve(CurveRef::NurbsCurve(e.curve()))})).collect::<Vec<_>>(),
            "faces":b.faces().iter().map(|f|json!({"reversed":f.is_reversed(),"definition":surface(f.surface()),
                "loops":f.loops().iter().map(|l|json!({"type":match l.loop_type(){BrepLoopType::Outer=>"Outer",BrepLoopType::Inner=>"Inner"},
                    "trims":l.trims().iter().map(|t|json!({"edge":t.edge(),"reversed":t.is_reversed_3d(),"definition":definition2(t.curve())})).collect::<Vec<_>>()
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        }),
        _ => panic!("unexpected source type"),
    }
}
fn compare(a: &Value, b: &Value, path: &str) -> f64 {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let a = a.as_f64().unwrap();
            let b = b.as_f64().unwrap();
            assert!((a - b).abs() <= 1e-12, "{path}: {a} vs {b}");
            (a - b).abs()
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}: array length");
            a.iter()
                .zip(b)
                .enumerate()
                .map(|(i, (a, b))| compare(a, b, &format!("{path}/{i}")))
                .fold(0., f64::max)
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}: keys"
            );
            a.iter()
                .map(|(k, v)| compare(v, &b[k], &format!("{path}/{k}")))
                .fold(0., f64::max)
        }
        _ => {
            assert_eq!(a, b, "{path}");
            0.
        }
    }
}

#[test]
fn native_nested_sources_preserve_storage_basis_geometry_topology_and_metadata() {
    let sources: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/nested_rational.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/nested_rational");
    let directory = tempfile::tempdir().unwrap();
    for row in sources["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let source = read_3dm_file(root.join(format!("{name}.3dm")), Tolerance::DEFAULT).unwrap();
        assert_eq!(source.unsupported_object_count(), 0, "{name}");
        assert_eq!(source.objects.len(), 1);
        let original = source.clone();
        let record = |m: &viboceros_io::ThreeDmModel| {
            let o = &m.objects[0];
            json!({"geometry":geometry(&o.geometry),"name":o.name,
                "attribute_text":o.user_text.get("ObjectCode"),"geometry_text":o.geometry_user_text.get("GeometryCode")})
        };
        let mut maximum = compare(&record(&source), &row["value"], name);
        let output = directory.path().join(format!("{name}.3dm"));
        write_3dm_file(&output, &source).unwrap();
        assert_eq!(source, original, "{name}: source mutation");
        let rewritten = read_3dm_file(output, Tolerance::DEFAULT).unwrap();
        assert_eq!(rewritten.objects.len(), 1);
        assert_eq!(rewritten.unsupported_object_count(), 0);
        maximum = maximum.max(compare(&record(&rewritten), &row["value"], name));
        eprintln!(
            "NESTED_RATIONAL_AUDIT {}",
            json!({"id":name,"maximum_error":maximum})
        );
    }
}

#[test]
fn public_rhino_readback_retains_both_polynomial_and_rational_storage() {
    let sources: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/nested_rational.json"
    ))
    .unwrap();
    let readback: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/nested_rational_readback.json"
    ))
    .unwrap();
    assert_eq!(sources["results"].as_array().unwrap().len(), 16);
    assert_eq!(readback["results"].as_array().unwrap().len(), 16);
    for (a, b) in sources["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(readback["results"].as_array().unwrap())
    {
        assert_eq!(a["id"], b["id"]);
        let maximum = compare(&a["value"], &b["value"], a["id"].as_str().unwrap());
        eprintln!(
            "NESTED_RATIONAL_READBACK {}",
            json!({"id":a["id"],"maximum_error":maximum})
        );
    }
}
