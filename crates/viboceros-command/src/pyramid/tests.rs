use super::{Input, Kind};
use crate::{CommandContext, CommandRegistry};
use serde_json::Value;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Brep, Point3};

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
    let name = command.split_whitespace().next().unwrap().to_owned();
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
    verify_brep(brep, case);
    assert_eq!(
        doc.undo().unwrap().as_deref(),
        Some(name.as_str()),
        "{label}"
    );
    assert!(doc.objects().map(|obj| obj.id()).eq(guide_id), "{label}");
    assert_eq!(
        doc.redo().unwrap().as_deref(),
        Some(name.as_str()),
        "{label}"
    );
    assert_eq!(doc.objects().last().unwrap().id(), id, "{label}");
}

#[test]
fn truncated_pyramid_stages_match_native_rings_edges_and_walls() {
    let captures: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/truncated_pyramid_interactive.json"
    ))
    .unwrap();
    for case in captures["cases"].as_array().unwrap() {
        if let Some(cursor) = case["height_cursor"].as_array() {
            let coords = std::array::from_fn(|i| cursor[i].as_f64().unwrap());
            let brep = frustum_with_cursor(
                case["command"].as_str().unwrap(),
                Point3::try_from(coords).unwrap(),
            );
            verify_brep(&brep, case);
        } else {
            verify_case(case);
        }
    }
}

#[test]
fn truncated_pyramid_matches_native_typed_top_and_free_height_click() {
    // Rhino 8.32 accepted both typed top picks in the live getter.
    let registry = CommandRegistry::with_builtins();
    for (inputs, top_corner) in [
        ("w0,0,0 w4,0,0 w0,0,5 w-1.5,0.5,5", [1.5, 0., 5.]),
        (
            "w1,2,3 w5,2,7 6 w3,5,11",
            [1.7573593128807152, 2., 12.242640687119284],
        ),
    ] {
        let mut doc = Document::default();
        registry
            .execute(
                &mut doc,
                &format!("TruncatedPyramid NumSides=5 DirectionConstraint=Vertical {inputs}"),
            )
            .unwrap();
        let Geometry::Brep(brep) = doc.objects().next().unwrap().geometry() else {
            panic!("expected a frustum for {inputs}");
        };
        assert!(
            brep.vertices()[0]
                .point()
                .distance_to(Point3::try_from(top_corner).unwrap())
                .unwrap()
                < 2e-12
        );
    }

    // The captured click determines both the free orientation and top center.
    let picked = [5.977099236641222, 7.0400763358778615, 0.];
    let brep = frustum_with_cursor(
        "TruncatedPyramid NumSides=5 DirectionConstraint=None w1,2,3 w5,2,3 \
         w5.977099236641222,7.0400763358778615,0 2",
        Point3::try_from(picked).unwrap(),
    );
    let expected = Point3::try_from([4.554022761801278, 8.445371090282473, 0.]).unwrap();
    assert!(brep.vertices()[0].point().distance_to(expected).unwrap() < 2e-12);
}

fn frustum_with_cursor(command: &str, cursor: Point3) -> Brep {
    let doc = Document::default();
    let mut input = Input::new(
        Kind::TruncatedPyramid,
        CommandContext::default().construction_plane,
        Default::default(),
        doc.tolerance(),
    );
    for token in command.split_whitespace().skip(1) {
        if input.option(token).unwrap() {
            continue;
        }
        input.observe_cursor(cursor);
        let result = if let Ok(number) = token.parse() {
            input.number(number)
        } else {
            let (point, _) = crate::parse_point(&[token]).unwrap();
            input.point(point)
        };
        if let Some(definition) = result.unwrap() {
            return definition.brep(doc.tolerance()).unwrap();
        }
    }
    panic!("unfinished input {command}");
}

fn verify_brep(brep: &Brep, case: &Value) {
    let label = case["id"].as_str().unwrap();
    // For nominally equal slants, a rounding difference can select the other
    // rail as the reference for U. The control net and interval length agree.
    let axial_frustum = case["edge_domains"].is_array() && case["height_cursor"].is_null();
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
    if let Some(domains) = case["edge_domains"].as_array() {
        assert_eq!(domains.len(), brep.edges().len(), "{label}");
        for (index, (edge, expected)) in brep.edges().iter().zip(domains).enumerate() {
            let context = format!("{label} edge {index}");
            let domain = edge.curve().domain();
            let [start, end] = edge.vertices();
            let base_vertex = |i| i == 1 || i > 1 && i % 2 == 0;
            if axial_frustum && base_vertex(start) != base_vertex(end) {
                interval_length(&domain, expected, &context);
            } else {
                coordinates(&[*domain.start(), *domain.end()], expected, &context);
            }
        }
    }
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
        if axial_frustum && index < brep.vertices().len() / 2 {
            interval_length(&u, &expected["domain_u"], &context);
        } else {
            coordinates(&[*u.start(), *u.end()], &expected["domain_u"], &context);
        }
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
}

fn interval_length(actual: &std::ops::RangeInclusive<f64>, expected: &Value, context: &str) {
    let length = expected[1].as_f64().unwrap() - expected[0].as_f64().unwrap();
    assert!(
        (actual.end() - actual.start() - length).abs() <= 2e-12 * length.max(1.0),
        "{context}"
    );
}

#[test]
fn pyramid_free_star_matches_captured_numeric_radius_cursor_rays() {
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
        let mut input = Input::new(Kind::Pyramid, plane, Default::default(), Tolerance::DEFAULT);
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
        let mut input = Input::new(Kind::Pyramid, plane, Default::default(), Tolerance::DEFAULT);
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
