use crate::morph_fixtures as morph;
use serde_json::Value;
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{Brep, Frame3, Point3, Tolerance, Vector3};
pub fn rows() -> Vec<(Value, Value)> {
    let pairs = [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/orient_curve_edge_subcurve.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/orient_curve_edge_subcurve.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/orient_curve_edge.json"),
            include_str!("../../../../tools/rhino_oracle/observations/orient_curve_edge.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/orient_curve_edge_options.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/orient_curve_edge_options.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/orient_curve_edge_spatial.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/orient_curve_edge_spatial.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/orient_curve_edge_parallel.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/orient_curve_edge_parallel.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/orient_curve_edge_threshold.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/orient_curve_edge_threshold.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/orient_curve_edge_cutoff.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/orient_curve_edge_cutoff.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/orient_curve_edge_boundary.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/orient_curve_edge_boundary.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/orient_curve_edge_angle.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/orient_curve_edge_angle.json"
            ),
        ),
    ];
    pairs
        .into_iter()
        .flat_map(|(q, r)| {
            let q: Value = serde_json::from_str(q).unwrap();
            let r: Value = serde_json::from_str(r).unwrap();
            let ops = q["operations"].as_array().unwrap();
            let results = r["results"].as_array().unwrap();
            assert_eq!(ops.len(), results.len());
            ops.iter()
                .zip(results)
                .map(|(op, r)| {
                    assert_eq!(op["id"], r["id"]);
                    assert!(r["error"].is_null(), "{r}");
                    assert_eq!(r["value"]["input_trace"]["errors"], serde_json::json!([]));
                    (op.clone(), r["value"].clone())
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn point(v: &Value) -> Point3 {
    morph::p(v)
}
pub fn token(p: Point3) -> String {
    p.to_array().map(|v| v.to_string()).join(",")
}
pub fn plane(op: &Value) -> Frame3 {
    Frame3::try_from_directions(
        point(&op["cplane"][0]),
        Vector3::try_from(point(&op["cplane"][1]).to_array()).unwrap(),
        Vector3::try_from(point(&op["cplane"][2]).to_array()).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap()
}
pub struct Scene {
    pub source: ObjectId,
    pub target: ObjectId,
    pub edge: usize,
    pub original: Geometry,
    pub target_snapshot: viboceros_document::GeometrySnapshot,
}
pub fn setup(op: &Value, v: &Value) -> (Document, Scene) {
    let absolute = v["actual_tolerance"]["absolute"].as_f64().unwrap_or(1e-6);
    let angular = v["actual_tolerance"]["angular"]
        .as_f64()
        .or_else(|| op["angle_tolerance"].as_f64())
        .unwrap_or(1e-6);
    let mut doc = Document::new(Tolerance::try_new(absolute, 1e-12, angular).unwrap());
    let original = Geometry::NurbsCurve(morph::curve(&v["before"][0]["geometry"]["curve"]));
    let source = doc.add_geometry(original.clone()).unwrap();
    let b =
        Brep::try_surface_face(morph::surface(&v["target_definition"]), doc.tolerance()).unwrap();
    let b = if v["target_reversed"] == true {
        b.reversed()
    } else {
        b
    };
    let target = doc.add_geometry(Geometry::Brep(b.clone())).unwrap();
    let index = op["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|a| a.get("edge"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let native = morph::curve(&v["edges"][index as usize]);
    let edge = b
        .edges()
        .iter()
        .position(|e| {
            [0., 0.25, 0.5, 0.75, 1.].into_iter().all(|f| {
                let d = native.domain();
                let p = native
                    .evaluate(d.start() + f * (d.end() - d.start()))
                    .unwrap();
                let t = e.curve().closest_parameter(p, doc.tolerance()).unwrap();
                e.curve().evaluate(t).unwrap().distance_to(p).unwrap() < 1e-10
            })
        })
        .expect("matching authored boundary");
    if op["grouped"] == true {
        doc.add_group(Some("Curve edge source group".into()), [source])
            .unwrap();
    }
    doc.set_object_names([
        (source, Some("Source".into())),
        (target, Some("Target".into())),
    ])
    .unwrap();
    doc.clear_history().unwrap();
    let target_snapshot = doc.object(target).unwrap().geometry_snapshot().clone();
    (
        doc,
        Scene {
            source,
            target,
            edge,
            original,
            target_snapshot,
        },
    )
}
pub fn check(doc: &Document, scene: &Scene, v: &Value, phase: &str) -> f64 {
    let expected = v[phase].as_array().unwrap();
    let actual = doc.objects().collect::<Vec<_>>();
    assert_eq!(actual.len(), expected.len(), "{phase}");
    let mut max = 0f64;
    for (a, b) in actual.iter().zip(expected) {
        assert_eq!(a.attributes().name(), b["name"].as_str());
        assert_eq!(doc.is_selected(a.id()), b["selected"].as_bool().unwrap());
        if b["role"] == "target" {
            assert_eq!(a.id(), scene.target);
            assert!(
                scene
                    .target_snapshot
                    .shares_storage_with(a.geometry_snapshot())
            );
            continue;
        }
        if b["role"] == "source" {
            assert_eq!(a.id(), scene.source);
        }
        if let Some(groups) = b["groups"].as_array() {
            let names = a
                .group_ids()
                .iter()
                .map(|g| doc.group(*g).unwrap().name().unwrap_or(""))
                .collect::<Vec<_>>();
            assert_eq!(
                names,
                groups
                    .iter()
                    .map(|g| g.as_str().unwrap())
                    .collect::<Vec<_>>()
            );
        }
        let curve = a.geometry().nurbs_curve_representation().unwrap().unwrap();
        let e = morph::curve(&b["geometry"]["curve"]);
        assert_eq!(curve.degree(), e.degree());
        assert_eq!(curve.control_points().len(), e.control_points().len());
        assert_eq!(curve.knots().len(), e.knots().len());
        for (a, b) in curve.knots().iter().zip(e.knots()) {
            assert!((a - b).abs() <= 1e-8 * (1. + b.abs()), "knot {a} != {b}");
        }
        for (a, b) in curve.control_points().iter().zip(e.control_points()) {
            assert_eq!(a.weight(), b.weight());
            max = max.max(a.point().distance_to(b.point()).unwrap());
        }
    }
    if let Some(table) = v["groups"][phase].as_array() {
        let actual = doc
            .groups()
            .map(|g| g.name().unwrap_or(""))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            table
                .iter()
                .map(|g| g["name"].as_str().unwrap())
                .collect::<Vec<_>>()
        );
    }
    max
}
pub fn execute(doc: &mut Document, scene: &Scene, commands: &CommandRegistry, op: &Value) {
    let mut group = doc.begin_history_group("OrientCrvToEdge").unwrap();
    let pick = op["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|a| a.get("pick").filter(|_| a["curve"] == 0))
        .map(point);
    let mut options = viboceros_command::orient_curve_edge::Options::default();
    let range = source_range(op, scene.original.curve_ref().unwrap(), doc.tolerance());
    let range = range.map_or_else(String::new, |[start, end]| format!(" SubCrv={start},{end}"));
    for (i, action) in op["actions"].as_array().unwrap().iter().enumerate() {
        if let Some(s) = action.as_str() {
            if s == "Cancel" || s == "Enter" {
                break;
            }
            let normalized = match s {
                "FlipSurface" => format!(
                    "FlipSurface={}",
                    if options.flip_surface { "No" } else { "Yes" }
                ),
                "ReverseCurve" => format!(
                    "ReverseCurve={}",
                    if options.reverse_curve { "No" } else { "Yes" }
                ),
                _ => s.into(),
            };
            let (_, o, _) =
                viboceros_command::orient_curve_edge::options(&[&normalized], options).unwrap();
            options = o;
        } else if action.is_array() && op["action_prompts"][i][0] == "Pick target edge point" {
            let pick = pick.map_or_else(String::new, |p| format!(" PickPoint={}", token(p)));
            commands
                .execute_in_history_group(
                    doc,
                    &format!(
                        "OrientCrvToEdge {} Edge={},{} {} PickedSources={}{}{}",
                        token(point(action)),
                        scene.target,
                        scene.edge,
                        options.arguments(),
                        scene.source,
                        pick,
                        range
                    ),
                    CommandContext {
                        construction_plane: plane(op),
                    },
                    &mut group,
                )
                .unwrap();
        }
    }
    doc.clear_selection();
}

/// Replay authored SubCrv inputs through the shared geometric getter policy.
/// Expected output geometry is never used to supply source parameters.
pub fn source_range(
    op: &Value,
    curve: viboceros_geometry::CurveRef<'_>,
    tol: Tolerance,
) -> Option<[f64; 2]> {
    use viboceros_command::subcurve_input as sub;
    let mut start = None;
    let mut hover = None;
    let mut locked = None;
    let mut length = None;
    for (i, a) in op["actions"].as_array().unwrap().iter().enumerate() {
        if a == "Cancel" || a == "Enter" {
            return None;
        }
        let gate = op["action_prompts"][i][0].as_str().unwrap();
        if gate == "Start of curve" && a.is_array() {
            start = Some(curve.closest_parameter(point(a), tol).unwrap());
        } else if gate == "Current length" || gate == "Select curve" {
            let start = start.unwrap();
            if let Some(p) = a.get("hover") {
                hover = Some(curve.closest_parameter(point(p), tol).unwrap());
            } else if let Some(s) = a.as_str() {
                let value = sub::direction_option(s).unwrap().unwrap();
                locked = value.then(|| sub::cursor_forward(curve, start, hover.unwrap()).unwrap());
            } else if let Some(n) = a.as_f64() {
                if n == 0. {
                    return None;
                }
                length = Some(n.abs());
                if let Some(forward) = locked {
                    if !sub::locked_numeric_requires_confirmation(
                        curve,
                        start,
                        n.abs(),
                        forward,
                        tol,
                    )
                    .unwrap()
                    {
                        return sub::interval_in_direction(curve, start, n.abs(), forward, tol)
                            .unwrap();
                    }
                    locked = None;
                }
            } else if a.is_array() {
                let t = curve.closest_parameter(point(a), tol).unwrap();
                return if let Some(length) = length {
                    sub::interval(curve, start, t, length, tol).unwrap()
                } else {
                    sub::point_parameters(curve, start, t, locked, false).unwrap()
                };
            }
        }
    }
    None
}
