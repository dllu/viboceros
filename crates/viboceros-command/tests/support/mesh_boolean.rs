//! Saved native mesh Boolean scenes and per-object metadata/surface comparison.
use super::{frozen_scene, mesh_reference as reference};
pub use frozen_scene::Scene;
use serde_json::{Value, json};
use viboceros_document::{Geometry, SelectionMode};
use viboceros_geometry::TriangleMesh;
#[path = "../../../viboceros-smlib/tests/support/mesh_boolean.rs"]
mod coverage;

pub fn rows() -> Vec<Value> {
    family_rows(&[
        "lifecycle",
        "options",
        "memory",
        "syntax",
        "tolerance",
        "policy",
    ])
}

pub fn family_rows(families: &[&str]) -> Vec<Value> {
    let mut rows = vec![];
    for family in families {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .find(|p| p.join("tools/rhino_oracle/observations").is_dir())
            .unwrap()
            .join("tools/rhino_oracle/observations")
            .join(format!("mesh_boolean_{family}.json"));
        let data: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for mut row in data["results"].as_array().unwrap().iter().cloned() {
            // Retain rejected exploratory recipes in the captures, outside the
            // command replay domain. The Python qualifier audits the exact set.
            if row["value"]["history"]
                .as_str()
                .unwrap()
                .contains("Unknown command:")
            {
                continue;
            }
            row["family"] = json!(family);
            rows.push(row);
        }
    }
    rows
}
pub fn setup(row: &Value) -> Scene {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap();
    let folder = format!("mesh_boolean_{}", row["family"].as_str().unwrap());
    let path = root
        .join("tools/rhino_oracle/fixtures")
        .join(folder)
        .join(format!("{}.3dm", row["id"].as_str().unwrap()));
    frozen_scene::setup(&path, &row["value"], "Boolean output")
}
pub fn sets(
    scene: &Scene,
    input: &Value,
) -> (
    Vec<viboceros_document::ObjectId>,
    Vec<viboceros_document::ObjectId>,
) {
    let operation = input["operation"].as_str().unwrap();
    let mut first = input
        .get("first")
        .or_else(|| input.get("selected"))
        .map(|v| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_u64().unwrap() as usize)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            if matches!(operation, "difference" | "split") {
                vec![0]
            } else {
                (0..scene.ids.len()).collect()
            }
        });
    // Native preselection is enumerated in document order, unlike later picks.
    if input["preselect"] == true {
        first.sort_unstable();
    }
    let second = input
        .get("second")
        .map(|v| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_u64().unwrap() as usize)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            if matches!(operation, "difference" | "split") {
                (1..scene.ids.len()).collect()
            } else {
                vec![]
            }
        });
    (
        first.into_iter().map(|i| scene.ids[i]).collect(),
        second.into_iter().map(|i| scene.ids[i]).collect(),
    )
}
pub fn command(scene: &Scene, input: &Value) -> String {
    let operation = input["operation"].as_str().unwrap();
    let name = match operation {
        "union" => "MeshBooleanUnion",
        "intersection" => "MeshBooleanIntersection",
        "difference" => "MeshBooleanDifference",
        "split" => "MeshBooleanSplit",
        _ => unreachable!(),
    };
    let mut text = format!(
        "{name} {} {}",
        input["arguments"].as_str().unwrap_or(""),
        input["second_arguments"].as_str().unwrap_or("")
    );
    if operation != "union" {
        let (first, second) = sets(scene, input);
        let list = |ids: Vec<viboceros_document::ObjectId>| {
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        };
        text.push_str(&format!(
            " FirstSet={} SecondSet={}",
            list(first),
            list(second)
        ));
    }
    text
}
pub fn pick(scene: &mut Scene, ids: impl IntoIterator<Item = viboceros_document::ObjectId>) {
    for id in ids {
        scene
            .doc
            .select_objects_direct([id], SelectionMode::Add)
            .unwrap();
    }
}
pub fn snapshot(scene: &Scene) -> Value {
    json!(scene.doc.objects().map(|obj|{
        let a=obj.attributes();let color=a.object_color();let source=scene.ids.iter().position(|id|*id==obj.id());
        let mut groups=obj.group_ids().iter().map(|id|scene.doc.group(*id).unwrap().name().unwrap().to_owned()).collect::<Vec<_>>();groups.sort();
        let mut row=json!({"source":source,"role":if source.is_some(){"source"}else if obj.id()==scene.witness{"witness"}else{"output"},
            "selected":scene.doc.is_selected(obj.id()),"name":a.name(),"layer":scene.doc.layer(a.layer_id()).unwrap().name(),
            "color":[color.red,color.green,color.blue],"color_source":a.color_source()as u8,"groups":groups});
        match obj.geometry() {
            Geometry::Mesh(m)=>row["mesh"]=json!({"vertices":m.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":m.faces().iter().map(|f|f.indices()).collect::<Vec<_>>()}),
            Geometry::EmptyMesh=>row["mesh"]=json!({"vertices":[],"faces":[]}),
            Geometry::Point(p)=>row["point"]=json!(p.to_array()),
            Geometry::Polyline(p)=>row["polyline"]=json!(p.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>()),
            Geometry::Brep(_)=>row["kind"]=json!("Brep"),
            _=>row["kind"]=json!("other"),
        }
        row
    }).collect::<Vec<_>>())
}
pub fn check(scene: &Scene, value: &Value, stage: &str) -> Value {
    let actual = snapshot(scene);
    let expected = &value[stage];
    check_snapshot(actual, expected)
}
pub fn check_snapshot(actual: Value, expected: &Value) -> Value {
    let a = actual.as_array().unwrap();
    let n = expected.as_array().unwrap();
    let fields = [
        "source",
        "role",
        "selected",
        "name",
        "layer",
        "color",
        "color_source",
        "groups",
    ];
    let metadata = a.len() == n.len()
        && a.iter()
            .zip(n)
            .all(|(a, n)| fields.iter().all(|k| a[k] == n[k]));
    let mut geometry = a.len() == n.len();
    let mut maximum = Some(0_f64);
    if geometry {
        for (a, n) in a.iter().zip(n) {
            let error = if a.get("mesh").is_some() && n.get("mesh").is_some() {
                if a["mesh"]["faces"].as_array().unwrap().is_empty()
                    || n["mesh"]["faces"].as_array().unwrap().is_empty()
                {
                    let both_empty = [&a["mesh"], &n["mesh"]].iter().all(|mesh| {
                        mesh["faces"].as_array().unwrap().is_empty()
                            && mesh["vertices"].as_array().unwrap().is_empty()
                    });
                    geometry &= both_empty;
                    maximum = if both_empty { maximum } else { None };
                    continue;
                }
                let a: TriangleMesh = reference::mesh(&a["mesh"]);
                let n: TriangleMesh = reference::mesh(&n["mesh"]);
                geometry &= (a.area().unwrap() - n.area().unwrap()).abs() <= 1e-7;
                geometry &= (a.signed_volume().unwrap() - n.signed_volume().unwrap()).abs() <= 1e-7;
                coverage::sampled_distance(&[a], &[n])
            } else if a.get("polyline").is_some() && n.get("polyline").is_some() {
                polyline_error(&a["polyline"], &n["polyline"])
            } else if a["point"] == n["point"]
                && a["kind"] == n["kind"]
                && a.get("mesh").is_none()
                && n.get("mesh").is_none()
                && a.get("polyline").is_none()
                && n.get("polyline").is_none()
            {
                Some(0.)
            } else {
                None
            };
            geometry &= error.is_some_and(|e| e <= 1e-7);
            maximum = maximum.zip(error).map(|(a, b)| a.max(b));
        }
    } else {
        maximum = None;
    }
    json!({"objects":actual,"metadata":metadata,"geometry":geometry,"maximum_sampled_boundary_deviation":maximum})
}
fn polyline_error(a: &Value, n: &Value) -> Option<f64> {
    use viboceros_geometry::{LineSegment, Point3, Tolerance};
    let points = |v: &Value| {
        v.as_array()
            .unwrap()
            .iter()
            .map(reference::point)
            .collect::<Vec<_>>()
    };
    let (a, n) = (points(a), points(n));
    if a.len() < 2 || n.len() < 2 {
        return None;
    }
    let length = |p: &[Point3]| {
        p.windows(2)
            .map(|p| p[0].distance_to(p[1]).unwrap())
            .sum::<f64>()
    };
    if (length(&a) - length(&n)).abs() > 1e-7 {
        return None;
    }
    let directed = |source: &[Point3], target: &[Point3]| {
        source
            .windows(2)
            .flat_map(|p| [p[0], p[1], p[0].midpoint(p[1]).unwrap()])
            .map(|p| {
                target
                    .windows(2)
                    .map(|edge| {
                        let line =
                            LineSegment::try_new(edge[0], edge[1], Tolerance::MESH_VALIDATION)
                                .unwrap();
                        line.closest_point(p, Tolerance::MESH_VALIDATION)
                            .unwrap()
                            .distance_to(p)
                            .unwrap()
                    })
                    .fold(f64::INFINITY, f64::min)
            })
            .fold(0., f64::max)
    };
    Some(directed(&a, &n).max(directed(&n, &a)))
}
pub fn sources_restored(scene: &Scene) -> bool {
    scene.ids.iter().zip(&scene.sources).all(|(id, source)| {
        scene
            .doc
            .object(*id)
            .is_some_and(|o| o.geometry_snapshot().shares_storage_with(source))
    })
}

pub fn expected_tolerance(value: &Value) -> Option<&str> {
    let (_, tail) = value["history"]
        .as_str()
        .unwrap()
        .rsplit_once("Tolerance=")?;
    tail.split(|c: char| !c.is_ascii_alphabetic()).next()
}

pub const COMPATIBILITY_GAPS: [&str; 18] = [
    "boolean_lifecycle_difference_face_touch_pre",
    "boolean_lifecycle_difference_identical_pre",
    "boolean_lifecycle_intersection_face_touch_pre",
    "boolean_lifecycle_intersection_first_many",
    "boolean_lifecycle_intersection_identical_pre",
    "boolean_lifecycle_union_identical_normals",
    "boolean_lifecycle_union_identical_pre",
    "boolean_options_difference_auto",
    "boolean_options_gap_3_auto",
    "boolean_options_gap_4_auto",
    "boolean_options_intersection_auto",
    "boolean_tolerance_difference_document",
    "boolean_tolerance_difference_zero",
    "boolean_tolerance_gap_1_auto",
    "boolean_tolerance_gap_1_document",
    "boolean_tolerance_gap_1_zero",
    "boolean_tolerance_intersection_document",
    "boolean_tolerance_intersection_zero",
];

pub const SET_COMPATIBILITY_GAPS: [&str; 12] = [
    "boolean_sets_difference_nested_targets_post",
    "boolean_sets_difference_nested_targets_pre",
    "boolean_sets_difference_overlapping_targets_post",
    "boolean_sets_difference_overlapping_targets_pre",
    "boolean_sets_intersection_nested_targets_post",
    "boolean_sets_intersection_nested_targets_pre",
    "boolean_sets_intersection_overlapping_targets_post",
    "boolean_sets_intersection_overlapping_targets_pre",
    "boolean_split_face_touch_post",
    "boolean_split_face_touch_pre",
    "boolean_split_identical_post",
    "boolean_split_identical_pre",
];
