//! Command expectations use captured observations; operands and metadata are authored.
use super::captured;
use serde_json::{Value, json};
use viboceros_command::{CommandRegistry, merge_surfaces::Options};
use viboceros_document::*;
use viboceros_geometry::*;

pub fn families() -> Vec<Vec<Value>> {
    ["qualified", "getters", "holdouts"]
        .map(|family| {
            captured::read(&format!("merge_surface_command_{family}"))["results"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["id"] != "merge_q_getters")
                .cloned()
                .collect()
        })
        .into()
}

pub fn source(d: &Document, index: usize) -> ObjectId {
    let name = format!("merge-source-{index}");
    d.objects()
        .find(|o| o.attributes().name() == Some(&name))
        .unwrap()
        .id()
}

pub fn setup(v: &Value) -> Document {
    setup_named(v, "merge", "Merge")
}
pub fn setup_named(v: &Value, prefix: &str, label: &str) -> Document {
    let t = &v["model_tolerance"];
    let mut d = Document::new(
        Tolerance::try_new(
            t["absolute"].as_f64().unwrap(),
            t["relative"].as_f64().unwrap(),
            t["angular"].as_f64().unwrap(),
        )
        .unwrap(),
    );
    let layer = d
        .add_layer("Plane sources", ColorRgb::new(10, 20, 30))
        .unwrap();
    let output = d
        .add_layer("Plane output", ColorRgb::new(70, 80, 90))
        .unwrap();
    let sentinel = d
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(50., 60., 70.).unwrap()),
            ObjectAttributes::on_layer(layer)
                .with_name("sentinel")
                .with_object_color(ColorRgb::new(12, 34, 56)),
        )
        .unwrap();
    let ids = v["prescribed_case"]["curves"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, g)| {
            d.add_geometry_with_attributes(
                captured::input(g, d.tolerance()),
                ObjectAttributes::on_layer(layer)
                    .with_name(format!("{prefix}-source-{i}"))
                    .with_object_color(ColorRgb::new(40 + 80 * i as u8, 60, 90)),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let common = if v["prescribed_case"]["common_group"] == false {
        vec![sentinel]
    } else {
        std::iter::once(sentinel)
            .chain(ids.iter().copied())
            .collect()
    };
    d.add_group(Some("Plane source group".into()), common)
        .unwrap();
    for (i, id) in ids.iter().enumerate() {
        d.add_group(Some(format!("{label} input group {i}")), [*id])
            .unwrap();
    }
    d.set_current_layer(output).unwrap();
    d.clear_history().unwrap();
    if v["prescribed_case"]["pending_redo"] == true {
        CommandRegistry::with_builtins()
            .execute(&mut d, "Point 90,91,92")
            .unwrap();
        d.undo().unwrap();
    }
    if v["prescribed_case"]["preselected"] == true {
        let selected = v["prescribed_case"]["selection_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| ids[i.as_u64().unwrap() as usize])
            .collect::<Vec<_>>();
        d.select_objects_direct(selected, SelectionMode::Replace)
            .unwrap();
    }
    d
}

/// Adapt only authored option tokens for the complete, noninteractive API.
/// Cancellation itself is exercised by the application replay.
pub fn options(v: &Value, base: Options, d: &Document) -> Options {
    let mut options = base.for_document(d.tolerance()).unwrap();
    let mut getter = None;
    for action in v["prescribed_case"]["actions"].as_array().unwrap() {
        if action == "Cancel" {
            break;
        }
        if action.is_object() && action.get("curve").is_some() {
            continue;
        }
        let text = if let Some(s) = action.as_str() {
            s.to_owned()
        } else if let Some(s) = action["number"].as_str() {
            s.to_owned()
        } else {
            action.to_string()
        };
        if text == "Enter" {
            getter = None;
            continue;
        }
        if ["Tolerance", "Roundness"].contains(&text.as_str()) {
            getter = Some(text);
            continue;
        }
        let args = if let Some(field) = &getter {
            format!("{field}={text}")
        } else {
            text
        };
        if let Ok(next) = viboceros_command::merge_surfaces::parse(
            &args.split_whitespace().collect::<Vec<_>>(),
            options,
            d.units(),
        ) && next.effective_tolerance(d.tolerance()).is_ok()
        {
            options = next;
            getter = None;
        }
    }
    options
}

#[allow(dead_code)] // Also included by the UI replay, which needs only verification.
pub fn outcome(v: &Value) -> &str {
    outcome_command(v, "MergeSrf")
}
pub fn outcome_command<'a>(v: &'a Value, command: &str) -> &'a str {
    v["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == command)
        .unwrap()["result"]
        .as_str()
        .unwrap()
}

pub fn check(d: &Document, rows: &Value, archived: bool) -> f64 {
    let rows = rows.as_array().unwrap();
    assert_eq!(d.objects().len(), rows.len());
    let mut maximum = 0_f64;
    for (o, r) in d.objects().zip(rows) {
        let a = o.attributes();
        let expected = &r["attributes"];
        assert_eq!(a.name().unwrap_or(""), expected["name"].as_str().unwrap());
        assert_eq!(
            d.layer(a.layer_id()).unwrap().name(),
            expected["layer"].as_str().unwrap()
        );
        assert_eq!(
            a.object_color(),
            ColorRgb::new(
                expected["color"][0].as_u64().unwrap() as u8,
                expected["color"][1].as_u64().unwrap() as u8,
                expected["color"][2].as_u64().unwrap() as u8
            )
        );
        let groups = o
            .group_ids()
            .iter()
            .map(|id| d.group(*id).unwrap().name().unwrap())
            .collect::<Vec<_>>();
        let expected_groups = expected["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(groups, expected_groups);
        if !archived {
            assert_eq!(d.is_selected(o.id()), r["selected"].as_u64().unwrap() != 0);
        }
        match o.geometry() {
            Geometry::Point(p) => {
                maximum = maximum.max(p.distance_to(captured::point(&r["point"])).unwrap());
            }
            geometry => {
                let mut edges = r["brep"]["edges"].clone();
                for (index, e) in edges.as_array_mut().unwrap().iter_mut().enumerate() {
                    e["vertices"] = r["brep"]["topology"]["edges"][index].clone();
                    e["curve"]["type"] = "nurbs".into();
                }
                let faces = r["brep"]["topology"]["faces"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| {
                        let loops = f["loops"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|l| {
                                json!([
                                    if l["outer"] == true { "Outer" } else { "Inner" },
                                    l["trims"]
                                        .as_array()
                                        .unwrap()
                                        .iter()
                                        .map(|t| json!([
                                            t["edge"],
                                            t["reversed"].as_bool().unwrap()
                                                ^ f["reversed"].as_bool().unwrap()
                                        ]))
                                        .collect::<Vec<_>>()
                                ])
                            })
                            .collect::<Vec<_>>();
                        json!([loops, 0])
                    })
                    .collect::<Vec<_>>();
                let native = json!({"surfaces":r["surfaces"],"edges":edges,"faces":faces,
                    "vertices":r["brep"]["vertices"].as_array().unwrap().iter().map(|v| v["point"].clone()).collect::<Vec<_>>(),
                    "face_reversed":r["brep"]["topology"]["faces"].as_array().unwrap().iter().map(|f| f["reversed"].clone()).collect::<Vec<_>>()});
                let b = match geometry {
                    Geometry::Brep(b) => b.clone(),
                    Geometry::NurbsSurface(s) => {
                        Brep::try_rectangular_surface_face_with_surface_edge_domains(
                            s.clone(),
                            s.domain_u(),
                            s.domain_v(),
                            false,
                            d.tolerance(),
                        )
                        .unwrap()
                    }
                    _ => panic!("surface expected"),
                };
                maximum = maximum.max(captured::compare(&b, &native));
            }
        }
    }
    assert!(maximum <= captured::EPSILON);
    maximum
}

pub struct State {
    ids: Vec<ObjectId>,
}
impl State {
    pub fn capture(d: &Document) -> Self {
        Self {
            ids: d.objects().map(|o| o.id()).collect(),
        }
    }
    pub fn identities(&self, d: &Document, rows: &Value) {
        for (o, r) in d.objects().zip(rows.as_array().unwrap()) {
            if let Some(i) = r["input_index"].as_u64() {
                assert_eq!(o.id(), self.ids[1 + i as usize]);
            } else if r["attributes"]["name"] == "sentinel" {
                assert_eq!(o.id(), self.ids[0]);
            }
        }
    }
    pub fn verify(&self, d: &mut Document, v: &Value) -> Value {
        self.verify_command(d, v, "MergeSrf")
    }
    pub fn verify_command(&self, d: &mut Document, v: &Value, command: &str) -> Value {
        let mut error = check(d, &v["after"], false);
        self.identities(d, &v["after"]);
        if v["last_selection"].is_array() {
            CommandRegistry::with_builtins()
                .execute(d, "SelLast")
                .unwrap();
            error = error.max(check(d, &v["last_selection"], false));
            d.clear_selection();
        }
        let changed = v["history_changed"].as_bool().unwrap();
        assert_eq!(d.can_undo(), changed);
        if changed {
            assert_eq!(d.undo_label(), Some(command));
            d.undo().unwrap();
            error = error.max(check(d, &v["undo"], false));
            self.identities(d, &v["undo"]);
            assert!(!d.can_undo());
            d.redo().unwrap();
            error = error.max(check(d, &v["redo"], false));
            self.identities(d, &v["redo"]);
            assert!(!d.can_redo());
        } else {
            assert!(d.can_redo());
            d.redo().unwrap();
            error = error.max(check(d, &v["pending_redo"], false));
            d.undo().unwrap();
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("merged.3dm");
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(d, &format!("SaveAs {}", path.display()))
            .unwrap();
        let mut archive = Document::new(d.tolerance());
        registry
            .execute(&mut archive, &format!("Open3dm {}", path.display()))
            .unwrap();
        error = error.max(check(&archive, &v["after"], true));
        self.identities(&archive, &v["after"]);
        json!({"maximum_geometry_error":error,"archives":1,"changed":changed,"result":outcome_command(v,command)})
    }
}
