//! Full corner charts, trims and topology, plus the established document checks.
use super::native;
use serde_json::{Value, json};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Brep, Point3, SurfaceExtensionEdge, SurfaceIso};

// These native zero-volume solids have no certified spatial orientation here.
// Keep the exceptions explicit; all geometry and topology checks still apply.
pub const FACE_SENSE_GAPS: &[&str] = &[
    "tiny_translated",
    "follow_tiny_translated",
    "shift_01_forward",
    "shift_02_forward",
    "shift_07_forward",
    "shift_08_forward",
    "shift_09_forward",
    "shift_10_forward",
];

pub fn tokens(v: &Value) -> Vec<String> {
    let t = &v["input_trace"][0];
    assert_eq!(t["completed"], t["expected"]);
    assert!(t["errors"].as_array().unwrap().is_empty());
    t["sent"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            s["token"]
                .as_str()
                .unwrap()
                .trim_start_matches('_')
                .to_owned()
        })
        .collect()
}
fn point_error(a: Point3, v: &Value) -> f64 {
    a.distance_to(native::point(v)).unwrap()
}
fn scalar(a: f64, v: &Value) -> f64 {
    (a - v.as_f64().unwrap()).abs()
}
pub fn check(doc: &Document, v: &Value, phase: &str) -> f64 {
    check_with_face_sense(doc, v, phase, false).0
}
pub fn check_with_face_sense(
    doc: &Document,
    v: &Value,
    phase: &str,
    allow_ambiguous: bool,
) -> (f64, bool) {
    let mut surface_view = doc.clone();
    let mut error = 0_f64;
    let mut face_sense_matches = true;
    for (o, target) in doc.objects().zip(v[phase].as_array().unwrap()) {
        if target["source"] == true {
            continue;
        }
        let brep = match o.geometry() {
            Geometry::Brep(b) => {
                surface_view
                    .replace_object_geometries([(
                        o.id(),
                        Geometry::NurbsSurface(b.faces()[0].surface().clone()),
                    )])
                    .unwrap();
                b.clone()
            }
            Geometry::NurbsSurface(s) => {
                Brep::try_surface_face_with_native_edge_parameters(s.clone(), doc.tolerance())
                    .unwrap()
            }
            _ => panic!("unexpected corner surface"),
        };
        let b = &target["brep"];
        let topology = &b["topology"];
        assert_eq!(brep.faces().len(), 1);
        assert_eq!(brep.is_solid(), topology["solid"]);
        assert_eq!(brep.vertices().len(), topology["vertices"]);
        assert_eq!(brep.edges().len(), b["edges"].as_array().unwrap().len());
        for (a, b) in brep
            .vertices()
            .iter()
            .zip(b["vertices"].as_array().unwrap())
        {
            assert_eq!(a.tolerance(), b["tolerance"]);
            error = error.max(point_error(a.point(), &b["point"]));
        }
        for (i, (a, b)) in brep
            .edges()
            .iter()
            .zip(b["edges"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(json!(a.vertices()), topology["edges"][i]);
            let d = &b["curve"]["definition"];
            assert_eq!(a.tolerance(), b["tolerance"]);
            assert_eq!(
                a.curve().control_points().len(),
                d["control_points"].as_array().unwrap().len()
            );
            assert_eq!(a.curve().degree(), d["degree"]);
            assert_eq!(
                a.curve().knots().len(),
                d["knots"].as_array().unwrap().len()
            );
            for (x, y) in a.curve().knots().iter().zip(d["knots"].as_array().unwrap()) {
                error = error.max(scalar(*x, y));
            }
            for (x, y) in a
                .curve()
                .control_points()
                .iter()
                .zip(d["control_points"].as_array().unwrap())
            {
                assert_eq!(x.weight(), y["weight"].as_f64().unwrap());
                error = error.max(point_error(x.point(), &y["point"]));
            }
        }
        let f = &brep.faces()[0];
        for (i, side) in [
            SurfaceExtensionEdge::South,
            SurfaceExtensionEdge::East,
            SurfaceExtensionEdge::North,
            SurfaceExtensionEdge::West,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                f.surface().has_coincident_boundary_controls(side),
                target["singular"][i]
            );
        }
        if f.is_reversed() != topology["faces"][0]["reversed"].as_bool().unwrap() {
            assert!(allow_ambiguous, "native face sense differs");
            assert!(brep.is_solid());
            assert_eq!(
                brep.solid_orientation().unwrap(),
                viboceros_geometry::BrepSolidOrientation::Unknown
            );
            face_sense_matches = false;
        }
        assert_eq!(f.loops().len(), 1);
        let native_trims = b["faces"][0]["loops"][0].as_array().unwrap();
        let trims = f.loops()[0].trims();
        assert_eq!(trims.len(), native_trims.len());
        for (i, (a, d)) in trims.iter().zip(native_trims).enumerate() {
            let t = &topology["faces"][0]["loops"][0]["trims"][i];
            assert_eq!(json!(a.vertices()), t["vertices"]);
            assert_eq!(json!(a.edge()), t["edge"]);
            assert_eq!(a.is_reversed_3d(), t["reversed"]);
            assert_eq!(format!("{:?}", a.trim_type()), t["type"].as_str().unwrap());
            let iso = match a.iso() {
                SurfaceIso::NotIso => 0,
                SurfaceIso::InteriorUConstant => 1,
                SurfaceIso::InteriorVConstant => 2,
                SurfaceIso::West => 3,
                SurfaceIso::South => 4,
                SurfaceIso::East => 5,
                SurfaceIso::North => 6,
            };
            assert_eq!(json!(iso), d["iso"]);
            let d = &d["definition"];
            assert_eq!(a.curve().degree(), d["degree"]);
            assert_eq!(
                a.curve().knots().len(),
                d["knots"].as_array().unwrap().len()
            );
            assert_eq!(
                a.curve().control_points().len(),
                d["control_points"].as_array().unwrap().len()
            );
            for (x, y) in a.curve().knots().iter().zip(d["knots"].as_array().unwrap()) {
                error = error.max(scalar(*x, y));
            }
            for (x, y) in a
                .curve()
                .control_points()
                .iter()
                .zip(d["control_points"].as_array().unwrap())
            {
                assert_eq!(x.weight(), y["weight"].as_f64().unwrap());
                error = error.max(
                    (x.point().x() - y["point"][0].as_f64().unwrap())
                        .hypot(x.point().y() - y["point"][1].as_f64().unwrap()),
                );
            }
        }
    }
    // The shared checker verifies complete underlying NURBS charts and a 9x9
    // parameter grid. The original B-rep's trim and edge data are checked above.
    (
        error.max(native::check(&surface_view, v, phase)),
        face_sense_matches,
    )
}
