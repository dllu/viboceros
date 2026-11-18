use crate::CommandRegistry;
use serde_json::Value;
use viboceros_document::{Document, Geometry};

fn coordinates(actual: &[f64], expected: &Value, context: &str) {
    let values = expected.as_array().unwrap();
    assert_eq!(actual.len(), values.len(), "{context}");
    for (axis, (actual, expected)) in actual.iter().zip(values).enumerate() {
        let expected = expected.as_f64().unwrap();
        assert!(
            (actual - expected).abs() <= 2e-12 * expected.abs().max(1.0),
            "{context} axis {axis}: {actual} != {expected}",
        );
    }
}

#[test]
fn pyramid_staged_input_matches_native_rims_faces_and_undo() {
    let captures: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/pyramid_interactive.json"
    ))
    .unwrap();
    for case in captures["cases"].as_array().unwrap() {
        verify_case(case);
    }
}

fn verify_case(case: &Value) {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let label = case["id"].as_str().unwrap();
    let mut command = case["command"].as_str().unwrap().to_owned();
    let guide_id = case["guide"].as_array().map(|ends| {
        let token = |point: &Value| {
            point
                .as_array()
                .unwrap()
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(",")
        };
        registry
            .execute(
                &mut doc,
                &format!("Line {} {}", token(&ends[0]), token(&ends[1])),
            )
            .unwrap();
        let id = doc.objects().next().unwrap().id();
        command = command.replace("{guide}", &id.to_string());
        id
    });
    registry
        .execute(&mut doc, &command)
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    assert_eq!(
        doc.objects().len(),
        1 + usize::from(guide_id.is_some()),
        "{label}"
    );
    let obj = doc.objects().last().unwrap();
    let id = obj.id();
    let Geometry::Brep(brep) = obj.geometry() else {
        panic!("{label}: expected B-rep");
    };
    assert!(!doc.is_selected(id), "{label}");
    assert_eq!(brep.is_solid(), case["solid"].as_bool().unwrap(), "{label}");
    let expected = case["vertices"].as_array().unwrap();
    assert_eq!(brep.vertices().len(), expected.len(), "{label}");
    for (index, (actual, expected)) in brep.vertices().iter().zip(expected).enumerate() {
        coordinates(
            &actual.point().to_array(),
            expected,
            &format!("{label} vertex {index}"),
        );
    }
    let edges = brep
        .edges()
        .iter()
        .map(|edge| edge.vertices())
        .collect::<Vec<_>>();
    assert_eq!(
        serde_json::to_value(edges).unwrap(),
        case["edges"],
        "{label}"
    );
    let reversed = brep
        .faces()
        .iter()
        .map(|face| face.is_reversed())
        .collect::<Vec<_>>();
    assert_eq!(
        serde_json::to_value(reversed).unwrap(),
        case["reversed"],
        "{label}"
    );
    let expected = case["surfaces"].as_array().unwrap();
    assert_eq!(brep.faces().len(), expected.len(), "{label}");
    for (index, (face, expected)) in brep.faces().iter().zip(expected).enumerate() {
        let context = format!("{label} face {index}");
        let surface = face.surface();
        let u = surface.domain_u();
        let v = surface.domain_v();
        coordinates(&[*u.start(), *u.end()], &expected["domain_u"], &context);
        coordinates(&[*v.start(), *v.end()], &expected["domain_v"], &context);
        let controls = expected["controls"].as_array().unwrap();
        assert_eq!(surface.control_points().len(), controls.len(), "{context}");
        for (index, (actual, expected)) in surface.control_points().iter().zip(controls).enumerate()
        {
            coordinates(
                &actual.point().to_array(),
                expected,
                &format!("{context} control {index}"),
            );
            assert_eq!(actual.weight(), 1.0, "{context}");
        }
    }
    assert_eq!(doc.undo().unwrap().as_deref(), Some("Pyramid"), "{label}");
    assert!(doc.objects().map(|obj| obj.id()).eq(guide_id), "{label}");
    assert_eq!(doc.redo().unwrap().as_deref(), Some("Pyramid"), "{label}");
    assert_eq!(doc.objects().last().unwrap().id(), id, "{label}");
}

#[test]
fn pyramid_free_star_matches_captured_numeric_radius_cursor_rays() {
    use super::Input;
    use crate::{CommandContext, picking::PointerRay};
    use viboceros_geometry::{Point3, Tolerance};

    // Rhino 8.32 cursor rays sampled with the default perspective view.
    let cases = [
        (
            [0., 0., 0.],
            [4., 0., 0.],
            [5., 5., 6.],
            [-29.125999250354244, -23.2417812670978, -15.03048770661773],
            [
                -25.341504038264905,
                -25.936269182038014,
                -11.634968683348395,
            ],
            [-0.28253514616289466, 3.1778520213057218, -2.412764062619023],
        ),
        (
            [1., 2., 3.],
            [5., 2., 3.],
            [6., 7., 9.],
            [-61.94413286795555, 0.12409964115165999, -44.47552350710945],
            [-3.5798707425322673, -41.43013418612138, 7.889970758252122],
            [0.4170950857567415, 5.265430744900401, 0.7645618077857153],
        ),
    ];
    let point = |xyz| Point3::try_from(xyz).unwrap();
    let plane = CommandContext::default().construction_plane;
    for (center, outer, apex, ray_start, ray_end, first) in cases {
        let mut input = Input::new(plane, Default::default(), Tolerance::DEFAULT);
        assert!(input.option("DirectionConstraint=None").unwrap());
        assert!(input.option("Star").unwrap());
        input.point(point(center)).unwrap();
        input.point(point(outer)).unwrap();
        let start = point(ray_start);
        let direction = start.vector_to(point(ray_end)).unwrap();
        let ray = PointerRay::try_new(start, direction, false).unwrap();
        let on_plane = ray
            .on_plane(input.star_roll_plane().unwrap())
            .unwrap()
            .unwrap();
        input.observe_cursor(on_plane);
        input.number(2.).unwrap();
        let brep = input
            .point(point(apex))
            .unwrap()
            .unwrap()
            .brep(Tolerance::DEFAULT)
            .unwrap();
        assert!(
            brep.vertices()[0]
                .point()
                .distance_to(point(first))
                .unwrap()
                < 2e-12
        );
    }
}

#[test]
fn pyramid_free_star_matches_the_planar_radius_pick_and_typed_point() {
    use super::Input;
    use crate::CommandContext;
    use viboceros_geometry::{Point3, Tolerance};

    let cases = [
        (
            [1.99618320610687, 1.0095419847328235, 0.],
            [-2.1190554624426414, -1.5203542228311382, 3.0328414043948175],
        ),
        (
            [2., 1., 0.],
            [-2.131023341499227, -1.5065747382259083, 3.03133173310428],
        ),
    ];
    let point = |xyz| Point3::try_from(xyz).unwrap();
    let plane = CommandContext::default().construction_plane;
    for (second, first) in cases {
        let mut input = Input::new(plane, Default::default(), Tolerance::DEFAULT);
        input.option("DirectionConstraint=None").unwrap();
        input.option("Star").unwrap();
        input.point(point([0., 0., 0.])).unwrap();
        input.point(point([4., 0., 0.])).unwrap();
        input.point(point(second)).unwrap();
        let brep = input
            .point(point([5., 5., 6.]))
            .unwrap()
            .unwrap()
            .brep(Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(brep.faces().len(), 11);
        assert!(
            brep.vertices()[0]
                .point()
                .distance_to(point(first))
                .unwrap()
                < 2e-12
        );
    }
}
