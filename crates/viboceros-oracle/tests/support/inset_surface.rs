//! Shared spatial and incidence comparisons for planar inset evidence.
use super::common;
use serde_json::Value;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::{Brep, LineSegment, NurbsCurve, Point3, Tolerance, WeightedPoint3};

#[path = "brep_archive.rs"]
mod archive;
pub use archive::source;
pub fn fixture(record: &Value) -> (Document, ObjectId) {
    let mut doc = Document::default();
    doc.set_tolerance(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    let layer = doc
        .add_layer("Inset surface source", ColorRgb::new(0, 0, 255))
        .unwrap();
    let id = doc
        .add_geometry_with_attributes(
            Geometry::Brep(source(record)),
            ObjectAttributes::on_layer(layer)
                .with_name("Input")
                .with_object_color(ColorRgb::new(17, 43, 91)),
        )
        .unwrap();
    doc.add_group(Some("Inset surface group".into()), [id])
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
pub fn assert_metadata(doc: &Document, id: ObjectId, native: &Value) {
    assert_eq!(doc.objects().len(), 1);
    let obj = doc.object(id).unwrap();
    let a = obj.attributes();
    let c = a.object_color();
    let actual = serde_json::json!({"source":0,"selected":doc.is_selected(id),
        "name":a.name(),"layer":doc.layer(a.layer_id()).map(|l| l.name()),
        "color":[c.red,c.green,c.blue],
        "groups":obj.group_ids().iter().map(|&g| doc.group(g).unwrap().name()).collect::<Vec<_>>()});
    let expected = serde_json::json!({"source":native["source"],"selected":native["selected"],
        "name":native["name"],"layer":if native["layer"] == 0 { Some("Inset surface source") } else { None },
        "color":native["color"],"groups":native["groups"].as_array().unwrap().iter().map(|g| {
            assert_eq!(g,0); Some("Inset surface group")
        }).collect::<Vec<_>>()});
    common::compare(&actual, &expected, "surface inset metadata");
}
fn point(v: &Value) -> Point3 {
    Point3::try_new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
    .unwrap()
}
fn cycle(mut vertices: Vec<usize>) -> Vec<usize> {
    let pivot = vertices
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| *v)
        .unwrap()
        .0;
    vertices.rotate_left(pivot);
    vertices
}
fn topology(brep: &Brep, mapping: &[usize]) -> Value {
    let mut edges = brep
        .edges()
        .iter()
        .map(|e| {
            let mut ends = e.vertices().map(|v| mapping[v]);
            ends.sort();
            ends
        })
        .collect::<Vec<_>>();
    edges.sort();
    let mut faces = brep
        .faces()
        .iter()
        .map(|f| {
            let mut loops = f
                .loops()
                .iter()
                .map(|l| cycle(l.trims().iter().map(|t| mapping[t.vertices()[0]]).collect()))
                .collect::<Vec<_>>();
            loops[1..].sort();
            (f.is_reversed(), loops)
        })
        .collect::<Vec<_>>();
    faces.sort();
    serde_json::json!({"edges": edges, "faces": faces, "solid":brep.is_solid()})
}
fn native_topology(record: &Value) -> Value {
    let mut edges = record["topology"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let mut ends = [
                e[0].as_u64().unwrap() as usize,
                e[1].as_u64().unwrap() as usize,
            ];
            ends.sort();
            ends
        })
        .collect::<Vec<_>>();
    edges.sort();
    let mut faces = record["topology"]["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let mut loops = f["loops"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| {
                    cycle(
                        l["trims"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|t| t["vertices"][0].as_u64().unwrap() as usize)
                            .collect(),
                    )
                })
                .collect::<Vec<_>>();
            loops[1..].sort();
            (f["reversed"].as_bool().unwrap(), loops)
        })
        .collect::<Vec<_>>();
    faces.sort();
    serde_json::json!({"edges":edges,"faces":faces,"solid":record["topology"]["solid"]})
}

/// Clocks and raw table order differ; match vertices bijectively, then compare
/// every oriented face loop and every edge endpoint pair. Spatial station
/// checks use those corresponding edges, never nearest unrelated geometry.
pub fn assert_geometry(output: &Brep, expected: &Value, tolerance: Tolerance, id: &str) {
    assert_geometry_with_epsilon(output, expected, tolerance, 1e-10, id)
}
pub fn epsilon_for(operation: &Value) -> f64 {
    if matches!(
        operation["op"].as_str(),
        Some("inset_cusp_sdk" | "inset_cusp_command")
    ) && operation["scene"] == "ellipse"
        && operation["distance"] == 3.99
    {
        // Near collapse, the native fitted-locus intersection is 1.2625e-5
        // from the analytic ellipse tip. The independent kernel stays analytic.
        return 1.5e-5;
    }
    if operation["loose"] == true {
        return 1e-10;
    }
    if matches!(
        operation["op"].as_str(),
        Some(
            "inset_continuity_sdk"
                | "inset_continuity_command"
                | "inset_continuity_limits"
                | "inset_fan_basis"
                | "inset_fan_interior"
                | "inset_fan_commands"
                | "inset_closed_network"
                | "inset_closed_network_faces"
                | "inset_closed_seams"
        )
    ) || operation["op"] == "inset_collinear_sdk" && operation["scene"] == "elliptic_stadium"
    {
        return 2e-6;
    }
    if matches!(
        operation["scene"].as_str().unwrap(),
        "ellipse" | "oval" | "concave_oval" | "bezier" | "holes"
    ) {
        // Independent fits can consume both model-tolerance allowances.
        if matches!(
            operation["op"].as_str().unwrap(),
            "inset_planar_curves_command"
                | "inset_output_modes"
                | "inset_output_preferences"
                | "inset_output_limits"
                | "inset_cusp_command"
                | "inset_cusp_sdk"
                | "inset_multiedge_sdk"
                | "inset_multiedge_command"
        ) {
            2e-6
        } else {
            1e-6
        }
    } else {
        1e-10
    }
}
pub fn nurbs_definition(definition: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        definition["degree"].as_u64().unwrap() as usize,
        definition["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(point(&c["point"]), c["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        definition["knots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_f64().unwrap())
            .collect(),
    )
    .unwrap()
}
pub fn assert_geometry_with_epsilon(
    output: &Brep,
    expected: &Value,
    tolerance: Tolerance,
    epsilon: f64,
    id: &str,
) {
    let vertices = expected["vertices"].as_array().unwrap();
    assert_eq!(output.vertices().len(), vertices.len(), "{id}");
    let mapping = output
        .vertices()
        .iter()
        .map(|v| {
            vertices
                .iter()
                .position(|w| v.point().distance_to(point(&w["point"])).unwrap() <= epsilon)
                .unwrap_or_else(|| panic!("{id}: unmatched vertex {:?}", v.point()))
        })
        .collect::<Vec<_>>();
    let mut distinct = mapping.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        mapping.len(),
        "{id}: bijective vertex correspondence"
    );
    common::compare(&topology(output, &mapping), &native_topology(expected), id);
    let mut used = std::collections::BTreeSet::new();
    for (i, edge) in expected["edges"].as_array().unwrap().iter().enumerate() {
        let ends = &expected["topology"]["edges"][i];
        let ends = [
            ends[0].as_u64().unwrap() as usize,
            ends[1].as_u64().unwrap() as usize,
        ];
        let candidates = output
            .edges()
            .iter()
            .enumerate()
            .filter(|(index, e)| {
                let mapped = e.vertices().map(|v| mapping[v]);
                !used.contains(index) && (mapped == ends || mapped == [ends[1], ends[0]])
            })
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        // Two complementary arcs can have the same endpoint pair. Match their
        // loci bijectively instead of selecting the first table entry twice.
        let index = if let [index] = candidates.as_slice() {
            *index
        } else {
            let native = nurbs_definition(&edge["curve"]["definition"]);
            let index = candidates
                .iter()
                .copied()
                .find(|&i| {
                    let actual = output.edges()[i].curve();
                    curve_witnesses(actual, &native, epsilon).is_ok()
                        && curve_witnesses(&native, actual, epsilon).is_ok()
                })
                .unwrap_or_else(|| panic!("{id}: no corresponding unused curve for edge {i}"));
            used.insert(index);
            continue; // Both directions were already checked above.
        };
        used.insert(index);
        let actual = &output.edges()[index];
        if actual.curve().degree() == 1 && actual.curve().control_points().len() == 2 {
            let ends = actual.vertices().map(|v| output.vertices()[v].point());
            let line = LineSegment::try_new(ends[0], ends[1], tolerance).unwrap();
            for p in edge["curve"]["samples"].as_array().unwrap() {
                let p = point(p);
                let distance = line
                    .closest_point(p, tolerance)
                    .unwrap()
                    .distance_to(p)
                    .unwrap();
                assert!(distance <= epsilon, "{id}: edge witness {distance}");
            }
        } else {
            let definition = &edge["curve"]["definition"];
            let native = nurbs_definition(definition);
            curve_witnesses(actual.curve(), &native, epsilon)
                .unwrap_or_else(|e| panic!("{id}: {e}"));
            curve_witnesses(&native, actual.curve(), epsilon)
                .unwrap_or_else(|e| panic!("{id}: {e}"));
        }
    }
}
pub fn curve_witnesses(
    source: &NurbsCurve,
    target: &NurbsCurve,
    epsilon: f64,
) -> Result<(), String> {
    let reverse = source
        .derivative_at(*source.domain().start())
        .unwrap()
        .dot(target.derivative_at(*target.domain().start()).unwrap())
        .unwrap()
        < 0.;
    // This needs a point within epsilon, not the exact minimum of two
    // independently fitted curves. Keep the final physical-distance check.
    let query = Tolerance::try_new(epsilon * 0.75, 1e-14, 1e-10).unwrap();
    let domain = source.domain();
    for (a, b) in source.spans() {
        for i in 0..=4 {
            let t = match i {
                0 => a,
                4 => b,
                _ => (a + (b - a) * (i as f64 / 4.)).clamp(a, b),
            };
            let p = source.evaluate(t).unwrap();
            let fraction = (t - domain.start()) / (domain.end() - domain.start());
            let seed = target
                .parameter_at(if reverse { 1. - fraction } else { fraction })
                .unwrap();
            let parameter = target
                .point_getter_parameter_from_seed(p, seed, query)
                .unwrap();
            let mut distance = target.evaluate(parameter).unwrap().distance_to(p).unwrap();
            if distance > epsilon {
                let global = target.closest_parameter(p, query).unwrap();
                distance = target.evaluate(global).unwrap().distance_to(p).unwrap();
            }
            if distance > epsilon {
                return Err(format!(
                    "corresponding curve witness {distance} exceeds {epsilon}"
                ));
            }
        }
    }
    Ok(())
}
