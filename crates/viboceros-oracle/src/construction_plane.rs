//! CPlane history and basis probes through the application's command parser.
use super::*;
use viboceros_command::construction_plane::{self as cplane, ConstructionPlaneState, PlaneAction};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConstructionPlaneFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub steps: Vec<PlaneStep>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaneStep {
    World {
        view: String,
    },
    Origin {
        point: [f64; 3],
    },
    ThreePoint {
        points: [[f64; 3]; 3],
    },
    ThreePointVertical {
        points: [[f64; 3]; 2],
    },
    ThreePointZAxis {
        points: [[f64; 3]; 2],
    },
    ThreePointInput {
        points: [String; 3],
    },
    OriginInput {
        point: String,
    },
    Elevation {
        distance: f64,
    },
    Through {
        point: [f64; 3],
    },
    Rotate {
        axis: [[f64; 3]; 2],
        angle: f64,
    },
    RotatePoints {
        axis: [[f64; 3]; 2],
        references: [[f64; 3]; 2],
    },
    ObjectCircle {
        center: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radius: f64,
    },
    ObjectArc {
        center: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radius: f64,
        sweep_radians: f64,
    },
    ObjectEllipse {
        center: [f64; 3],
        x_axis: [f64; 3],
        y_axis: [f64; 3],
        radius_x: f64,
        radius_y: f64,
    },
    ObjectSurface {
        corners: [[f64; 3]; 4],
    },
    ObjectMeshFace {
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
        face: usize,
    },
    Undo,
    Redo,
}

pub(super) fn run(
    f: &ConstructionPlaneFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.steps.is_empty() || f.steps.len() > 128 {
        return Err(ProbeError::FixtureInvariant(
            "expected 1 to 128 CPlane steps",
        ));
    }
    let frame = Frame3::try_from_directions(
        Point3::try_from(f.origin)?,
        Vector3::try_from(f.x_axis)?,
        Vector3::try_from(f.y_axis)?,
        tolerance,
    )?;
    let mut state = ConstructionPlaneState::new(frame);
    let record = |state: &ConstructionPlaneState| json!({"origin":state.frame().origin().to_array(), "axes":state.frame().axes().map(|a| a.as_vector().to_array())});
    let mut records = vec![record(&state)];
    for step in &f.steps {
        apply_step(&mut state, step, None, tolerance)?;
        records.push(record(&state));
    }
    Ok((json!({"states":records}), 0))
}

fn apply_step(
    state: &mut ConstructionPlaneState,
    step: &PlaneStep,
    previous: Option<Point3>,
    tolerance: Tolerance,
) -> Result<(), ProbeError> {
    if let PlaneStep::ObjectCircle {
        center,
        x_axis,
        y_axis,
        radius,
    } = step
    {
        let frame = Frame3::try_from_directions(
            Point3::try_from(*center)?,
            Vector3::try_from(*x_axis)?,
            Vector3::try_from(*y_axis)?,
            tolerance,
        )?;
        let circle = Circle3::try_from_frame(
            frame.origin(),
            *radius,
            frame.x_axis(),
            frame.z_axis(),
            tolerance,
        )?;
        let frame = cplane::frame_from_object(&Geometry::Circle(circle), tolerance)
            .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane object fixture"))?;
        state.set(frame);
        return Ok(());
    }
    if let PlaneStep::ObjectArc {
        center,
        x_axis,
        y_axis,
        radius,
        sweep_radians,
    } = step
    {
        let frame = Frame3::try_from_directions(
            Point3::try_from(*center)?,
            Vector3::try_from(*x_axis)?,
            Vector3::try_from(*y_axis)?,
            tolerance,
        )?;
        let circle = Circle3::try_from_frame(
            frame.origin(),
            *radius,
            frame.x_axis(),
            frame.z_axis(),
            tolerance,
        )?;
        let arc = CircularArc3::try_from_circle_sweep(circle, *sweep_radians)?;
        let frame = cplane::frame_from_object(&Geometry::Arc(arc), tolerance)
            .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane object fixture"))?;
        state.set(frame);
        return Ok(());
    }
    if let PlaneStep::ObjectEllipse {
        center,
        x_axis,
        y_axis,
        radius_x,
        radius_y,
    } = step
    {
        let frame = Frame3::try_from_directions(
            Point3::try_from(*center)?,
            Vector3::try_from(*x_axis)?,
            Vector3::try_from(*y_axis)?,
            tolerance,
        )?;
        let ellipse = Ellipse3::try_new(
            frame.origin(),
            *radius_x,
            *radius_y,
            frame.x_axis(),
            frame.y_axis(),
            tolerance,
        )?;
        let frame = cplane::frame_from_object(&Geometry::Ellipse(ellipse), tolerance)
            .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane object fixture"))?;
        state.set(frame);
        return Ok(());
    }
    if let PlaneStep::ObjectSurface { corners } = step {
        let corners = corners
            .map(Point3::try_from)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let surface = NurbsSurface::try_bilinear([corners[0], corners[1], corners[2], corners[3]])?;
        let frame = cplane::frame_from_object(&Geometry::NurbsSurface(surface), tolerance)
            .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane object fixture"))?;
        state.set(frame);
        return Ok(());
    }
    if let PlaneStep::ObjectMeshFace {
        vertices,
        faces,
        face,
    } = step
    {
        let vertices = vertices
            .iter()
            .copied()
            .map(Point3::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let faces = faces
            .iter()
            .map(|indices| match indices.as_slice() {
                &[a, b, c] => Ok(MeshFace::Triangle([a, b, c])),
                &[a, b, c, d] => Ok(MeshFace::Quad([a, b, c, d])),
                _ => Err(ProbeError::FixtureInvariant(
                    "mesh face needs three or four vertices",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mesh = TriangleMesh::try_new_faces(vertices, faces, tolerance)?;
        let frame = cplane::frame_from_mesh_face(&mesh, *face, tolerance)
            .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane mesh face fixture"))?;
        state.set(frame);
        return Ok(());
    }
    let point = |p: &[f64; 3]| format!("w{},{},{}", p[0], p[1], p[2]);
    let command = match step {
        PlaneStep::World { view } => format!("CPlane World {view}"),
        PlaneStep::Origin { point: p } => format!("CPlane {}", point(p)),
        PlaneStep::ThreePoint { points: [a, b, c] } => {
            format!("CPlane 3Point {} {} {}", point(a), point(b), point(c))
        }
        PlaneStep::ThreePointVertical { points: [a, b] } => {
            format!("CPlane 3Point {} Vertical {}", point(a), point(b))
        }
        PlaneStep::ThreePointZAxis { points: [a, b] } => {
            format!("CPlane 3Point {} ZAxis {}", point(a), point(b))
        }
        PlaneStep::ThreePointInput { points: [a, b, c] } => format!("CPlane 3Point {a} {b} {c}"),
        PlaneStep::OriginInput { point } => format!("CPlane {point}"),
        PlaneStep::Elevation { distance } => format!("CPlane Elevation {distance}"),
        PlaneStep::Through { point: p } => format!("CPlane Through {}", point(p)),
        PlaneStep::Rotate {
            axis: [a, b],
            angle,
        } => format!("CPlane Rotate {} {} {angle}", point(a), point(b)),
        PlaneStep::RotatePoints {
            axis: [a, b],
            references: [c, d],
        } => format!(
            "CPlane Rotate {} {} {} {}",
            point(a),
            point(b),
            point(c),
            point(d)
        ),
        PlaneStep::ObjectCircle { .. }
        | PlaneStep::ObjectArc { .. }
        | PlaneStep::ObjectEllipse { .. }
        | PlaneStep::ObjectSurface { .. }
        | PlaneStep::ObjectMeshFace { .. } => unreachable!(),
        PlaneStep::Undo => "CPlane Undo".into(),
        PlaneStep::Redo => "CPlane Redo".into(),
    };
    let action = cplane::parse(&command, state.frame(), previous, tolerance)
        .unwrap()
        .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane command fixture"))?;
    match action {
        PlaneAction::Set(frame) => {
            state.set(frame);
        }
        PlaneAction::Undo => {
            state.undo();
        }
        PlaneAction::Redo => {
            state.redo();
        }
        PlaneAction::Prompt(_) => {
            return Err(ProbeError::FixtureInvariant("incomplete CPlane fixture"));
        }
        PlaneAction::SetAllOrigin(_)
        | PlaneAction::SetThroughAll(_)
        | PlaneAction::AlignToView
        | PlaneAction::Object(_)
        | PlaneAction::ObjectFace(_, _) => {
            return Err(ProbeError::FixtureInvariant(
                "CPlane fixture requires a viewport-specific action",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConstructionPlaneInputFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub before: Vec<String>,
    pub step: PlaneStep,
    pub after: Vec<String>,
}

pub(super) fn run_input(
    f: &ConstructionPlaneInputFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.before.is_empty() || f.after.is_empty() || f.before.len() + f.after.len() > 256 {
        return Err(ProbeError::FixtureInvariant(
            "expected before/after points for nested CPlane",
        ));
    }
    let frame = Frame3::try_from_directions(
        Point3::try_from(f.origin)?,
        Vector3::try_from(f.x_axis)?,
        Vector3::try_from(f.y_axis)?,
        tolerance,
    )?;
    let mut state = ConstructionPlaneState::new(frame);
    let mut points = Vec::new();
    let append =
        |tokens: &[String], plane: Frame3, points: &mut Vec<Point3>| -> Result<(), ProbeError> {
            for token in tokens {
                let point = viboceros_drafting::PointInput::parse(token)
                    .ok_or(ProbeError::FixtureInvariant("expected point token"))?
                    .and_then(|p| p.resolve(plane, points.last().copied()))
                    .map_err(|_| ProbeError::FixtureInvariant("invalid nested point"))?;
                points.push(point);
            }
            Ok(())
        };
    append(&f.before, frame, &mut points)?;
    apply_step(&mut state, &f.step, points.last().copied(), tolerance)?;
    append(&f.after, state.frame(), &mut points)?;
    let polyline = Polyline3::try_new(points, tolerance)?;
    Ok((
        json!({"points":polyline.vertices().iter().map(|p| p.to_array()).collect::<Vec<_>>()}),
        0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_point_options_match_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_three_point_options.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_three_point_options.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 5);
    }

    #[test]
    fn picked_rotation_references_match_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_rotate_points.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_rotate_points.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 4);
    }

    #[test]
    fn object_circles_match_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_object_circle.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_object_circle.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 3);
    }

    #[test]
    fn object_arcs_match_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_object_arc.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_object_arc.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 2);
    }

    #[test]
    fn object_ellipses_match_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_object_ellipse.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_object_ellipse.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 2);
    }

    #[test]
    fn object_surfaces_match_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_object_surface.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_object_surface.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 3);
    }

    #[test]
    fn object_mesh_faces_match_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_object_mesh_face.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_object_mesh_face.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 4);
    }

    fn assert_saved_plane_frames(request: ProbeRequest, recorded: Value, count: usize) {
        let actual = run_request(&request).unwrap();
        let expected = recorded["results"].as_array().unwrap();
        assert_eq!(actual.results.len(), count);
        assert_eq!(expected.len(), count);
        for (result, observation) in actual.results.iter().zip(expected) {
            assert_eq!(result.id, observation["id"].as_str().unwrap());
            let states = result.value["states"].as_array().unwrap();
            let references = observation["value"]["states"].as_array().unwrap();
            assert_eq!(states.len(), references.len());
            for (state, reference) in states.iter().zip(references) {
                let actual = state["origin"].as_array().unwrap().iter().chain(
                    state["axes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .flat_map(|axis| axis.as_array().unwrap()),
                );
                let expected = reference["origin"].as_array().unwrap().iter().chain(
                    reference["axes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .flat_map(|axis| axis.as_array().unwrap()),
                );
                for (actual, expected) in actual.zip(expected) {
                    assert!((actual.as_f64().unwrap() - expected.as_f64().unwrap()).abs() <= 1e-10);
                }
            }
        }
    }

    #[test]
    fn nested_cplane_fixtures_preserve_prior_polyline_points() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_input.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 24);
        for result in response.results {
            assert_eq!(result.elapsed_ns, 0);
            assert_eq!(result.value["points"].as_array().unwrap().len(), 3);
            assert_eq!(result.value["points"][0], json!([1., 2., 3.]));
            assert_eq!(result.value["points"][1], json!([4., 5., 6.]));
        }
    }
    #[test]
    fn permanent_construction_plane_fixtures_remain_orthonormal_and_right_handed() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_planes.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        for result in response.results {
            assert_eq!(result.elapsed_ns, 0);
            for frame in result.value["states"].as_array().unwrap() {
                let axes: [[f64; 3]; 3] = serde_json::from_value(frame["axes"].clone()).unwrap();
                let axes = axes.map(|a| Vector3::try_from(a).unwrap());
                for axis in axes {
                    assert!((axis.length().unwrap() - 1.0).abs() < 1e-12);
                }
                assert!(axes[0].dot(axes[1]).unwrap().abs() < 1e-12);
                assert!(
                    (axes[0].cross(axes[1]).unwrap().dot(axes[2]).unwrap() - 1.0).abs() < 1e-12
                );
            }
        }
    }
}
