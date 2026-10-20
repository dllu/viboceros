//! Native input scenes and complete observable array state, shared by API and UI.
use crate::morph_fixtures as morph;
use serde_json::{Value, json};
use viboceros_command::{
    CommandContext, CommandRegistry,
    array_curve_surface::{Layout, Placement, References},
};
use viboceros_document::{Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::{Brep, Frame3, Point3, PointCloud3, Tolerance, Vector3};
pub fn rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/array_curve_surface.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/array_curve_surface.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 44);
    assert_eq!(r["results"].as_array().unwrap().len(), 44);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, r)| {
            assert_eq!(op["id"], r["id"]);
            assert!(r["error"].is_null());
            assert_eq!(r["value"]["input_trace"]["errors"], json!([]));
            (op.clone(), r["value"].clone())
        })
        .collect()
}
pub fn point(v: &Value) -> Point3 {
    morph::p(v)
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
pub fn token(p: Point3) -> String {
    p.to_array().map(|x| x.to_string()).join(",")
}
pub struct Scene {
    pub sources: Vec<ObjectId>,
    pub rail: ObjectId,
    pub target: ObjectId,
    original: Vec<(ObjectId, Geometry)>,
}
pub fn setup(value: &Value) -> (Document, Scene) {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    for name in value["before"]["used_names"].as_array().unwrap() {
        let g = doc
            .add_empty_group(Some(name.as_str().unwrap().into()))
            .unwrap();
        doc.remove_group(g).unwrap();
    }
    let mut ids = Vec::new();
    let mut sources = Vec::new();
    let mut rail = None;
    let mut target = None;
    for row in value["before"]["objects"].as_array().unwrap() {
        let g = &row["geometry"];
        let geometry = match g["kind"].as_str().unwrap() {
            "PointCloud" => Geometry::PointCloud(
                PointCloud3::try_new(g["points"].as_array().unwrap().iter().map(point).collect())
                    .unwrap(),
            ),
            "Brep" => {
                let surface = morph::surface(&g["surface"]);
                let b = if let Some(range) = g["trim_range"].as_array() {
                    Brep::try_rectangular_surface_face(
                        surface,
                        range[0][0].as_f64().unwrap()..=range[0][1].as_f64().unwrap(),
                        range[1][0].as_f64().unwrap()..=range[1][1].as_f64().unwrap(),
                        doc.tolerance(),
                    )
                    .unwrap()
                } else {
                    Brep::try_surface_face(surface, doc.tolerance()).unwrap()
                };
                Geometry::Brep(if g["reversed"] == true {
                    b.reversed()
                } else {
                    b
                })
            }
            _ => Geometry::NurbsCurve(morph::curve(&g["curve"])),
        };
        let id = doc.add_geometry(geometry).unwrap();
        ids.push(id);
        doc.set_object_names([(id, Some(row["name"].as_str().unwrap().into()))])
            .unwrap();
        if row["role"] == "source" {
            sources.push(id);
        } else if row["role"] == "rail" {
            rail = Some(id);
        } else {
            target = Some(id);
        }
        if row["selected"] == true {
            doc.select_objects_direct([id], SelectionMode::Add).unwrap();
        }
    }
    for g in value["before"]["groups"].as_array().unwrap() {
        let members = value["before"]["objects"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&ids)
            .filter(|(o, _)| o["groups"].as_array().unwrap().contains(&g["index"]))
            .map(|(_, id)| *id)
            .collect::<Vec<_>>();
        if members.is_empty() {
            doc.add_empty_group(Some(g["name"].as_str().unwrap().into()))
                .unwrap();
        } else {
            doc.add_group(Some(g["name"].as_str().unwrap().into()), members)
                .unwrap();
        }
    }
    let original = doc
        .objects()
        .map(|o| (o.id(), o.geometry().clone()))
        .collect();
    doc.clear_history().unwrap();
    (
        doc,
        Scene {
            sources,
            rail: rail.unwrap_or(target.unwrap()),
            target: target.unwrap(),
            original,
        },
    )
}
impl Scene {
    pub fn refs(&self, op: &Value, value: &Value) -> (References, Option<[f64; 2]>) {
        let curve = self
            .original
            .iter()
            .find(|(id, _)| *id == self.rail)
            .unwrap()
            .1
            .nurbs_curve_representation()
            .unwrap()
            .unwrap();
        let domain = curve.domain();
        let range = op["subcurve"].as_array().map(|v| {
            [v[0].as_f64().unwrap(), v[1].as_f64().unwrap()]
                .map(|f| domain.start() + f * (domain.end() - domain.start()))
        });
        // SubCrv's final authored point is the reference pick for the shortened rail.
        let pick = if let Some([_, end]) = range {
            Some(curve.evaluate(end).unwrap())
        } else {
            value["input_trace"]["clicks"]
                .as_array()
                .and_then(|c| c.first())
                .map(|c| point(&c["projected_point"]))
        };
        (
            References {
                curve: self.rail,
                edge: None,
                surface: self.target,
                face: None,
                pick,
            },
            range,
        )
    }
    pub fn execute(
        &self,
        doc: &mut Document,
        commands: &CommandRegistry,
        op: &Value,
        value: &Value,
    ) {
        let (refs, range) = self.refs(op, value);
        let placement =
            Placement::try_new(doc, point(&op["base_point"]), plane(op), refs, range, false)
                .unwrap();
        let mut group = doc.begin_history_group("ArrayCrvOnSrf").unwrap();
        let mut last = 0.;
        let range = range.map_or_else(String::new, |[a, b]| format!(" SubCrv={a},{b}"));
        let mut apply = |mode: &str, value: &Value| {
            let steps = if matches!(mode, "Positions" | "Distances") {
                value.as_array().unwrap().clone()
            } else {
                vec![value.clone()]
            };
            for v in steps {
                let args = match mode {
                    "Divide" => format!(
                        "Divide={}",
                        v.as_u64()
                            .map_or_else(|| "Default".into(), |n| n.to_string())
                    ),
                    "Multiple" => format!(
                        "Multiple={},{} Offset={last}",
                        v[0].as_u64()
                            .map_or_else(|| "Default".into(), |n| n.to_string()),
                        v[1].as_f64()
                            .map_or_else(|| "Default".into(), |n| n.to_string())
                    ),
                    "Positions" => format!("Positions={}", token(point(&v))),
                    "Distances" => format!("Distances={} Offset={last}", v.as_f64().unwrap()),
                    _ => unreachable!(),
                };
                let input = format!(
                    "ArrayCrvOnSrf {} {} {args}{range} Finish=No {}={}",
                    token(point(&op["base_point"])),
                    refs.arguments(),
                    if op["postselect"] == true {
                        "PickedSources"
                    } else {
                        "Sources"
                    },
                    self.sources
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                );
                if commands
                    .execute_in_history_group(
                        doc,
                        &input,
                        CommandContext {
                            construction_plane: plane(op),
                        },
                        &mut group,
                    )
                    .is_ok()
                {
                    match mode {
                        "Positions" => last = placement.distance_at(point(&v)).unwrap(),
                        "Distances" => last += v.as_f64().unwrap(),
                        "Multiple" => {
                            let p = commands.array_curve_surface_preferences();
                            let maps = placement
                                .transforms(
                                    &Layout::Multiple {
                                        count: p.count,
                                        distance: p.distance,
                                        start: last,
                                    },
                                    self.sources.len(),
                                )
                                .unwrap();
                            last += maps.len() as f64 * p.distance;
                        }
                        _ => {}
                    }
                }
            }
        };
        if op["mode"] == "Sequence" {
            for step in op["value"].as_array().unwrap() {
                let (key, v) = step.as_object().unwrap().iter().next().unwrap();
                let mode = match key.as_str() {
                    "divide" => "Divide",
                    "multiple" => "Multiple",
                    "point" => "Positions",
                    "distance" => "Distances",
                    _ => unreachable!(),
                };
                let v = if matches!(mode, "Positions" | "Distances") {
                    json!([v])
                } else {
                    v.clone()
                };
                apply(mode, &v);
            }
        } else {
            apply(op["mode"].as_str().unwrap(), &op["value"]);
        }
        if op["postselect"] == true || !op["subcurve"].is_null() {
            doc.clear_selection();
        }
    }
    pub fn check(&self, doc: &Document, value: &Value, phase: &str, label: &str) -> f64 {
        let expected = &value[phase];
        let rows = expected["objects"].as_array().unwrap();
        assert_eq!(doc.objects().count(), rows.len(), "{label} {phase} count");
        let names = expected["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["name"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            doc.groups()
                .map(|g| g.name().unwrap())
                .collect::<std::collections::BTreeSet<_>>(),
            names,
            "{label} {phase} definitions"
        );
        let mut max = 0_f64;
        for (o, row) in doc.objects().zip(rows) {
            assert_eq!(
                o.attributes().name(),
                row["name"].as_str(),
                "{label} {phase} name"
            );
            assert_eq!(
                doc.is_selected(o.id()),
                row["selected"].as_bool().unwrap(),
                "{label} {phase} selection"
            );
            let actual_groups = o
                .group_ids()
                .iter()
                .map(|id| doc.group(*id).unwrap().name().unwrap())
                .collect::<Vec<_>>();
            let expected_groups = row["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|index| {
                    expected["groups"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|g| g["index"] == *index)
                        .unwrap()["name"]
                        .as_str()
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                actual_groups, expected_groups,
                "{label} {phase} memberships"
            );
            if let Geometry::PointCloud(cloud) = o.geometry() {
                let points = row["geometry"]["points"].as_array().unwrap();
                assert_eq!(cloud.points().len(), points.len());
                for (a, b) in cloud.points().iter().zip(points) {
                    max = max.max(a.distance_to(point(b)).unwrap());
                }
            } else {
                assert_eq!(
                    o.geometry(),
                    &self
                        .original
                        .iter()
                        .find(|(id, _)| *id == o.id())
                        .unwrap()
                        .1,
                    "{label} {phase} guide changed"
                );
            }
        }
        assert!(max <= 1e-6, "{label} {phase} cloud error {max}");
        max
    }
}
