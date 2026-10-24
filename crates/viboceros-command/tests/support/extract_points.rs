//! Native grip locations, exact attributes, source storage, and history.
#![allow(dead_code)] // Shared by command and application test binaries.
use serde_json::Value;
use std::path::Path;
use viboceros_document::{ControlPointId, Document, Geometry, ObjectId, SelectionMode};

pub fn rows() -> Vec<Value> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle").is_dir())
        .unwrap();
    ["grips", "verified", "metadata"]
        .into_iter()
        .flat_map(|family| {
            let data: Value = serde_json::from_str(
                &std::fs::read_to_string(root.join(format!(
                    "tools/rhino_oracle/observations/extract_points_{family}.json"
                )))
                .unwrap(),
            )
            .unwrap();
            data["results"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| family != "grips" || r["value"]["input"]["output"] == "Points")
                .cloned()
                .map(move |mut row| {
                    row["family"] = family.into();
                    row["path"] = if family == "grips" {
                        "preselected"
                    } else {
                        "picked"
                    }
                    .into();
                    row
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn setup(row: &Value) -> (Document, Vec<ObjectId>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap();
    let path = root.join(format!(
        "tools/rhino_oracle/fixtures/extract_points_{}/{}.3dm",
        row["family"].as_str().unwrap(),
        row["id"].as_str().unwrap()
    ));
    let mut doc = viboceros_command::open_3dm_with_views(path.to_str().unwrap())
        .unwrap()
        .0;
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.set_current_layer(doc.layer_by_name("Extract current").unwrap().id())
        .unwrap();
    doc.add_group(Some("Extract assembly".into()), ids.iter().copied())
        .unwrap();
    let before = row["value"]["before"].as_array().unwrap();
    for (i, &id) in ids.iter().enumerate() {
        if row["value"]["input"]["sources"][i]["mode"] == "edit" {
            doc.enable_edit_points([id]).unwrap();
        } else if row["value"]["input"]["sources"][i]["mode"] == "control" {
            doc.enable_control_points([id]).unwrap();
        }
        let hidden = before[i]["grips"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|g| g["hidden"] == true)
            .map(|g| ControlPointId {
                object: id,
                index: g["index"].as_u64().unwrap() as usize,
            })
            .collect::<Vec<_>>();
        doc.select_control_points(hidden, SelectionMode::Replace)
            .unwrap();
        doc.hide_selected_control_points();
    }
    doc.clear_selection();
    doc.select_objects_direct(
        ids.iter()
            .enumerate()
            .filter(|(i, _)| before[*i]["selected"] == true)
            .map(|(_, id)| *id),
        SelectionMode::Replace,
    )
    .unwrap();
    for (i, &id) in ids.iter().enumerate() {
        let selected = before[i]["grips"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|g| g["selected"] == true)
            .map(|g| ControlPointId {
                object: id,
                index: g["index"].as_u64().unwrap() as usize,
            });
        doc.select_control_points(selected, SelectionMode::Add)
            .unwrap();
    }
    doc.clear_history().unwrap();
    compare(&doc, &ids, &row["value"]["before"]);
    (doc, ids)
}

fn near(actual: &[viboceros_geometry::Point3], expected: &Value) -> f64 {
    let expected = expected.as_array().unwrap();
    assert_eq!(actual.len(), expected.len());
    let error = actual
        .iter()
        .zip(expected)
        .flat_map(|(a, b)| {
            a.to_array()
                .into_iter()
                .zip(b.as_array().unwrap())
                .map(|(x, y)| (x - y.as_f64().unwrap()).abs())
        })
        .fold(0., f64::max);
    assert!(error <= 1e-7, "point deviation {error}");
    error
}

pub fn compare(doc: &Document, ids: &[ObjectId], expected: &Value) -> f64 {
    let expected = expected.as_array().unwrap();
    assert_eq!(doc.objects().len(), expected.len());
    let mut error = 0_f64;
    for (object, native) in doc.objects().zip(expected) {
        let a = object.attributes();
        assert_eq!(a.name(), native["name"].as_str());
        assert_eq!(
            doc.layer(a.layer_id()).unwrap().name(),
            native["layer"].as_str().unwrap()
        );
        let color = a.object_color();
        assert_eq!(
            serde_json::json!([color.red, color.green, color.blue]),
            native["color"]
        );
        assert_eq!(
            a.color_source() as u8,
            native["color_source"].as_u64().unwrap() as u8
        );
        assert_eq!(
            serde_json::json!(
                object
                    .group_ids()
                    .iter()
                    .map(|g| doc.group(*g).unwrap().name().unwrap())
                    .collect::<Vec<_>>()
            ),
            native["groups"]
        );
        assert_eq!(
            doc.is_selected(object.id()),
            native["selected"].as_bool().unwrap()
        );
        if let Some(index) = native["source"].as_u64() {
            assert_eq!(object.id(), ids[index as usize]);
            let grips = native["grips"].as_array().unwrap();
            assert_eq!(
                doc.control_point_locations(object.id()).is_some(),
                native["grips_on"] == true
            );
            let points = doc.control_point_locations(object.id()).unwrap_or_default();
            error = error.max(near(
                points,
                &Value::Array(grips.iter().map(|g| g["point"].clone()).collect()),
            ));
            let selected = doc
                .selected_logical_control_points()
                .filter(|(g, _)| g.object == object.id())
                .map(|(g, _)| g.index)
                .collect::<Vec<_>>();
            assert_eq!(
                selected,
                grips
                    .iter()
                    .filter(|g| g["selected"] == true)
                    .map(|g| g["index"].as_u64().unwrap() as usize)
                    .collect::<Vec<_>>()
            );
            for g in grips {
                assert_eq!(
                    doc.is_control_point_hidden(ControlPointId {
                        object: object.id(),
                        index: g["index"].as_u64().unwrap() as usize
                    }),
                    g["hidden"].as_bool()
                );
            }
        } else {
            match object.geometry() {
                Geometry::Point(p) => {
                    assert_eq!(native["cloud"], false);
                    error = error.max(near(&[*p], &native["points"]));
                }
                Geometry::PointCloud(c) => {
                    assert_eq!(native["cloud"], true);
                    error = error.max(near(c.points(), &native["points"]));
                }
                _ => panic!("unexpected extracted output"),
            }
        }
    }
    error
}

pub fn command(row: &Value) -> String {
    format!(
        "ExtractPt OutputLayer={} Output={}",
        row["value"]["input"]["layer"].as_str().unwrap(),
        row["value"]["input"]["output"].as_str().unwrap()
    )
}

pub fn snapshot(doc: &Document, ids: &[ObjectId]) -> Value {
    Value::Array(doc.objects().map(|object| {
        let a = object.attributes();
        let color = a.object_color();
        let (points, cloud) = match object.geometry() {
            Geometry::Point(p) => (serde_json::json!([p.to_array()]), false),
            Geometry::PointCloud(c) => (serde_json::json!(c.points().iter().map(|p| p.to_array()).collect::<Vec<_>>()), true),
            _ => (Value::Null, false),
        };
        let selected = doc.selected_logical_control_points().map(|(g, _)| g).collect::<std::collections::BTreeSet<_>>();
        let grips = doc.control_point_locations(object.id()).unwrap_or_default().iter().enumerate().map(|(index, point)| {
            let id = ControlPointId { object: object.id(), index };
            serde_json::json!({"index":index,"point":point.to_array(),
                "selected":selected.contains(&id),"hidden":doc.is_control_point_hidden(id).unwrap()})
        }).collect::<Vec<_>>();
        serde_json::json!({"source":ids.iter().position(|id| *id == object.id()),
            "name":a.name(),"layer":doc.layer(a.layer_id()).unwrap().name(),
            "color":[color.red,color.green,color.blue],"color_source":a.color_source() as u8,
            "groups":object.group_ids().iter().map(|g| doc.group(*g).unwrap().name().unwrap()).collect::<Vec<_>>(),
            "selected":doc.is_selected(object.id()),"points":points,"cloud":cloud,
            "grips_on":doc.grip_mode(object.id()).is_some(),"grips":grips})
    }).collect())
}

/// Original component source files and fixed whole/grip/component recipes.
pub fn component_rows() -> Vec<Value> {
    component_rows_for(&[
        "component_selection",
        "component_edge_holdouts",
        "component_mixed_verified",
        "component_picked",
        "component_brep",
        "component_brep_picked",
        "component_brep_holdouts",
        "component_brep_periodic_edges",
    ])
}
pub fn edge_form_rows() -> Vec<Value> {
    component_rows_for(&["component_edge_holdouts"])
}

pub fn range_rows() -> Vec<Value> {
    component_rows_for(&[
        "ranges",
        "ranges_grips",
        "ranges_owners",
        "ranges_edges",
        "ranges_edge_forms",
        "ranges_edge_numeric_verified",
    ])
    .into_iter()
    .map(|mut row| {
        row["path"] = "picked".into();
        row
    })
    .collect()
}
pub fn range_command(row: &Value, ids: &[ObjectId]) -> String {
    let mut result = command(row);
    for (index, range) in row["value"]["getter"]["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if let Some(edge) = row["value"]["getter"]["edges"][index].as_u64() {
            result.push_str(&format!(
                " SubCrv={},{},{},{},{}",
                ids[range[0].as_u64().unwrap() as usize],
                if row["value"]["getter"]["numeric_length"].is_number() {
                    "EdgeWrap"
                } else {
                    "Edge"
                },
                edge,
                range[1],
                range[2]
            ));
            continue;
        }
        result.push_str(&format!(
            " SubCrv={},{},{}",
            ids[range[0].as_u64().unwrap() as usize],
            range[1],
            range[2]
        ));
    }
    result
}
pub fn select_range_grips(doc: &mut Document, row: &Value) {
    if row["value"]["input"]["range_grips"] == true {
        viboceros_command::point_selection::PointSelectionAction::from_input("SelPt")
            .unwrap()
            .unwrap()
            .apply(doc, |_| true)
            .unwrap();
    }
}
fn component_rows_for(families: &[&str]) -> Vec<Value> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle").is_dir())
        .unwrap();
    families
        .iter()
        .copied()
        .flat_map(|family| {
            let path = root.join(format!(
                "tools/rhino_oracle/observations/extract_points_{family}.json"
            ));
            let data: Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            data["results"]
                .as_array()
                .unwrap()
                .iter()
                .cloned()
                .map(move |mut row| {
                    row["family"] = family.into();
                    row["path"] = if row["value"]["input"]["component_getter"] == true {
                        "picked"
                    } else {
                        "preselected"
                    }
                    .into();
                    row
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
pub fn component_command(row: &Value, ids: &[ObjectId]) -> String {
    let mut command = command(row);
    if row["value"]["input"]["flat"] == true {
        command.push_str(" SubDLocation=ControlNet");
    }
    for pick in component_picks(row) {
        command.push_str(&format!(
            " Component={},{},{}",
            ids[pick[0].as_u64().unwrap() as usize],
            pick[1].as_str().unwrap(),
            pick[2].as_u64().unwrap()
        ));
    }
    command
}

/// Whole-object preselection can replace the earlier subobject selections.
pub fn component_picks(row: &Value) -> Vec<Value> {
    if row["path"] == "picked" {
        return row["value"]["input"]["components"]
            .as_array()
            .unwrap()
            .clone();
    }
    row["value"]["before"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| {
            o["source"]
                .as_u64()
                .map(|source| (source, &o["components"]))
        })
        .flat_map(|(source, picks)| {
            picks
                .as_array()
                .unwrap()
                .iter()
                .map(move |p| (source, p[0].as_str().unwrap(), p[1].as_u64().unwrap()))
        })
        // Unwelded mesh vertex aliases may report the same topology pick
        // multiple times. A second UI click would toggle that pick off.
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|(source, kind, index)| serde_json::json!([source, kind, index]))
        .collect()
}
