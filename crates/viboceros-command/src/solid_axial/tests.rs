use super::*;
use crate::CommandContext;
use serde_json::json;
use viboceros_document::{Document, SelectionMode};
use viboceros_geometry::{BrepLoopType, Point3, Vector3};

const AXIAL_FIXTURES: &str = include_str!("tests/rhino_primitives.json");
mod tube;

#[test]
fn solid_axial_numeric_height_uses_the_base_normal_at_a_coincident_cursor() {
    let plane = crate::CommandContext::default().construction_plane;
    for kind in [Kind::Cone, Kind::Cylinder] {
        let preferences = Preferences {
            direction: Direction::None,
            ..Preferences::default()
        };
        let mut input = Input::new(kind, plane, preferences);
        input.point(plane.origin()).unwrap();
        input.number(2.).unwrap();
        input.observe_cursor(plane.origin());
        input.number(4.).unwrap();
        let definition = input.definition.unwrap();
        assert_eq!(definition.frame.z_axis(), plane.z_axis());
        assert_eq!(definition.height, 4.);
    }
}

#[test]
fn solid_axial_replays_rhino_faces_caps_and_signed_heights() {
    replay_rhino_primitives(AXIAL_FIXTURES, false);
}

#[test]
fn solid_axial_truncated_cone_replays_rhino_surfaces_and_topology() {
    replay_rhino_primitives(include_str!("tests/rhino_truncated_cone.json"), false);
}

#[test]
#[ignore = "the fitted normal can point in the opposite direction in Rhino"]
fn solid_axial_replays_rhino_signed_height_from_a_clockwise_fit() {
    replay_rhino_primitives(AXIAL_FIXTURES, true);
}

fn replay_rhino_primitives(fixture: &str, signed_fit_orientation_gap: bool) {
    let rows: Vec<serde_json::Value> = serde_json::from_str(fixture).unwrap();
    for row in rows.into_iter().filter(|row| {
        row["signed_fit_orientation_gap"].as_bool().unwrap_or(false) == signed_fit_orientation_gap
    }) {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let mut command = row["command"].as_str().unwrap().to_owned();
        if command.contains("{guide}") {
            registry.execute(&mut doc, "Line 0,0,0 0,10,10").unwrap();
            command = command.replace("{guide}", &doc.objects().last().unwrap().id().to_string());
        }
        if let Some(points) = row["points"].as_array() {
            let ids = points
                .iter()
                .map(|point| {
                    let xyz: [f64; 3] = serde_json::from_value(point.clone()).unwrap();
                    doc.add_geometry(Geometry::Point(Point3::try_from(xyz).unwrap()))
                        .unwrap()
                })
                .collect::<Vec<_>>();
            doc.select_objects_direct(ids, SelectionMode::Replace)
                .unwrap();
        }
        let before = doc.objects().len();
        let mut context = CommandContext::default();
        if let Some(plane) = row.get("construction_plane") {
            let [origin, x, y]: [[f64; 3]; 3] = serde_json::from_value(plane.clone()).unwrap();
            context.construction_plane = Frame3::try_from_directions(
                Point3::try_from(origin).unwrap(),
                Vector3::try_from(x).unwrap(),
                Vector3::try_from(y).unwrap(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap();
        }
        registry
            .execute_in_context(&mut doc, &command, context)
            .unwrap_or_else(|error| panic!("{command}: {error}"));
        let Geometry::Brep(brep) = doc.objects().last().unwrap().geometry() else {
            panic!("{command}: expected a B-rep");
        };
        assert_eq!(
            brep.is_extrusion(),
            row["extrusion"].as_bool().unwrap(),
            "{command}"
        );
        assert_eq!(
            brep.is_solid(),
            row["solid"].as_bool().unwrap(),
            "{command}"
        );
        let faces = row["surfaces"].as_array().unwrap();
        if !brep.is_extrusion() {
            assert_eq!(brep.faces().len(), faces.len(), "{command}");
        }
        for (face, expected) in brep.faces().iter().zip(faces) {
            assert_surface_matches(&command, face.surface(), expected);
        }
        if let Some(expected) = row.get("topology") {
            assert_eq!(&topology(brep), expected, "{command}");
        }
        let name = command.split_whitespace().next().unwrap();
        assert_eq!(doc.undo_label(), Some(name));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), before);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().len(), before + 1);
    }
}

fn topology(brep: &Brep) -> serde_json::Value {
    let faces =
        brep.faces()
            .iter()
            .map(|face| {
                let loops = face.loops().iter().map(|face_loop| {
            let trims = face_loop.trims().iter().map(|trim| {
                json!({
                    "type": format!("{:?}", trim.trim_type()),
                    "vertices": trim.vertices(),
                    "edge": trim.edge(),
                    "reversed": trim.is_reversed_3d(),
                })
            }).collect::<Vec<_>>();
            json!({ "outer": face_loop.loop_type() == BrepLoopType::Outer, "trims": trims })
        }).collect::<Vec<_>>();
                json!({ "reversed": face.is_reversed(), "loops": loops })
            })
            .collect::<Vec<_>>();
    let edges = brep
        .edges()
        .iter()
        .map(|edge| edge.vertices())
        .collect::<Vec<_>>();
    json!({
        "vertices": brep.vertices().len(),
        "solid": brep.is_solid(),
        "faces": faces,
        "edges": edges,
    })
}

fn assert_surface_matches(command: &str, surface: &NurbsSurface, expected: &serde_json::Value) {
    let [u, v]: [usize; 2] = serde_json::from_value(expected["control_count"].clone()).unwrap();
    assert_eq!(
        [
            surface.control_point_count_u(),
            surface.control_point_count_v()
        ],
        [u, v],
        "{command}"
    );
    assert_eq!(
        [surface.degree_u(), surface.degree_v()],
        serde_json::from_value::<[usize; 2]>(expected["degree"].clone()).unwrap(),
        "{command}"
    );
    for (axis, actual) in [
        ("knots_u", surface.knots_u()),
        ("knots_v", surface.knots_v()),
    ] {
        let reference: Vec<f64> = serde_json::from_value(expected[axis].clone()).unwrap();
        assert_eq!(actual.len(), reference.len(), "{command} {axis}");
        for (&actual, expected) in actual.iter().zip(reference) {
            assert!(
                (actual - expected).abs() < 1e-9,
                "{command} {axis}: {actual} != {expected}"
            );
        }
    }
    for (i, native) in expected["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let uv = [i % u, i / u];
        let control = surface.control_point(uv[0], uv[1]).unwrap();
        let point: [f64; 3] = serde_json::from_value(native["point"].clone()).unwrap();
        let point = Point3::try_from(point).unwrap();
        assert!(
            control.point().distance_to(point).unwrap() < 1e-9,
            "{command} CV {uv:?}: {:?} != {point:?}",
            control.point()
        );
        assert!(
            (control.weight() - native["weight"].as_f64().unwrap()).abs() < 1e-12,
            "{command} CV {uv:?} weight"
        );
    }
}
