//! Native tensor coefficients, face geometry and observable document policy.
use super::native;
use serde_json::{Value, json};
use viboceros_document::{Document, Geometry, ObjectAttributes, ObjectColorSource};
use viboceros_geometry::{NurbsSurface, Point3};

fn surface_error(actual: &NurbsSurface, target: &Value) -> f64 {
    let target = native::surface(target);
    assert_eq!(
        [actual.degree_u(), actual.degree_v()],
        [target.degree_u(), target.degree_v()]
    );
    assert_eq!(
        [
            actual.control_point_count_u(),
            actual.control_point_count_v()
        ],
        [
            target.control_point_count_u(),
            target.control_point_count_v()
        ]
    );
    let mut error = 0_f64;
    for (a, b) in actual.control_points().iter().zip(target.control_points()) {
        assert_eq!(a.weight(), b.weight());
        error = error.max(a.point().distance_to(b.point()).unwrap());
    }
    for (a, b) in [
        (actual.knots_u(), target.knots_u()),
        (actual.knots_v(), target.knots_v()),
    ] {
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(b) {
            error = error.max((a - b).abs());
        }
    }
    for i in 0..=8 {
        for j in 0..=8 {
            let point = |s: &NurbsSurface| {
                s.evaluate(
                    s.parameter_at_u(i as f64 / 8.).unwrap(),
                    s.parameter_at_v(j as f64 / 8.).unwrap(),
                )
                .unwrap()
            };
            error = error.max(point(actual).distance_to(point(&target)).unwrap());
        }
    }
    error
}
fn sorted_points(points: impl Iterator<Item = Point3>) -> Vec<Point3> {
    let mut points = points.collect::<Vec<_>>();
    points.sort_by(|a, b| {
        a.x()
            .total_cmp(&b.x())
            .then(a.y().total_cmp(&b.y()))
            .then(a.z().total_cmp(&b.z()))
    });
    points
}
pub fn check(doc: &Document, v: &Value, phase: &str, archive: bool) -> f64 {
    let expected = v[phase].as_array().unwrap();
    assert_eq!(doc.objects().len(), expected.len());
    let mut error = 0_f64;
    for (object, target) in doc.objects().zip(expected) {
        let a = object.attributes();
        assert_eq!(
            json!({"name":a.name().unwrap_or(""),"layer":doc.layer(a.layer_id()).unwrap().name(),
            "color_source":if a.color_source()==ObjectColorSource::Object {"ColorFromObject"} else {"ColorFromLayer"},
            "color":[a.object_color().red,a.object_color().green,a.object_color().blue],
            "groups":object.group_ids().iter().map(|id|doc.group(*id).unwrap().name().unwrap()).collect::<Vec<_>>()}),
            target["attributes"]
        );
        assert_eq!(
            if doc.is_selected(object.id()) { 2 } else { 0 },
            if archive {
                json!(0)
            } else {
                target["selected"].clone()
            }
        );
        assert_eq!(target["native_valid"], true);
        if target["source"] == true {
            assert_eq!(
                object.geometry(),
                &Geometry::Point(native::point(&target["point"]))
            );
            assert_eq!(object.attributes().name(), Some("sentinel"));
            assert_eq!(object.group_ids().len(), 1);
            continue;
        }
        assert_eq!(
            object.attributes(),
            &ObjectAttributes::on_layer(object.attributes().layer_id())
        );
        assert!(object.group_ids().is_empty());
        assert_eq!(
            target["attributes"],
            json!({"name":"","layer":"Plane output","color_source":"ColorFromLayer","color":[0,0,0],"groups":[]})
        );
        match object.geometry() {
            Geometry::PointCloud(cloud) => {
                assert_eq!(
                    cloud.points().len(),
                    target["points"].as_array().unwrap().len()
                );
                for (a, b) in cloud
                    .points()
                    .iter()
                    .zip(target["points"].as_array().unwrap())
                {
                    error = error.max(a.distance_to(native::point(b)).unwrap());
                }
            }
            Geometry::Brep(brep) => {
                error = error.max(check_brep(brep, target));
            }
            other => panic!("unexpected grid output {other:?}"),
        }
    }
    error
}

pub fn check_brep(brep: &viboceros_geometry::Brep, target: &Value) -> f64 {
    let mut error = 0_f64;
    let topology = &target["brep"]["topology"];
    assert_eq!(brep.is_solid(), topology["solid"]);
    assert_eq!(brep.vertices().len(), topology["vertices"]);
    assert_eq!(
        brep.edges().len(),
        target["brep"]["edges"].as_array().unwrap().len()
    );
    assert_eq!(brep.faces().len(), target["face_count"]);
    let vertices = sorted_points(brep.vertices().iter().map(|v| v.point()));
    let target_vertices = sorted_points(
        target["brep"]["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| native::point(&v["point"])),
    );
    for (a, b) in vertices.iter().zip(target_vertices) {
        error = error.max(a.distance_to(b).unwrap());
    }
    let key = |s: &NurbsSurface| {
        [
            *s.domain_u().start(),
            *s.domain_v().start(),
            *s.domain_u().end(),
            *s.domain_v().end(),
        ]
    };
    let mut faces = brep.faces().iter().collect::<Vec<_>>();
    faces.sort_by(|a, b| key(a.surface()).partial_cmp(&key(b.surface())).unwrap());
    let mut indices = (0..faces.len()).collect::<Vec<_>>();
    indices.sort_by(|&a, &b| {
        key(&native::surface(&target["surfaces"][a]))
            .partial_cmp(&key(&native::surface(&target["surfaces"][b])))
            .unwrap()
    });
    for (face, index) in faces.iter().zip(indices) {
        error = error.max(surface_error(face.surface(), &target["surfaces"][index]));
        let native = &topology["faces"][index];
        assert_eq!(face.is_reversed(), native["reversed"]);
        assert_eq!(
            face.loops().len(),
            native["loops"].as_array().unwrap().len()
        );
        for (boundary, target) in face.loops().iter().zip(native["loops"].as_array().unwrap()) {
            assert_eq!(
                boundary.trims().len(),
                target["trims"].as_array().unwrap().len()
            );
            let mut types = boundary
                .trims()
                .iter()
                .map(|t| format!("{:?}", t.trim_type()))
                .collect::<Vec<_>>();
            let mut target = target["trims"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t["type"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            types.sort();
            target.sort();
            assert_eq!(types, target);
        }
    }
    error
}
