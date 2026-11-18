use super::*;
use serde_json::Value;

mod circular;

#[test]
fn pipe_subd_matches_rhino_control_positions_and_topology() {
    // Extracted from Rhino 8 using ReadCommandFile and public RhinoCommon
    // SubD vertex, edge and face enumeration.
    let cases: Vec<Value> = serde_json::from_str(include_str!("rhino.json")).unwrap();
    for row in cases {
        let case = row["case"].as_str().unwrap();
        let rail: [[Real; 3]; 2] = serde_json::from_value(row["rail"].clone()).unwrap();
        let outputs = row["outputs"].as_array().unwrap();
        let mut doc = Document::default();
        let tol = doc.tolerance();
        let [start, end] = rail.map(|p| Point3::try_from(p).unwrap());
        let line = LineSegment::try_new(start, end, tol).unwrap();
        let id = doc.add_geometry(Geometry::Line(line)).unwrap();
        let rail = Rail::capture(&doc, id, None).unwrap();
        let mut input = Input::new(Preferences::default());
        input.start_sizes(false);
        let mut complete = false;
        for token in row["inputs"].as_array().unwrap() {
            assert!(!complete, "{case}: finished early");
            let result = if let Some(text) = token.as_str() {
                match input.option(text).unwrap() {
                    Some(result) => Ok(result),
                    None if text == "Enter" => input.enter(),
                    None => input.number(text.parse().unwrap()),
                }
            } else {
                let coords: [Real; 3] = serde_json::from_value(token.clone()).unwrap();
                input.point(&rail, Point3::try_from(coords).unwrap(), tol)
            };
            complete = result.unwrap_or_else(|error| panic!("{case}: {error}"));
        }
        assert!(complete, "{case}");
        let actual = input
            .geometries(&[rail], true, tol)
            .unwrap_or_else(|error| panic!("{case}: {error}"));
        assert_eq!(actual.len(), outputs.len(), "{case}");
        for (geometry, expected) in actual.into_iter().zip(outputs) {
            assert_capture(geometry, expected, case);
        }
    }
}

fn assert_capture(geometry: Geometry, capture: &Value, case: &str) {
    let Geometry::SubD(subd) = geometry else {
        panic!("{case}: expected SubD");
    };
    let points: Vec<[Real; 3]> = serde_json::from_value(capture["points"].clone()).unwrap();
    let faces: Vec<Vec<u32>> = serde_json::from_value(capture["faces"].clone()).unwrap();
    let vertex_tags: Vec<u8> = serde_json::from_value(capture["vertex_tags"].clone()).unwrap();
    let mut edge_tags: Vec<u8> = serde_json::from_value(capture["edge_tags"].clone()).unwrap();
    assert_eq!(subd.control_points().len(), points.len(), "{case}");
    for (i, (actual, expected)) in subd.control_points().iter().zip(points).enumerate() {
        let expected = Point3::try_from(expected).unwrap();
        assert!(
            actual.distance_to(expected).unwrap() < 1e-9,
            "{case} control {i}: {actual:?} != {expected:?}"
        );
    }
    assert_eq!(subd.faces(), faces, "{case}");
    assert_eq!(subd.vertex_tags(), vertex_tags, "{case}");
    let mut tags = subd.edge_tags().to_vec();
    tags.sort_unstable();
    edge_tags.sort_unstable();
    assert_eq!(tags, edge_tags, "{case}");
}

#[test]
fn pipe_subd_positional_output_uses_the_same_cage_as_the_size_getter() {
    let mut doc = Document::default();
    let tol = doc.tolerance();
    let [a, b] = [[0., 0., 0.], [0., 0., 12.]].map(|p| Point3::try_from(p).unwrap());
    let id = doc
        .add_geometry(Geometry::Line(LineSegment::try_new(a, b, tol).unwrap()))
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(
            &mut doc,
            &format!("Pipe {id} 2 4 Stations=0.5:3 WallThickness=1 Output=SubD"),
        )
        .unwrap();
    let Geometry::SubD(positional) = doc.objects().last().unwrap().geometry() else {
        panic!("expected SubD");
    };
    let rail = Rail::capture(&doc, id, None).unwrap();
    let mut input = Input::new(Preferences::default());
    input.start_sizes(false);
    input.option("Output=SubD").unwrap();
    input.option("WallThickness=1").unwrap();
    input.number(2.).unwrap();
    input.number(4.).unwrap();
    input.point(&rail, a.midpoint(b).unwrap(), tol).unwrap();
    input.number(3.).unwrap();
    input.enter().unwrap();
    let interactive = input.geometries(&[rail], false, tol).unwrap();
    let [Geometry::SubD(interactive)] = interactive.as_slice() else {
        panic!("expected one SubD");
    };
    assert_eq!(positional.control_points(), interactive.control_points());
    assert_eq!(positional.faces(), interactive.faces());
    assert_eq!(positional.edge_tags(), interactive.edge_tags());
}
