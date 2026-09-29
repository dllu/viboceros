//! CPlane history and basis probes through the application's command parser.
use super::*;
use viboceros_command::construction_plane::{self as cplane, ConstructionPlaneState, PlaneAction};
use viboceros_geometry::{Brep, CurveSegment3, PolyCurve3};

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
    ObjectLine {
        start: [f64; 3],
        end: [f64; 3],
    },
    ObjectPolyline {
        vertices: Vec<[f64; 3]>,
    },
    ObjectNurbs {
        definition: NurbsCurveDefinition,
    },
    ObjectPolycurve {
        segments: Vec<PlanePolycurveSegment>,
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
    ObjectBrepFace {
        r#box: [[f64; 3]; 2],
        face: usize,
    },
    SurfaceCplane {
        corners: [[f64; 3]; 4],
        pick_origin: Option<[f64; 3]>,
        pick_x: Option<[f64; 3]>,
        flip: Option<bool>,
    },
    SurfaceCplaneTrimmed {
        outer: Vec<[f64; 3]>,
        holes: Vec<Vec<[f64; 3]>>,
        pick_origin: Option<[f64; 3]>,
        pick_x: Option<[f64; 3]>,
        flip: Option<bool>,
        ignore_trims: Option<bool>,
    },
    Undo,
    Redo,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlanePolycurveSegment {
    Line {
        start: [f64; 3],
        end: [f64; 3],
    },
    Arc {
        points: [[f64; 3]; 3],
    },
    Nurbs {
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
        #[serde(default)]
        domain: Option<[f64; 2]>,
    },
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
    if let PlaneStep::ObjectLine { start, end } = step {
        let line = LineSegment::try_new(
            Point3::try_from(*start)?,
            Point3::try_from(*end)?,
            tolerance,
        )?;
        let frame = cplane::frame_from_object(&Geometry::Line(line), tolerance)
            .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane line fixture"))?;
        state.set(frame);
        return Ok(());
    }
    if let PlaneStep::ObjectPolyline { vertices } = step {
        let vertices = vertices
            .iter()
            .copied()
            .map(Point3::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let polyline = Polyline3::try_new(vertices, tolerance)?;
        let frame = cplane::frame_from_object(&Geometry::Polyline(polyline), tolerance)
            .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane polyline fixture"))?;
        state.set(frame);
        return Ok(());
    }
    if let PlaneStep::ObjectNurbs { definition } = step {
        let curve = nurbs_curve_from_definition(definition)?;
        let frame = cplane::frame_from_object(&Geometry::NurbsCurve(curve), tolerance)
            .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane NURBS fixture"))?;
        state.set(frame);
        return Ok(());
    }
    if let PlaneStep::ObjectPolycurve { segments } = step {
        let segments = segments
            .iter()
            .map(|segment| -> Result<CurveSegment3, ProbeError> {
                Ok(match segment {
                    PlanePolycurveSegment::Line { start, end } => {
                        CurveSegment3::Line(LineSegment::try_new(
                            Point3::try_from(*start)?,
                            Point3::try_from(*end)?,
                            tolerance,
                        )?)
                    }
                    PlanePolycurveSegment::Arc { points } => {
                        CurveSegment3::Arc(CircularArc3::try_from_three_points(
                            Point3::try_from(points[0])?,
                            Point3::try_from(points[1])?,
                            Point3::try_from(points[2])?,
                            tolerance,
                        )?)
                    }
                    PlanePolycurveSegment::Nurbs {
                        degree,
                        control_points,
                        knots,
                        domain,
                    } => CurveSegment3::NurbsCurve(nurbs_curve_from_definition(
                        &NurbsCurveDefinition {
                            degree: *degree,
                            control_points: control_points.clone(),
                            knots: knots.clone(),
                            domain: *domain,
                        },
                    )?),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let polycurve = PolyCurve3::try_new(segments)?;
        let frame = cplane::frame_from_object(&Geometry::PolyCurve(polycurve), tolerance)
            .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane polycurve fixture"))?;
        state.set(frame);
        return Ok(());
    }
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
    if let PlaneStep::ObjectBrepFace { r#box, face } = step {
        // Rhino's CreateFromBox and our box builder enumerate the same physical
        // faces in different orders. Keep this mapping in the fixture adapter.
        let native_face = *[2, 5, 3, 4, 0, 1]
            .get(*face)
            .ok_or(ProbeError::FixtureInvariant("invalid Rhino box face index"))?;
        let bounds = [
            [r#box[0][0], r#box[1][0]],
            [r#box[0][1], r#box[1][1]],
            [r#box[0][2], r#box[1][2]],
        ];
        let brep = Brep::try_box(cplane::WorldPlane::Top.frame(), bounds, tolerance)?;
        let frame = cplane::frame_from_brep_face(&brep, native_face, tolerance)
            .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane B-rep face fixture"))?;
        state.set(frame);
        return Ok(());
    }
    if let PlaneStep::SurfaceCplane {
        corners,
        pick_origin,
        pick_x,
        flip,
    } = step
    {
        let corners = corners
            .map(Point3::try_from)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let surface = NurbsSurface::try_bilinear([corners[0], corners[1], corners[2], corners[3]])?;
        let frame = cplane::surface_frame_with_flip(
            &surface,
            false,
            pick_origin.map(Point3::try_from).transpose()?,
            pick_x.map(Point3::try_from).transpose()?,
            flip.unwrap_or(false),
            tolerance,
        )
        .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane Surface fixture"))?;
        state.set(frame);
        return Ok(());
    }
    if let PlaneStep::SurfaceCplaneTrimmed {
        outer,
        holes,
        pick_origin,
        pick_x,
        flip,
        ignore_trims,
    } = step
    {
        let curve = |vertices: &Vec<[f64; 3]>| -> Result<_, ProbeError> {
            let mut points = vertices
                .iter()
                .copied()
                .map(Point3::try_from)
                .collect::<Result<Vec<_>, _>>()?;
            let first = *points
                .first()
                .ok_or(ProbeError::FixtureInvariant("empty CPlane trim"))?;
            points.push(first);
            Ok(Polyline3::try_new(points, tolerance)?.to_nurbs()?)
        };
        let outer = curve(outer)?;
        let holes = holes.iter().map(curve).collect::<Result<Vec<_>, _>>()?;
        let brep = Brep::try_planar_face_with_holes(&outer, &holes, tolerance)?;
        let frame = cplane::surface_frame_on_brep_face(
            &brep,
            0,
            pick_origin.map(Point3::try_from).transpose()?,
            pick_x.map(Point3::try_from).transpose()?,
            flip.unwrap_or(false),
            ignore_trims.unwrap_or(false),
            tolerance,
        )
        .map_err(|_| ProbeError::FixtureInvariant("invalid trimmed CPlane Surface fixture"))?;
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
        PlaneStep::ObjectLine { .. }
        | PlaneStep::ObjectPolyline { .. }
        | PlaneStep::ObjectNurbs { .. }
        | PlaneStep::ObjectPolycurve { .. }
        | PlaneStep::ObjectCircle { .. }
        | PlaneStep::ObjectArc { .. }
        | PlaneStep::ObjectEllipse { .. }
        | PlaneStep::ObjectSurface { .. }
        | PlaneStep::ObjectMeshFace { .. }
        | PlaneStep::ObjectBrepFace { .. } => unreachable!(),
        PlaneStep::SurfaceCplane { .. } | PlaneStep::SurfaceCplaneTrimmed { .. } => unreachable!(),
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
        | PlaneAction::ObjectFace(_, _)
        | PlaneAction::Surface { .. } => {
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

    #[test]
    fn object_brep_faces_match_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_object_brep_face.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_object_brep_face.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 6);
    }

    #[test]
    fn surface_option_matches_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_surface.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_surface.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 6);
    }

    #[test]
    fn surface_flip_matches_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_surface_options.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_surface_options.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 10);
    }

    #[test]
    fn surface_trims_match_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_surface_trimmed.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_surface_trimmed.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 6);
    }

    #[test]
    fn object_lines_polylines_and_nurbs_match_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_object_curve.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_object_curve.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 15);
    }

    #[test]
    fn object_nonplanar_curves_match_saved_rhino_start_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_object_nonplanar.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_object_nonplanar.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 2);
    }

    #[test]
    fn object_polycurves_match_saved_rhino_plane_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_object_polycurve.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_object_polycurve.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 6);
    }

    #[test]
    fn object_polycurve_single_and_linear_segments_match_saved_rhino_frames() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_object_polycurve_edges.json"
        ))
        .unwrap();
        let recorded: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/construction_plane_object_polycurve_edges.json"
        ))
        .unwrap();
        assert_saved_plane_frames(request, recorded, 3);
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
