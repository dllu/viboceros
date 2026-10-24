//! Native grip locations, exact attributes, source storage, and history.
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
        } else {
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
