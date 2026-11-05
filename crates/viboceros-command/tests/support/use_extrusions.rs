//! Shared native creator replay for the registry and the headless app harness.
use serde_json::Value;
use std::io::Read;
use viboceros_command::CommandContext;
use viboceros_document::{Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::{
    Brep, Frame3, LineSegment, NurbsCurve, NurbsSurface, Point3, Polyline3, Tolerance, Vector3,
    WeightedPoint3,
};

pub fn read(family: &str) -> Value {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // This module is compiled from both workspace packages.
    let base = if base.join("tools/rhino_oracle").is_dir() {
        base.to_path_buf()
    } else {
        base.join("../..")
    };
    let path = base.join(format!("tools/rhino_oracle/observations/{family}.json"));
    let bytes = if path.exists() {
        std::fs::read(path).unwrap()
    } else {
        let mut data = Vec::new();
        flate2::read::GzDecoder::new(std::fs::File::open(path.with_extension("json.gz")).unwrap())
            .read_to_end(&mut data)
            .unwrap();
        data
    };
    serde_json::from_slice(&bytes).unwrap()
}
pub fn point(v: &Value) -> Point3 {
    Point3::try_from(std::array::from_fn(|i| v[i].as_f64().unwrap())).unwrap()
}
fn numbers(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| {
            WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap()).unwrap()
        })
        .collect()
}
pub fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        numbers(&v["knots"]),
    )
    .unwrap()
}
pub fn surface(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        numbers(&v["knots_u"]),
        numbers(&v["knots_v"]),
    )
    .unwrap()
}
pub fn setup(value: &Value) -> (Document, Option<ObjectId>, CommandContext) {
    let mut doc = Document::default();
    let input = &value["input"];
    let source = value["before"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["source"] == true)
        .map(|row| {
            let geometry = if row["kind"] == "curve" {
                let c = curve(&row["curve"]);
                match input["rail"].as_str() {
                    Some("line" | "reverse" | "oblique") => Geometry::Line(
                        LineSegment::try_new(
                            c.evaluate(*c.domain().start()).unwrap(),
                            c.evaluate(*c.domain().end()).unwrap(),
                            doc.tolerance(),
                        )
                        .unwrap(),
                    ),
                    Some("polyline") => Geometry::Polyline(
                        Polyline3::try_new(
                            c.control_points().iter().map(|p| p.point()).collect(),
                            doc.tolerance(),
                        )
                        .unwrap(),
                    ),
                    _ => Geometry::NurbsCurve(c),
                }
            } else {
                Geometry::NurbsSurface(surface(&row["geometry"]["faces"][0]["definition"]))
            };
            let id = doc.add_geometry(geometry).unwrap();
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            id
        });
    doc.clear_history().unwrap();
    let context = if let Some(p) = input["construction_plane"].as_array() {
        CommandContext {
            construction_plane: Frame3::try_from_x_and_normal(
                point(&p[0]),
                Vector3::try_from(point(&p[1]).to_array()).unwrap(),
                Vector3::try_from(point(&p[1]).to_array())
                    .unwrap()
                    .cross(Vector3::try_from(point(&p[2]).to_array()).unwrap())
                    .unwrap(),
                doc.tolerance(),
            )
            .unwrap(),
        }
    } else {
        CommandContext::default()
    };
    (doc, source, context)
}
fn distance_to_curves(p: Point3, curves: &[&NurbsCurve]) -> f64 {
    curves
        .iter()
        .map(|c| {
            let t = c.closest_parameter(p, Tolerance::DEFAULT).unwrap();
            p.distance_to(c.evaluate(t).unwrap()).unwrap()
        })
        .fold(f64::INFINITY, f64::min)
}
fn boundaries_error(a: &[&NurbsCurve], b: &[&NurbsCurve]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return if a.len() == b.len() {
            0.
        } else {
            f64::INFINITY
        };
    }
    [(a, b), (b, a)]
        .into_iter()
        .flat_map(|(from, to)| {
            from.iter().flat_map(move |c| {
                (0..=16).map(move |i| {
                    distance_to_curves(
                        c.evaluate(c.parameter_at(i as f64 / 16.).unwrap()).unwrap(),
                        to,
                    )
                })
            })
        })
        .fold(0., f64::max)
}
fn eval(s: &NurbsSurface, u: f64, v: f64) -> Point3 {
    let a = s.domain_u();
    let b = s.domain_v();
    s.evaluate(
        (a.start() + (a.end() - a.start()) * u).clamp(*a.start(), *a.end()),
        (b.start() + (b.end() - b.start()) * v).clamp(*b.start(), *b.end()),
    )
    .unwrap()
}
fn plane(s: &NurbsSurface) -> Option<(Point3, Vector3)> {
    let points = s
        .control_points()
        .iter()
        .map(|p| p.point())
        .collect::<Vec<_>>();
    let origin = points[0];
    let x = origin.vector_to(eval(s, 1., 0.)).unwrap();
    let y = origin.vector_to(eval(s, 0., 1.)).unwrap();
    let normal = x.cross(y).unwrap().normalized_nonzero().ok()?.as_vector();
    points
        .iter()
        .all(|p| origin.vector_to(*p).unwrap().dot(normal).unwrap().abs() < 1e-10)
        .then_some((origin, normal))
}
fn surface_error(a: &NurbsSurface, b: &NurbsSurface) -> f64 {
    if let (Some((pa, na)), Some((pb, nb))) = (plane(a), plane(b)) {
        return na
            .cross(nb)
            .unwrap()
            .length()
            .unwrap()
            .max(pa.vector_to(pb).unwrap().dot(na).unwrap().abs());
    }
    let mut best = f64::INFINITY;
    for transpose in [false, true] {
        for ru in [false, true] {
            for rv in [false, true] {
                let mut error = 0_f64;
                for i in 0..=8 {
                    for j in 0..=8 {
                        let (u, v) = (i as f64 / 8., j as f64 / 8.);
                        let (x, y) = if transpose { (v, u) } else { (u, v) };
                        error = error.max(
                            eval(a, u, v)
                                .distance_to(eval(
                                    b,
                                    if ru { 1. - x } else { x },
                                    if rv { 1. - y } else { y },
                                ))
                                .unwrap(),
                        );
                    }
                }
                best = best.min(error);
            }
        }
    }
    best
}
/// Faces are paired geometrically: explicit Rhino creators may reorder faces,
/// reverse parameter axes, elevate the rail degree, and resize cap supports.
/// Every paired face must have the same support and both boundary curve sets.
fn check_brep(actual: &Brep, expected: &Value, label: &str) {
    let faces = expected["faces"].as_array().unwrap();
    assert_eq!(actual.faces().len(), faces.len(), "{label} faces");
    assert_eq!(
        actual.is_closed(),
        expected["topology"]["solid"].as_bool().unwrap(),
        "{label} solid"
    );
    let edges = expected["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| curve(&e["curve"]["definition"]))
        .collect::<Vec<_>>();
    let mut used = vec![false; actual.faces().len()];
    for (index, f) in faces.iter().enumerate() {
        let target = surface(&f["definition"]);
        let native = &expected["topology"]["faces"][index]["loops"];
        let target_edges = native
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|l| l["trims"].as_array().unwrap())
            .filter_map(|t| t["edge"].as_u64())
            .map(|i| &edges[i as usize])
            .collect::<Vec<_>>();
        let best = actual
            .faces()
            .iter()
            .enumerate()
            .filter(|(i, face)| !used[*i] && face.loops().len() == native.as_array().unwrap().len())
            .map(|(i, face)| {
                let error = surface_error(face.surface(), &target);
                let curves = face
                    .loops()
                    .iter()
                    .flat_map(|l| l.trims())
                    .filter_map(|t| t.edge())
                    .map(|e| actual.edges()[e].curve())
                    .collect::<Vec<_>>();
                (
                    i,
                    if error < 1e-6 {
                        error.max(boundaries_error(&curves, &target_edges))
                    } else {
                        error
                    },
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert!(best.1 < 1e-6, "{label} face {index}: {}", best.1);
        used[best.0] = true;
    }
}
pub fn check(doc: &Document, source: Option<ObjectId>, value: &Value, label: &str) {
    if let Some(id) = source {
        let row = value["after"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["source"] == true)
            .unwrap();
        assert_eq!(
            doc.selected_object_ids().any(|key| key == id),
            row["selected"] != 0,
            "{label} source selection"
        );
    }
    let outputs = doc
        .objects()
        .filter(|o| Some(o.id()) != source)
        .collect::<Vec<_>>();
    let expected = value["after"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["source"] == false)
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), expected.len(), "{label} objects");
    for (object, target) in outputs.into_iter().zip(expected) {
        let b = &target["geometry"];
        let actual = match object.geometry() {
            Geometry::Brep(b) => b.clone(),
            Geometry::NurbsSurface(s) => {
                Brep::try_surface_face(s.clone(), doc.tolerance()).unwrap()
            }
            _ => panic!("{label}: unexpected geometry"),
        };
        assert_eq!(
            actual.is_extrusion(),
            target["kind"] == "extrusion",
            "{label} type"
        );
        assert_eq!(
            doc.selected_object_ids().any(|id| id == object.id()),
            target["selected"] != 0,
            "{label} selection"
        );
        check_brep(&actual, b, label);
        if actual.is_extrusion() && value["input"]["command"] != "Box" {
            let form = actual.extrusion().unwrap().nurbs_form().unwrap();
            let expected = surface(&target["extrusion"]["surface"]);
            assert_eq!(
                form.caps,
                target["extrusion"]["caps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_bool().unwrap())
                    .collect::<Vec<_>>()
                    .as_slice()
            );
            assert_eq!(
                [form.surface.degree_u(), form.surface.degree_v()],
                [expected.degree_u(), expected.degree_v()],
                "{label}"
            );
            assert_eq!(
                form.surface.control_points().len(),
                expected.control_points().len(),
                "{label}"
            );
            for (a, b) in form
                .surface
                .control_points()
                .iter()
                .zip(expected.control_points())
            {
                assert!(
                    a.point().distance_to(b.point()).unwrap() < 1e-6,
                    "{label} extrusion control"
                );
                assert!(
                    (a.weight() - b.weight()).abs() < 1e-12,
                    "{label} extrusion weight"
                );
            }
            for (a, b) in [
                (form.surface.knots_u(), expected.knots_u()),
                (form.surface.knots_v(), expected.knots_v()),
            ] {
                assert_eq!(a.len(), b.len(), "{label}");
                for (x, y) in a.iter().zip(b) {
                    assert!((x - y).abs() < 1e-12, "{label} extrusion knot {x} {y}");
                }
            }
        }
    }
}
