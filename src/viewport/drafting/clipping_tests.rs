use super::*;
use viboceros_drafting::ObjectSnapKind;
use viboceros_geometry::{
    Circle3, CircularArc3, Ellipse3, LineSegment, Polyline3, UnitVector3, WeightedPoint3,
};

fn point(value: &serde_json::Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(value.clone()).unwrap()).unwrap()
}

fn vector(value: &serde_json::Value) -> UnitVector3 {
    Vector3::try_from(serde_json::from_value::<[Real; 3]>(value.clone()).unwrap())
        .unwrap()
        .normalized_nonzero()
        .unwrap()
}

fn source(value: &serde_json::Value) -> Geometry {
    match value["type"].as_str().unwrap() {
        "line" => Geometry::Line(
            LineSegment::try_new(
                point(&value["start"]),
                point(&value["end"]),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        "arc" => Geometry::Arc(
            CircularArc3::try_from_three_points(
                point(&value["points"][0]),
                point(&value["points"][1]),
                point(&value["points"][2]),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        "nurbs" => Geometry::NurbsCurve(
            NurbsCurve::try_new_rational(
                value["degree"].as_u64().unwrap() as usize,
                value["control_points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|control| {
                        WeightedPoint3::try_new(
                            point(&control["point"]),
                            control["weight"].as_f64().unwrap(),
                        )
                        .unwrap()
                    })
                    .collect(),
                serde_json::from_value(value["knots"].clone()).unwrap(),
            )
            .unwrap(),
        ),
        "ellipse" => Geometry::Ellipse(
            Ellipse3::try_new(
                point(&value["center"]),
                value["radius_x"].as_f64().unwrap(),
                value["radius_y"].as_f64().unwrap(),
                vector(&value["x_axis"]),
                vector(&value["y_axis"]),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        "circle" => Geometry::Circle(
            Circle3::try_from_frame(
                point(&value["center"]),
                value["radius"].as_f64().unwrap(),
                vector(&value["x_axis"]),
                vector(&value["normal"]),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        "polyline" => Geometry::Polyline(
            Polyline3::try_new(
                value["vertices"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(point)
                    .collect(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        kind => panic!("unsupported captured snap source: {kind}"),
    }
}

fn captures() -> (serde_json::Value, serde_json::Value) {
    (
        serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/snap_depth_visibility.json"
        ))
        .unwrap(),
        serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/snap_depth_visibility.json"
        ))
        .unwrap(),
    )
}

fn modes(operation: &serde_json::Value) -> ObjectSnapModes {
    operation["persistent_snaps"]
        .as_array()
        .unwrap()
        .iter()
        .fold(ObjectSnapModes::NONE, |modes, name| {
            modes.with(
                match name.as_str().unwrap() {
                    "Cen" => ObjectSnapKind::Center,
                    "Mid" => ObjectSnapKind::Mid,
                    "End" => ObjectSnapKind::End,
                    "Near" => ObjectSnapKind::Near,
                    name => panic!("unsupported captured snap mode: {name}"),
                },
                true,
            )
        })
}

/// Independent closed-form ray/circle intersection for the fixture's YZ arcs.
/// Their images lie on one screen line; first project the cursor onto that line,
/// then solve the quadratic on the original supporting circle's visible cap.
fn arc_near_reference(operation: &serde_json::Value, frame: &serde_json::Value) -> Point3 {
    let matrix: [[Real; 4]; 4] = serde_json::from_value(frame["world_to_screen"].clone()).unwrap();
    let pointer: [Real; 2] = serde_json::from_value(frame["click_client"].clone()).unwrap();
    let origin: [Real; 2] = std::array::from_fn(|i| matrix[i][3] / matrix[3][3]);
    let direction: [Real; 2] = std::array::from_fn(|i| matrix[i][1] / -matrix[3][2]);
    let q = ((pointer[0] - origin[0]) * direction[0] + (pointer[1] - origin[1]) * direction[1])
        / (direction[0] * direction[0] + direction[1] * direction[1]);
    let a = point(&operation["sources"][0]["points"][0]);
    let middle = point(&operation["sources"][0]["points"][1]);
    let dz = a.z() - middle.z();
    let radius = (a.y() * a.y() + dz * dz) / (2. * dz);
    let center = middle.z() + radius;
    let camera = operation["camera_pose"]["location"][2].as_f64().unwrap();
    let s = camera - center;
    let coefficient = 1. + q * q;
    let depth = (s + (s * s + coefficient * (radius * radius - s * s)).sqrt()) / coefficient;
    Point3::try_new(0., q * depth, camera - depth).unwrap()
}

#[test]
fn curve_hover_snap_targets_match_rhino_outside_depth_planes_and_behind_the_camera() {
    let (request, observed) = captures();
    let operations = request["operations"].as_array().unwrap();
    let rows = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), 30);
    assert_eq!(rows.len(), operations.len());
    let mut clipped = 0;
    let mut behind = 0;
    for (operation, row) in operations.iter().zip(rows) {
        assert_eq!(operation["id"], row["id"]);
        let expected = &row["value"];
        let view = super::super::clip_tests::captured_view(&expected["frame"]["clipping_camera"]);
        let rect = view.last_rect.unwrap();
        let pointer: [f32; 2] =
            serde_json::from_value(expected["frame"]["click_client"].clone()).unwrap();
        let pointer = Pos2::new(pointer[0], pointer[1]);
        let mut document = Document::default();
        let ids: Vec<_> = operation["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| document.add_geometry(source(value)).unwrap())
            .collect();
        let snap = view.object_snap(pointer, rect, &document, modes(operation));
        if expected["kind"] == "None" {
            assert!(snap.is_none(), "{} got {snap:?}", row["id"]);
            continue;
        }
        let snap = snap.unwrap_or_else(|| panic!("{} missing {}", row["id"], expected["kind"]));
        let kind = match expected["kind"].as_str().unwrap() {
            "Center" => ObjectSnapKind::Center,
            "Midpoint" => ObjectSnapKind::Mid,
            "End" => ObjectSnapKind::End,
            "Near" => ObjectSnapKind::Near,
            kind => panic!("unexpected Rhino snap: {kind}"),
        };
        assert_eq!(snap.kind(), kind, "{}", row["id"]);
        assert_eq!(
            snap.object_id(),
            ids[expected["source"].as_u64().unwrap() as usize]
        );
        let target = point(&expected["point"]);
        let epsilon = if operation["sources"][0]["type"] == "arc" && kind == ObjectSnapKind::Near {
            let reference = arc_near_reference(operation, &expected["frame"]);
            assert!(
                snap.point().distance_to(reference).unwrap() < 1e-12,
                "{} {:?} vs independent ray/circle {:?}",
                row["id"],
                snap.point(),
                reference
            );
            1e-8 // Public Rhino Near differs from the analytic optimum by up to 3.1e-9.
        } else {
            1e-9
        };
        assert!(
            snap.point().distance_to(target).unwrap() < epsilon,
            "{} {:?} vs {:?}",
            row["id"],
            snap.point(),
            target
        );
        let cursor = view
            .drafting_cursor(
                pointer,
                rect,
                &document,
                DraftingInput {
                    active: true,
                    osnap: modes(operation),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(cursor.point, snap.point());
        assert_eq!(cursor.source_point, snap.point());
        if !expected["clipping"]["queries"][1]["visible"]
            .as_bool()
            .unwrap()
        {
            assert!(
                !view.point_within_display_depth(snap.point()),
                "{}",
                row["id"]
            );
            clipped += 1;
        }
        if view.view_depth(snap.point()) < 0. {
            assert!(view.project(snap.point(), rect).is_none());
            behind += 1;
        }
        let matrix: [[Real; 4]; 4] =
            serde_json::from_value(expected["frame"]["world_to_screen"].clone()).unwrap();
        let p = target.to_array();
        let homogeneous = matrix
            .map(|row| row[0].mul_add(p[0], row[1].mul_add(p[1], row[2].mul_add(p[2], row[3]))));
        let image = view.project_snap_target(snap.point(), rect).unwrap();
        assert!((Real::from(image.x) - homogeneous[0] / homogeneous[3]).abs() < 1e-3);
        assert!((Real::from(image.y) - homogeneous[1] / homogeneous[3]).abs() < 1e-3);
    }
    assert_eq!(clipped, 14);
    assert_eq!(behind, 7);
}

#[test]
fn camera_crossing_near_matches_rhino_endpoint_reversal_aperture_and_front_controls() {
    for (request, observed, expected_counts) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/snap_crossing_line_near.json"),
            include_str!("../../../tools/rhino_oracle/observations/snap_crossing_line_near.json"),
            (22, 5),
        ),
        (
            include_str!(
                "../../../tools/rhino_oracle/fixtures/snap_crossing_straight_sources.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/observations/snap_crossing_straight_sources.json"
            ),
            (48, 24),
        ),
    ] {
        replay_near_captures(request, observed, expected_counts);
    }
}

fn replay_near_captures(request: &str, observed: &str, expected_counts: (usize, usize)) {
    let request: serde_json::Value = serde_json::from_str(request).unwrap();
    let observed: serde_json::Value = serde_json::from_str(observed).unwrap();
    let operations = request["operations"].as_array().unwrap();
    let rows = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), expected_counts.0 + expected_counts.1);
    assert_eq!(rows.len(), operations.len());
    let (mut hits, mut misses) = (0, 0);
    for (operation, row) in operations.iter().zip(rows) {
        assert_eq!(operation["id"], row["id"]);
        let expected = &row["value"];
        let view = super::super::clip_tests::captured_view(&expected["frame"]["clipping_camera"]);
        let rect = view.last_rect.unwrap();
        let pointer: [f32; 2] =
            serde_json::from_value(expected["frame"]["click_client"].clone()).unwrap();
        let mut document = Document::default();
        let id = document
            .add_geometry(source(&operation["sources"][0]))
            .unwrap();
        let snap = view.object_snap(
            Pos2::new(pointer[0], pointer[1]),
            rect,
            &document,
            modes(operation),
        );
        if expected["kind"] == "None" {
            assert!(snap.is_none(), "{} got {snap:?}", row["id"]);
            misses += 1;
        } else {
            let snap = snap.unwrap_or_else(|| panic!("{} missing Near", row["id"]));
            assert_eq!(snap.kind(), ObjectSnapKind::Near);
            assert_eq!(snap.object_id(), id);
            assert!(
                snap.point().distance_to(point(&expected["point"])).unwrap() < 1e-9,
                "{} {:?} vs {}",
                row["id"],
                snap.point(),
                expected["point"]
            );
            hits += 1;
        }
    }
    assert_eq!((hits, misses), expected_counts);
}

#[test]
fn camera_plane_targets_match_rhino_conic_admission_and_keep_singular_overlay_labels() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/snap_camera_plane_targets.json"
    ))
    .unwrap();
    let observed: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/snap_camera_plane_targets.json"
    ))
    .unwrap();
    let operations = request["operations"].as_array().unwrap();
    let rows = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), 15);
    assert_eq!(rows.len(), operations.len());
    let context = egui::Context::default();
    let mut misses = 0;
    for (operation, row) in operations.iter().zip(rows) {
        assert_eq!(operation["id"], row["id"]);
        let expected = &row["value"];
        let view = super::super::clip_tests::captured_view(&expected["frame"]["clipping_camera"]);
        let rect = view.last_rect.unwrap();
        let pointer: [f32; 2] =
            serde_json::from_value(expected["frame"]["click_client"].clone()).unwrap();
        let pointer = Pos2::new(pointer[0], pointer[1]);
        let mut document = Document::default();
        // Rhino stores ellipses as NURBS. Reconstruct the actual captured net:
        // scaling around an origin can round a one-ULP center shift away.
        let geometry = if operation["sources"][0]["type"] == "ellipse" {
            let mut definition = expected["before"][0]["nurbs_definition"].clone();
            definition["type"] = "nurbs".into();
            source(&definition)
        } else {
            source(&operation["sources"][0])
        };
        let id = document.add_geometry(geometry).unwrap();
        let input = DraftingInput {
            active: true,
            osnap: modes(operation),
            ..Default::default()
        };
        let snap = view.object_snap(pointer, rect, &document, modes(operation));
        if expected["kind"] == "None" {
            assert!(snap.is_none(), "{} got {snap:?}", row["id"]);
            misses += 1;
            continue;
        }
        let snap = snap.unwrap_or_else(|| panic!("{} missing snap", row["id"]));
        assert_eq!(snap.object_id(), id);
        assert_eq!(
            snap.kind(),
            if expected["kind"] == "Midpoint" {
                ObjectSnapKind::Mid
            } else {
                ObjectSnapKind::Center
            }
        );
        assert!(
            snap.point().distance_to(point(&expected["point"])).unwrap() < 1e-9,
            "{}",
            row["id"]
        );
        assert!(
            view.project_snap_target(snap.point(), rect).is_none(),
            "{}",
            row["id"]
        );
        let cursor = view
            .drafting_cursor(pointer, rect, &document, input)
            .unwrap();
        let label = snap.kind().label();
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ui| {
                view.paint_drafting(ui.painter(), rect, input, cursor);
            },
        );
        assert!(
            output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.text() == label && text.pos.is_finite())),
            "{}",
            row["id"]
        );
        output.drop_without_applying_deltas();
    }
    assert_eq!(misses, 4);
}

#[test]
fn admitted_behind_camera_snaps_keep_their_overlay_labels() {
    let (request, observed) = captures();
    for name in ["arc-center-behind-camera-hover", "crossing-line-mid-behind"] {
        let operation = request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|op| op["id"] == name)
            .unwrap();
        let expected = &observed["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == name)
            .unwrap()["value"];
        let view = super::super::clip_tests::captured_view(&expected["frame"]["clipping_camera"]);
        let rect = view.last_rect.unwrap();
        let mut document = Document::default();
        document
            .add_geometry(source(&operation["sources"][0]))
            .unwrap();
        let pointer: [f32; 2] =
            serde_json::from_value(expected["frame"]["click_client"].clone()).unwrap();
        let input = DraftingInput {
            active: true,
            osnap: modes(operation),
            ..Default::default()
        };
        let cursor = view
            .drafting_cursor(Pos2::new(pointer[0], pointer[1]), rect, &document, input)
            .unwrap();
        let label = cursor.object_snap.unwrap().kind().label();
        let context = egui::Context::default();
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ui| {
                view.paint_drafting(ui.painter(), rect, input, cursor);
            },
        );
        assert!(
            output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.text() == label)),
            "{name}"
        );
        output.drop_without_applying_deltas();
    }
}
