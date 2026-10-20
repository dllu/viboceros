//! Shared native Stretch sources and geometry, selection and history checks.
use serde_json::Value;
use viboceros_command::stretch::{Options, Target};
use viboceros_document::{ControlPointId, Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::{Brep, Point3, PointMorph, Real, Tolerance};
#[allow(dead_code)]
#[path = "grip_transform.rs"]
pub mod grips;
#[allow(dead_code)]
#[path = "../../src/morph_test_support.rs"]
pub mod morph;
pub fn rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/stretch.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/stretch.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 142);
    assert_eq!(r["results"].as_array().unwrap().len(), 142);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            (op.clone(), row["value"].clone())
        })
        .collect()
}
pub fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
pub fn token(p: Point3) -> String {
    p.to_array().map(|v| v.to_string()).join(",")
}
pub fn axis(op: &Value) -> (Point3, Point3) {
    if op["op"] == "stretch_grips" {
        return (
            Point3::try_new(-1., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
        );
    }
    if !op["start"].is_null() {
        return (point(&op["start"]), point(&op["end"]));
    }
    if op["axis"] == "Skew" {
        return (
            point(&serde_json::json!([1., 2., 3.])),
            point(&serde_json::json!([4., -2., 11.])),
        );
    }
    morph::axis(op)
}
pub fn options(op: &Value) -> Options {
    Options {
        copy: op["copy"] == true,
        rigid: op["rigid"] == true,
    }
}
pub fn targets(op: &Value) -> Vec<Target> {
    let first = if let Some(length) = op["length"].as_f64() {
        Target::Length(length)
    } else if op["target"].is_array() {
        Target::Point(point(&op["target"]))
    } else {
        Target::Factor(op["factor"].as_f64().unwrap_or(2.))
    };
    std::iter::once(first)
        .chain(op["targets"].as_array().into_iter().flatten().map(|v| {
            if v.is_array() {
                Target::Point(point(v))
            } else {
                Target::Factor(v.as_f64().unwrap())
            }
        }))
        .collect()
}
pub struct Sources {
    pub ids: Vec<ObjectId>,
    pub grips: Vec<ControlPointId>,
    pub source: Option<ObjectId>,
    pub extra: Option<ObjectId>,
    originals: Vec<Geometry>,
}
impl Sources {
    pub fn arguments(&self, op: &Value) -> String {
        let post = op["preselect"] == false;
        let ids = &self.ids;
        let grips = if self.grips.is_empty() {
            String::new()
        } else {
            format!(
                "GripSources={}",
                self.grips
                    .iter()
                    .map(|g| format!("{}:{}", g.object, g.index))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        format!(
            "{}={} {grips}",
            if post { "PickedSources" } else { "Sources" },
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
    pub fn check(&self, doc: &Document, op: &Value, value: &Value, phase: &str) -> Real {
        let label = format!("{} {phase}", op["id"].as_str().unwrap());
        let expected = &value[phase];
        if op["op"] == "stretch_grips" {
            return grips::compare_error(
                &grips::snapshot(doc, self.source.unwrap(), self.extra),
                expected,
                &label,
            );
        }
        if op["op"] == "stretch_command" {
            let rows = expected.as_array().unwrap();
            assert_eq!(doc.objects().len(), rows.len(), "{label}");
            let mut gap = 0_f64;
            for (object, row) in doc.objects().zip(rows) {
                let Geometry::Point(p) = object.geometry() else {
                    panic!("{label}: point")
                };
                assert_eq!(
                    doc.is_selected(object.id()),
                    row["selected"].as_bool().unwrap(),
                    "{label}"
                );
                assert_eq!(
                    self.ids
                        .iter()
                        .position(|id| *id == object.id())
                        .map(|i| i as u64),
                    row["source"].as_u64(),
                    "{label}: identity"
                );
                gap = gap.max(p.distance_to(point(&row["point"])).unwrap());
            }
            assert!(gap <= 1e-7, "{label}: {gap}");
            return gap;
        }
        assert_eq!(
            doc.objects().len(),
            expected["objects"].as_array().unwrap().len(),
            "{label}"
        );
        morph::assert_metadata(doc, expected, &label);
        let mut gap = 0_f64;
        for (i, (object, row)) in doc
            .objects()
            .zip(expected["objects"].as_array().unwrap())
            .enumerate()
        {
            let epsilon = if op["rigid"] == true {
                1e-7
            } else if op["shape"] == "Points" {
                1e-10
            } else if matches!(op["axis"].as_str(), Some("Spatial" | "Skew"))
                && matches!(op["shape"].as_str(), Some("Surface" | "Box"))
            {
                // Public SDK ClosestPoint measurements establish actual native
                // approximation error, with same-UV residuals up to 6.45e-4.
                // Check our own fit against the exact point map separately below.
                7e-4
            } else {
                2e-5
            };
            gap = gap.max(morph::compare_geometry_error(
                object.geometry(),
                &row["geometry"],
                &self.originals[i % self.originals.len()],
                &value["before"]["objects"][i % self.originals.len()]["geometry"],
                epsilon,
                &label,
            ));
            if op["rigid"] != true {
                let source_index = i % self.originals.len();
                let source_record = &value["before"]["objects"][source_index]["geometry"];
                let mut ideal = source_record.clone();
                let changed = matches!(phase, "after" | "redo")
                    && (op["copy"] != true || i >= self.originals.len());
                let mut dense_error = 0_f64;
                if changed {
                    let (start, end) = axis(op);
                    let targets = targets(op);
                    let index = if op["copy"] == true {
                        i / self.originals.len() - 1
                    } else {
                        0
                    };
                    let map = targets[index].morph(start, end).unwrap();
                    dense_error = check_fit(
                        object.geometry(),
                        &self.originals[source_index],
                        &map,
                        &label,
                    );
                    for field in ["points", "samples", "vertices"] {
                        if let Some(points) = ideal[field].as_array_mut() {
                            for v in points {
                                *v = serde_json::json!(
                                    map.morph_point(point(v)).unwrap().to_array()
                                );
                            }
                        }
                    }
                }
                let fit_error = morph::compare_geometry_error(
                    object.geometry(),
                    &ideal,
                    &self.originals[source_index],
                    source_record,
                    1.1e-5,
                    &format!("{label}: exact map"),
                );
                eprintln!(
                    "STRETCH_FIT_AUDIT {}",
                    serde_json::json!({"id":op["id"],"phase":phase,"object":i,"maximum_error":fit_error.max(dense_error)})
                );
            }
        }
        gap
    }
}

fn check_fit(actual: &Geometry, source: &Geometry, map: &impl PointMorph, label: &str) -> Real {
    let mut worst = 0_f64;
    let mut near = |a: Point3, p: Point3| {
        let error = a.distance_to(map.morph_point(p).unwrap()).unwrap();
        worst = worst.max(error);
        assert!(error <= 1.1e-5, "{label}: dense exact-map error {error}");
    };
    if let (Geometry::Brep(a), Geometry::Brep(s)) = (actual, source) {
        assert_eq!(a.faces().len(), s.faces().len());
        let stations = std::iter::once(0.)
            .chain((0..31).map(|i| (i as f64 + 0.37) / 31.))
            .chain(std::iter::once(1.))
            .collect::<Vec<_>>();
        for (a, s) in a.faces().iter().zip(s.faces()) {
            let sample = |surface: &viboceros_geometry::NurbsSurface, u: Real, v: Real| {
                let du = surface.domain_u();
                let dv = surface.domain_v();
                surface
                    .evaluate(
                        du.start() + (du.end() - du.start()) * u,
                        dv.start() + (dv.end() - dv.start()) * v,
                    )
                    .unwrap()
            };
            for &v in &stations {
                for &u in &stations {
                    near(sample(a.surface(), u, v), sample(s.surface(), u, v));
                }
            }
        }
    } else if let (Some(a), Some(s)) = (actual.curve_ref(), source.curve_ref()) {
        let a = a.to_nurbs().unwrap();
        let s = s.to_nurbs().unwrap();
        let a = a.parameter_sampler().unwrap();
        let s = s.parameter_sampler().unwrap();
        for t in std::iter::once(0.)
            .chain((0..257).map(|i| (i as f64 + 0.37) / 257.))
            .chain(std::iter::once(1.))
        {
            near(a.evaluate(t).unwrap(), s.evaluate(t).unwrap());
        }
    }
    worst
}
pub fn setup(op: &Value, value: &Value) -> (Document, Sources) {
    if op["op"] == "stretch_geometry_command" {
        let mut recipe = op.clone();
        recipe["tolerance"] = op["tolerance"].as_f64().unwrap_or(1e-6).into();
        let (mut doc, ids) = morph::setup(&recipe, &value["before"]);
        for id in &ids {
            if let Geometry::NurbsSurface(s) = doc.object(*id).unwrap().geometry() {
                let b = Brep::try_surface_face(s.clone(), doc.tolerance()).unwrap();
                doc.replace_object_geometries([(*id, Geometry::Brep(b))])
                    .unwrap();
            }
        }
        let originals = ids
            .iter()
            .map(|id| doc.object(*id).unwrap().geometry().clone())
            .collect();
        doc.clear_history().unwrap();
        return (
            doc,
            Sources {
                ids,
                grips: vec![],
                source: None,
                extra: None,
                originals,
            },
        );
    }
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap());
    let mut ids = Vec::new();
    let mut picks = Vec::new();
    let mut source = None;
    let mut extra = None;
    if op["op"] == "stretch_grips" {
        let id = doc
            .add_geometry(grips::geometry(op, &value["before"][0]))
            .unwrap();
        source = Some(id);
        doc.set_object_names([(id, Some("grip source".into()))])
            .unwrap();
        doc.enable_control_points([id]).unwrap();
        picks = op["selected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| ControlPointId {
                object: id,
                index: i.as_u64().unwrap() as usize,
            })
            .collect();
        doc.select_control_points(picks.iter().copied(), SelectionMode::Add)
            .unwrap();
        if op["parent"] == true {
            doc.select_objects_direct([id], SelectionMode::Add).unwrap();
            ids.push(id);
        }
        if op["point"] == true {
            let id = doc
                .add_geometry(Geometry::Point(Point3::try_new(8., 0., 0.).unwrap()))
                .unwrap();
            extra = Some(id);
            doc.select_objects_direct([id], SelectionMode::Add).unwrap();
            ids.push(id);
        }
    } else {
        for row in value["before"].as_array().unwrap() {
            let id = doc
                .add_geometry(Geometry::Point(point(&row["point"])))
                .unwrap();
            if row["selected"] == true {
                doc.select_objects_direct([id], SelectionMode::Add).unwrap();
            }
            ids.push(id);
        }
        if op["grouped"] == true {
            doc.add_group(None, ids.iter().copied()).unwrap();
        }
    }
    doc.clear_history().unwrap();
    (
        doc,
        Sources {
            ids,
            grips: picks,
            source,
            extra,
            originals: vec![],
        },
    )
}
