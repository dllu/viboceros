//! Whole-file scenes: exact points, NURBS coefficients, finite samples and metadata.
use super::{grid, native};
use serde_json::{Value, json};
use viboceros_document::{Document, Geometry, ObjectColorSource};

pub fn check(doc: &Document, expected: &Value, archive: bool) -> f64 {
    let expected = expected.as_array().unwrap();
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
        match object.geometry() {
            Geometry::Point(p) => {
                error = error.max(p.distance_to(native::point(&target["point"])).unwrap())
            }
            Geometry::Brep(brep) => error = error.max(grid::check_brep(brep, target)),
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
            geometry => {
                let actual = geometry.curve_ref().unwrap().to_nurbs().unwrap();
                let target = native::curve(&target["curve"]);
                assert_eq!(actual.degree(), target.degree());
                assert_eq!(actual.control_points().len(), target.control_points().len());
                for (a, b) in actual.control_points().iter().zip(target.control_points()) {
                    assert_eq!(a.weight(), b.weight());
                    error = error.max(a.point().distance_to(b.point()).unwrap());
                }
                assert_eq!(actual.knots().len(), target.knots().len());
                for (a, b) in actual.knots().iter().zip(target.knots()) {
                    error = error.max((a - b).abs());
                }
                for i in 0..=32 {
                    let at = |c: &viboceros_geometry::NurbsCurve| {
                        c.evaluate(c.parameter_at(i as f64 / 32.).unwrap()).unwrap()
                    };
                    error = error.max(at(&actual).distance_to(at(&target)).unwrap());
                }
            }
        }
    }
    error
}
