//! Native grip definitions and complete geometry/display comparisons.
use serde_json::{Value, json};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{
    Brep, Circle3, MeshFace, NurbsCurve, NurbsSurface, Point3, Polyline3, Real, Tolerance,
    TriangleMesh, WeightedPoint3,
};
fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn p(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}
fn controls(value: &Value) -> Vec<WeightedPoint3> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| WeightedPoint3::try_new(p(&v["point"]), v["weight"].as_f64().unwrap()).unwrap())
        .collect()
}
fn floats(value: &Value) -> Vec<f64> {
    serde_json::from_value(value.clone()).unwrap()
}
pub fn geometry(op: &Value, native: &Value) -> Geometry {
    let kind = op["source"].as_str().unwrap();
    if kind == "circle" || kind == "arc" {
        let circle = Circle3::try_new(
            point(0., 0., 0.),
            2.,
            viboceros_geometry::Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized(Tolerance::DEFAULT)
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        return if kind == "circle" {
            Geometry::Circle(circle)
        } else {
            Geometry::Arc(
                viboceros_geometry::CircularArc3::try_from_circle_sweep(
                    circle,
                    std::f64::consts::PI,
                )
                .unwrap(),
            )
        };
    }
    if kind.starts_with("surface") {
        let s = &native["surface"];
        let surface = NurbsSurface::try_new_rational(
            s["degree"][0].as_u64().unwrap() as usize,
            s["degree"][1].as_u64().unwrap() as usize,
            s["control_count"][0].as_u64().unwrap() as usize,
            s["control_count"][1].as_u64().unwrap() as usize,
            controls(&s["control_points"]),
            floats(&s["knots_u"]),
            floats(&s["knots_v"]),
        )
        .unwrap();
        return Geometry::Brep(Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap());
    }
    if kind == "mesh" {
        let m = &native["mesh"];
        let faces = m["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let a = v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>();
                match *a.as_slice() {
                    [a, b, c] => MeshFace::Triangle([a, b, c]),
                    [a, b, c, d] => MeshFace::Quad([a, b, c, d]),
                    _ => unreachable!(),
                }
            })
            .collect();
        return Geometry::Mesh(
            TriangleMesh::try_new_faces(
                m["vertices"].as_array().unwrap().iter().map(p).collect(),
                faces,
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
    }
    let c = &native["curve"];
    let controls = controls(&c["control_points"]);
    if kind == "line" {
        return Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                controls[0].point(),
                controls[1].point(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
    }
    if kind == "polyline" {
        return Geometry::Polyline(
            Polyline3::try_new(
                controls.iter().map(|p| p.point()).collect(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
    }
    Geometry::NurbsCurve(
        NurbsCurve::try_new_rational(
            c["degree"].as_u64().unwrap() as usize,
            controls,
            floats(&c["knots"]),
        )
        .unwrap(),
    )
}
fn control_json(points: &[WeightedPoint3]) -> Value {
    json!(
        points
            .iter()
            .map(|p| json!({"point":p.point().to_array(), "weight":p.weight()}))
            .collect::<Vec<_>>()
    )
}
pub fn snapshot(doc: &Document, source: ObjectId, extra: Option<ObjectId>) -> Value {
    json!(doc.objects().map(|o| {
        let locations = doc.control_point_locations(o.id());
        let mut row = json!({"role":if o.id()==source {"source"} else if Some(o.id())==extra {"point"} else {"output"},
            "selected":doc.is_selected(o.id()), "name":o.attributes().name(),
            "grips_on":locations.is_some(), "grips":doc.control_points().filter(|(id,_,_)|id.object==o.id())
                .map(|(id,pick,selected)|json!({"index":id.index,"point":pick.to_array(),"selected":selected})).collect::<Vec<_>>()});
        match o.geometry() {
            Geometry::Brep(b) => {
                let s = b.faces()[0].surface(); row["kind"]=json!("Brep");
                row["surface"]=json!({"degree":[s.degree_u(),s.degree_v()], "control_count":[s.control_point_count_u(),s.control_point_count_v()],
                    "control_points":control_json(s.control_points()),"knots_u":s.knots_u(),"knots_v":s.knots_v(),
                    "domain_u":[*s.domain_u().start(),*s.domain_u().end()],"domain_v":[*s.domain_v().start(),*s.domain_v().end()]});
            }
            Geometry::Mesh(m) => { row["kind"]=json!("Mesh");row["mesh"]=json!({"vertices":m.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),
                "faces":m.faces().iter().map(|f|f.indices().to_vec()).collect::<Vec<_>>()}); }
            Geometry::Point(p) => { row["kind"]=json!("Point");row["point"]=json!(p.to_array()); }
            geometry => {
                row["kind"]=json!(match geometry { Geometry::Line(_)=>"LineCurve", Geometry::Polyline(_)=>"PolylineCurve",
                    Geometry::Circle(_)|Geometry::Arc(_)=>"ArcCurve", Geometry::PolyCurve(_)=>"PolyCurve", _=>"NurbsCurve" });
                let c = geometry.nurbs_curve_representation().unwrap().unwrap();
                row["curve"]=json!({"degree":c.degree(),"control_points":control_json(c.control_points()),"knots":c.knots(),
                    "domain":[*c.domain().start(),*c.domain().end()]});
            }
        }
        row
    }).collect::<Vec<_>>())
}
pub fn compare(actual: &Value, expected: &Value, context: &str) {
    compare_error(actual, expected, context);
}
pub fn compare_error(actual: &Value, expected: &Value, context: &str) -> Real {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            let gap = (a.as_f64().unwrap() - b.as_f64().unwrap()).abs();
            assert!(gap < 1e-9, "{context}: {actual} vs {expected}");
            gap
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{context}");
            let mut worst = 0_f64;
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                worst = worst.max(compare_error(a, b, &format!("{context}/{i}")));
            }
            worst
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{context}"
            );
            b.iter()
                .map(|(k, b)| compare_error(&a[k], b, &format!("{context}/{k}")))
                .fold(0_f64, Real::max)
        }
        _ => {
            assert_eq!(actual, expected, "{context}");
            0.
        }
    }
}
