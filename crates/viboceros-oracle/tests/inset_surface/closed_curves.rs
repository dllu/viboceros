use super::*;

#[test]
fn expanding_holes_match_native_source_crossings() {
    cusps::replay_family("inset_fan_basis", (6, 6), None);
}

#[test]
fn expanding_hole_commands_match_native_curves_pieces_and_joined_outputs() {
    let cases = common::cases_for("inset_fan_commands")
        .into_iter()
        .filter(|(op, _)| op["scene"] == "hole_stadium")
        .collect();
    cusps::replay_command_cases(cases, (3, 0));
}

fn adjacent_cases() -> Vec<(serde_json::Value, serde_json::Value)> {
    common::cases_for("inset_closed_network_faces")
        .into_iter()
        .chain(common::cases_for("inset_closed_network"))
        .filter(|(op, _)| op["mode"] == "sdk")
        .collect()
}

#[test]
fn expanding_holes_partition_unselected_coplanar_faces() {
    cusps::replay_cases(adjacent_cases(), (2, 0), None);
}

#[test]
fn adjacent_selected_closed_networks_decline_without_mutating_the_source() {
    let (_, native) = adjacent_cases().pop().unwrap();
    let source = source(&native["before"]);
    let before = source.clone();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let error = source
        .try_inset_planar_faces(&[0, 1], 1.5, true, tolerance)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("adjacent selected coplanar faces"),
        "{error}"
    );
    assert_eq!(source, before);
}

fn reflected_u(face: &viboceros_geometry::BrepFace) -> viboceros_geometry::BrepFace {
    use viboceros_geometry::{BrepFace, BrepLoop, BrepTrim, NurbsCurve2, Point2, WeightedPoint2};
    let loops = face
        .loops()
        .iter()
        .map(|boundary| {
            let trims = boundary
                .trims()
                .iter()
                .rev()
                .map(|trim| {
                    let curve = trim.curve();
                    let points = curve
                        .control_points()
                        .iter()
                        .map(|p| {
                            WeightedPoint2::try_new(
                                Point2::try_new(-p.point().x(), p.point().y()).unwrap(),
                                p.weight(),
                            )
                            .unwrap()
                        })
                        .collect();
                    let curve = NurbsCurve2::try_new_rational(
                        curve.degree(),
                        points,
                        curve.knots().to_vec(),
                    )
                    .unwrap()
                    .reversed()
                    .unwrap();
                    let [a, b] = trim.vertices();
                    BrepTrim::try_new(
                        [b, a],
                        trim.edge(),
                        !trim.is_reversed_3d(),
                        curve,
                        trim.trim_type(),
                        trim.iso(),
                        trim.tolerance(),
                    )
                    .unwrap()
                })
                .collect();
            BrepLoop::try_new(boundary.loop_type(), trims).unwrap()
        })
        .collect();
    BrepFace::try_new(
        face.surface().try_reversed_u().unwrap(),
        !face.is_reversed(),
        loops,
    )
    .unwrap()
}

#[test]
fn neighbor_subdivision_preserves_a_reflected_uv_chart_and_face_sense() {
    use viboceros_geometry::Brep;
    let (op, native) = adjacent_cases().pop().unwrap();
    let original = source(&native["before"]);
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let mut faces = original.faces().to_vec();
    faces[1] = reflected_u(&faces[1]);
    let surface = faces[1].surface().clone();
    let reversed = faces[1].is_reversed();
    let source = Brep::try_new(
        original.vertices().to_vec(),
        original.edges().to_vec(),
        faces,
        tolerance,
    )
    .unwrap();
    let before = source.clone();
    let output = source
        .try_inset_planar_faces(&[0], 1.5, true, tolerance)
        .unwrap();
    assert_eq!(source, before);
    assert_eq!(
        output
            .faces()
            .iter()
            .filter(|f| f.surface() == &surface && f.is_reversed() == reversed)
            .count(),
        3
    );
    let faces = output
        .faces()
        .iter()
        .map(|f| {
            if f.surface() == &surface {
                reflected_u(f)
            } else {
                f.clone()
            }
        })
        .collect();
    let normalized = Brep::try_new(
        output.vertices().to_vec(),
        output.edges().to_vec(),
        faces,
        tolerance,
    )
    .unwrap();
    native_surface::assert_geometry_with_epsilon(
        &normalized,
        &native["output"]["geometry"],
        tolerance,
        native_surface::epsilon_for(&op),
        "reflected neighbor",
    );
}

#[test]
fn adjacent_face_commands_match_native_curves_pieces_and_joined_outputs() {
    let cases = common::cases_for("inset_closed_network_faces")
        .into_iter()
        .filter(|(op, _)| op["mode"] != "sdk")
        .collect();
    cusps::replay_command_cases(cases, (3, 0));
}

#[test]
fn stored_seams_distinguish_straight_curved_and_g2_stations() {
    cusps::replay_family("inset_closed_seams", (6, 0), None);
}

#[cfg(feature = "native-smlib")]
#[test]
fn expanding_holes_with_usd_brep_match_native_source_crossings() {
    cusps::replay_family(
        "inset_fan_basis",
        (6, 6),
        Some(viboceros_smlib::curve_offset_operations()),
    );
}

#[cfg(feature = "native-smlib")]
#[test]
fn adjacent_faces_and_seams_with_usd_brep_match_native() {
    let operations = Some(viboceros_smlib::curve_offset_operations());
    cusps::replay_cases(adjacent_cases(), (2, 0), operations);
    cusps::replay_family("inset_closed_seams", (6, 0), operations);
}
