//! Picked and typed drafting points dispatched to the active command.
use super::*;

impl VibocerosApp {
    pub(super) fn apply_drafting_point(&mut self, point: Point3) -> bool {
        self.apply_drafting_point_with_snap(point, None)
    }
    pub(super) fn apply_drafting_point_with_snap(
        &mut self,
        point: Point3,
        snap: Option<viboceros_drafting::ObjectSnap>,
    ) -> bool {
        if self
            .select_uv_prompt
            .as_ref()
            .is_some_and(|p| !p.selecting())
        {
            return self.select_uv_point(point);
        }
        if let Some(accepted) = self.accept_refinement_drafting_point(point) {
            return accepted;
        }
        if !self.prepare_arc_deform_point() {
            return false;
        }
        if let Some(accepted) = self.accept_arc_extension_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_arc_length_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_arc_tilted_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_circle_deform_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_tangent_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_circle_around_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_subd_ellipsoid_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_ellipse_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_catenary_point(point, snap.map(|s| s.kind())) {
            return accepted;
        }
        if let Some(accepted) = self.accept_spiral_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_hyperbola_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_parabola_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_conic_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_subd_torus_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_subd_sphere_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_subd_axial_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_subd_primitive_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_rectangle_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_surface_flow_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_flow_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_orient_surface_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_orient_points_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_curve_array_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_surface_array_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_array_curve_surface_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_face_create_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_mesh_polyline_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_surface_interpolation_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_short_path_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_surface_offset_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_orient_edge_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_orient_curve_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_contour_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_section_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_block_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_standalone_subcurve_point(point) {
            return accepted;
        }
        if let Some(accepted) = self.accept_uv_subcurve_point(point) {
            return accepted;
        }
        let Some(command) = self.active_command else {
            return false;
        };
        let plane = self
            .drafting_plane
            .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane());
        match command {
            InteractiveCommand::ProjectPlane { .. } => {
                return self.accept_project_plane_point(point);
            }
            InteractiveCommand::ProjectDirection { start } => {
                return self.accept_project_direction_point(start, point);
            }
            InteractiveCommand::RemapCPlane
            | InteractiveCommand::OrientPoints { .. }
            | InteractiveCommand::OrientCurveToEdge
            | InteractiveCommand::OffsetCurveOnSurface
            | InteractiveCommand::ShortPath { .. }
            | InteractiveCommand::FaceCreate { .. }
            | InteractiveCommand::PolylineOnMesh { .. }
            | InteractiveCommand::InterpolateCurveOnSurface { .. }
            | InteractiveCommand::OrientOnCurve
            | InteractiveCommand::ArrayCurveOnSurface
            | InteractiveCommand::SurfaceArray { .. }
            | InteractiveCommand::CurveArray { .. }
            | InteractiveCommand::CircleDeform { .. }
            | InteractiveCommand::ArcTangent { .. }
            | InteractiveCommand::ArcExtension { .. }
            | InteractiveCommand::ArcTilted { .. }
            | InteractiveCommand::ArcDeform { .. }
            | InteractiveCommand::CircleTangent { .. }
            | InteractiveCommand::CircleAround { .. }
            | InteractiveCommand::Ellipse { .. }
            | InteractiveCommand::Catenary { .. }
            | InteractiveCommand::Spiral { .. }
            | InteractiveCommand::Hyperbola { .. }
            | InteractiveCommand::Parabola { .. }
            | InteractiveCommand::Conic { .. }
            | InteractiveCommand::SubDTorus { .. }
            | InteractiveCommand::SubDEllipsoid { .. }
            | InteractiveCommand::SubDSphere { .. }
            | InteractiveCommand::SubDTruncatedCone { .. }
            | InteractiveCommand::SubDCone { .. }
            | InteractiveCommand::SubDCylinder { .. }
            | InteractiveCommand::SubDPrimitive { .. }
            | InteractiveCommand::Rectangle { .. }
            | InteractiveCommand::SurfaceFlow { .. }
            | InteractiveCommand::Flow { .. }
            | InteractiveCommand::OrientOnSurface { .. }
            | InteractiveCommand::Splop { .. }
            | InteractiveCommand::Section { .. }
            | InteractiveCommand::Contour { .. } => {
                return false;
            }
            InteractiveCommand::Block { .. }
            | InteractiveCommand::BlockEditAdd
            | InteractiveCommand::BlockEditBasePoint
            | InteractiveCommand::AddObjectsToBlock
            | InteractiveCommand::Insert { .. }
            | InteractiveCommand::CreateUniqueBlock
            | InteractiveCommand::ReplaceBlock => return false,
            InteractiveCommand::ScaleByPlane(prompt) => {
                return self.accept_scale_by_plane_point(prompt, point);
            }
            InteractiveCommand::ScaleNu(prompt) => {
                return self.accept_scale_nu_point(prompt, point);
            }
            InteractiveCommand::ScalePositions(prompt) => {
                return self.accept_scale_positions_point(prompt, point);
            }
            InteractiveCommand::Angle { mut points } => {
                let index = points.iter().position(Option::is_none).unwrap_or(3);
                if index == 1 || index == 3 {
                    let start = points[index - 1].unwrap();
                    if let Err(error) = start.direction_to(point) {
                        self.push_log(format!("Error: {error}"));
                        return false;
                    }
                }
                if index < 3 {
                    points[index] = Some(point);
                    let next = InteractiveCommand::Angle { points };
                    self.active_command = Some(next);
                    self.push_log(next.prompt().to_owned());
                } else {
                    let [a, b, c] = points.map(Option::unwrap);
                    self.active_command = None;
                    self.execute_command(&format!(
                        "Angle {} {} {} {}",
                        format_model_point(a),
                        format_model_point(b),
                        format_model_point(c),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::Points => return self.apply_points_point(point),
            InteractiveCommand::EvaluatePoint => return self.finish_evaluate_point(point, plane),
            InteractiveCommand::EvaluateUv { options } => {
                return self.apply_evaluate_uv(point, options);
            }
            InteractiveCommand::DomainFace => return self.finish_domain_face(point),
            InteractiveCommand::DomainSubCrv { start } => {
                return self.accept_subcurve_measurement_point("Domain", start, point, None);
            }
            InteractiveCommand::LengthSubCrv {
                start,
                display_units,
            } => {
                return self.accept_subcurve_measurement_point(
                    "Length",
                    start,
                    point,
                    display_units,
                );
            }
            InteractiveCommand::Align { options, .. } => {
                return self.finish_align(Some(point), options);
            }
            InteractiveCommand::Point => {
                self.active_command = None;
                self.execute_command(&format!("Point {}", format_model_point(point)));
            }
            InteractiveCommand::Line { start: None } => {
                self.active_command = Some(InteractiveCommand::Line { start: Some(point) });
                self.push_log(format!("Start: {}", format_model_point(point)));
                self.push_log(
                    InteractiveCommand::Line { start: Some(point) }
                        .prompt()
                        .to_owned(),
                );
            }
            InteractiveCommand::Distance {
                start: None,
                previous_last,
                display_units,
            } => {
                let next = InteractiveCommand::Distance {
                    start: Some(point),
                    previous_last,
                    display_units,
                };
                self.active_command = Some(next);
                self.push_log(next.prompt().to_owned());
            }
            InteractiveCommand::Distance {
                start: Some(start),
                display_units,
                ..
            } => {
                return self.finish_distance(start, point, display_units, plane);
            }
            InteractiveCommand::Line { start: Some(start) } => {
                if !start
                    .distance_to(point)
                    .is_ok_and(|distance| distance > self.document.tolerance().absolute())
                {
                    self.push_log("Error: line end must have a finite distance greater than tolerance from its start".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Line {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Circle { center: None } => {
                let command = InteractiveCommand::Circle {
                    center: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Circle {
                center: Some(center),
            } => {
                if !center
                    .distance_to(point)
                    .is_ok_and(|radius| radius > self.document.tolerance().absolute())
                {
                    self.push_log("Error: circle point must differ from its center".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Circle {} {}",
                    format_model_point(center),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::CircleSize { center, mode } => {
                let argument = if matches!(mode, CircleSizeMode::Radius | CircleSizeMode::Diameter)
                {
                    format_model_point(point)
                } else {
                    let Ok(distance) = center.distance_to(point) else {
                        self.push_log("Error: circle size is not representable".into());
                        return false;
                    };
                    format!("{}={distance}", mode.label())
                };
                return self.finish_circle_size(
                    InteractiveCommand::CircleSize { center, mode },
                    &argument,
                );
            }
            InteractiveCommand::CircleVertical {
                center: None,
                radius,
                mode,
            } => {
                let next = InteractiveCommand::CircleVertical {
                    center: Some(point),
                    radius,
                    mode,
                };
                self.active_command = Some(next);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(next.prompt().to_owned());
            }
            InteractiveCommand::CircleVertical {
                center: Some(center),
                radius,
                mode,
            } => {
                let state = InteractiveCommand::CircleVertical {
                    center: Some(center),
                    radius,
                    mode,
                };
                return self.finish_circle_vertical(state, point);
            }
            InteractiveCommand::CircleOrientation {
                center,
                normal_point: None,
                mode,
            } => {
                let direction = match center.vector_to(point) {
                    Ok(direction) => direction,
                    Err(error) => {
                        self.push_log(format!("Error: {error}"));
                        return false;
                    }
                };
                if let Err(error) =
                    Frame3::try_from_normal(center, direction, self.document.tolerance())
                {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
                let next = InteractiveCommand::CircleOrientation {
                    center,
                    normal_point: Some(point),
                    mode,
                };
                self.active_command = Some(next);
                self.push_log(next.prompt().to_owned());
            }
            InteractiveCommand::CircleOrientation {
                center,
                normal_point: Some(normal_point),
                mode,
            } => {
                return self.finish_circle_orientation(
                    InteractiveCommand::CircleOrientation {
                        center,
                        normal_point: Some(normal_point),
                        mode,
                    },
                    point,
                );
            }
            InteractiveCommand::CircleTwoPoint { first: None } => {
                let next = InteractiveCommand::CircleTwoPoint { first: Some(point) };
                self.active_command = Some(next);
                self.push_log(next.prompt().to_owned());
            }
            InteractiveCommand::CircleTwoPoint { first: Some(first) } => {
                if let Err(error) = viboceros_geometry::Circle3::try_from_diameter_on_plane(
                    first,
                    point,
                    plane,
                    self.document.tolerance(),
                ) {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Circle 2Point {} {}",
                    format_model_point(first),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::CircleThreePoint { mut points } => {
                let index = points.iter().position(Option::is_none).unwrap_or(2);
                if index == 1
                    && !points[0]
                        .unwrap()
                        .distance_to(point)
                        .is_ok_and(|distance| distance > self.document.tolerance().absolute())
                {
                    self.push_log("Error: circle points must differ".into());
                    return false;
                }
                if index < 2 {
                    points[index] = Some(point);
                    let next = InteractiveCommand::CircleThreePoint { points };
                    self.active_command = Some(next);
                    self.push_log(next.prompt().to_owned());
                } else {
                    let [first, second] = points.map(Option::unwrap);
                    if let Err(error) = viboceros_geometry::Circle3::try_from_three_points(
                        first,
                        second,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(format!("Error: {error}"));
                        return false;
                    }
                    self.active_command = None;
                    self.execute_command(&format!(
                        "Circle 3Point {} {} {}",
                        format_model_point(first),
                        format_model_point(second),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::CircleThreePointRadius {
                first,
                second,
                radius,
            } => {
                return self.finish_three_point_radius(
                    InteractiveCommand::CircleThreePointRadius {
                        first,
                        second,
                        radius,
                    },
                    point,
                );
            }
            InteractiveCommand::Sphere { center: None } => {
                let command = InteractiveCommand::Sphere {
                    center: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Sphere {
                center: Some(center),
            } => {
                if !center
                    .distance_to(point)
                    .is_ok_and(|radius| radius > self.document.tolerance().absolute())
                {
                    self.push_log(
                        "Error: sphere radius must be finite and greater than tolerance".to_owned(),
                    );
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Sphere {} {}",
                    format_model_point(center),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::SelVolumeSphere { center: None, mode } => {
                let command = InteractiveCommand::SelVolumeSphere {
                    center: Some(point),
                    mode,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::SelVolumeSphere {
                center: Some(center),
                mode,
            } => {
                let Ok(radius) = center.distance_to(point) else {
                    self.push_log("Error: sphere radius is not finite".to_owned());
                    return false;
                };
                if radius <= 0.0 {
                    self.push_log("Error: sphere radius must be positive".to_owned());
                    return false;
                }
                let mode_name = match mode {
                    RectSelectionMode::Automatic | RectSelectionMode::Crossing => "Crossing",
                    RectSelectionMode::Window => "Window",
                    RectSelectionMode::InvertWindow => "InvertWindow",
                    RectSelectionMode::InvertCrossing => "InvertCrossing",
                };
                self.active_command = None;
                self.execute_command(&format!(
                    "SelVolumeSphere {} {radius} SelectionMode={mode_name}",
                    format_model_point(center),
                ));
            }
            InteractiveCommand::SelVolumePipe { source: None, .. } => {
                self.push_log("Select a centerline curve first".to_owned());
                return false;
            }
            InteractiveCommand::SelVolumePipe {
                source: Some(source),
                mode,
            } => {
                let radius = self
                    .document
                    .object(source)
                    .and_then(|object| object.geometry().curve_ref())
                    .and_then(|curve| {
                        let parameter = curve
                            .closest_parameter(point, self.document.tolerance())
                            .ok()?;
                        let nearest = curve.evaluate(parameter).ok()?;
                        point.distance_to(nearest).ok()
                    });
                let Some(radius) = radius else {
                    self.push_log("Error: could not measure pipe radius".to_owned());
                    return false;
                };
                if radius <= 0.0 {
                    self.push_log("Error: pipe radius must be positive".to_owned());
                    return false;
                }
                let mode_name = match mode {
                    RectSelectionMode::Automatic | RectSelectionMode::Crossing => "Crossing",
                    RectSelectionMode::Window => "Window",
                    RectSelectionMode::InvertWindow => "InvertWindow",
                    RectSelectionMode::InvertCrossing => "InvertCrossing",
                };
                self.active_command = None;
                self.execute_command(&format!(
                    "SelVolumePipe {source} {radius} SelectionMode={mode_name}"
                ));
            }
            InteractiveCommand::SelVolumeObject { .. } => {
                self.push_log("Select a closed mesh or polysurface first".to_owned());
                return false;
            }
            InteractiveCommand::Pipe { source: None, .. } => {
                self.push_log("Select a rail curve first".to_owned());
                return false;
            }
            InteractiveCommand::Pipe {
                source: Some(source),
                cap_flat,
                cap_round,
                blend_global,
                fit_rail,
                wall_thickness,
                pick_second_radius,
                first_radius,
            } => {
                let radius = self
                    .document
                    .object(source)
                    .and_then(|object| object.geometry().curve_ref())
                    .and_then(|curve| {
                        let parameter = curve
                            .closest_parameter(point, self.document.tolerance())
                            .ok()?;
                        let nearest = curve.evaluate(parameter).ok()?;
                        point.distance_to(nearest).ok()
                    });
                let Some(radius) = radius.filter(|radius| *radius > 0.0) else {
                    self.push_log("Error: pipe radius must be positive".to_owned());
                    return false;
                };
                if pick_second_radius && first_radius.is_none() {
                    let command = InteractiveCommand::Pipe {
                        source: Some(source),
                        cap_flat,
                        cap_round,
                        blend_global,
                        fit_rail,
                        wall_thickness,
                        pick_second_radius,
                        first_radius: Some(radius),
                    };
                    self.active_command = Some(command);
                    self.push_log(command.prompt().to_owned());
                    return true;
                }
                let start_radius = first_radius.unwrap_or(radius);
                let wall_thickness = if pick_second_radius {
                    Some(radius - start_radius)
                } else {
                    wall_thickness
                };
                if wall_thickness
                    .is_some_and(|thickness| thickness == 0.0 || start_radius + thickness <= 0.0)
                {
                    self.push_log(
                        "Error: second pipe radius must be positive and distinct".to_owned(),
                    );
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Pipe {source} {start_radius} Cap={} ShapeBlending={} FitRail={}{}",
                    if cap_round {
                        "Round"
                    } else if cap_flat {
                        "Flat"
                    } else {
                        "None"
                    },
                    if blend_global { "Global" } else { "Local" },
                    if fit_rail { "Yes" } else { "No" },
                    wall_thickness.map_or_else(String::new, |thickness| {
                        format!(" WallThickness={thickness}")
                    })
                ));
            }
            InteractiveCommand::Ellipsoid { mut points } => {
                let point_count = points.iter().flatten().count();
                if point_count == 1 {
                    let Some(center) = points[0] else {
                        self.push_log("Error: ellipsoid point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    if center.is_near(point, self.document.tolerance()) {
                        self.push_log(
                            "Error: ellipsoid axis point must differ from its center".to_owned(),
                        );
                        return false;
                    }
                } else if point_count == 2 {
                    let [Some(center), Some(first_axis), _] = points else {
                        self.push_log("Error: ellipsoid point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    if let Err(error) = Frame3::try_from_points(
                        center,
                        first_axis,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(format!("Error: {error}"));
                        return false;
                    }
                }

                if point_count < points.len() {
                    points[point_count] = Some(point);
                    let command = InteractiveCommand::Ellipsoid { points };
                    self.active_command = Some(command);
                    let label = ["Center", "First axis", "Second axis"][point_count];
                    self.push_log(format!("{label}: {}", format_model_point(point)));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(center), Some(first_axis), Some(second_axis)] = points else {
                        self.push_log("Error: ellipsoid point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    if !ellipsoid_third_radius_exceeds_tolerance(
                        center,
                        first_axis,
                        second_axis,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(
                            "Error: ellipsoid third-axis radius must be positive".to_owned(),
                        );
                        return false;
                    }
                    self.active_command = None;
                    self.execute_command(&format!(
                        "Ellipsoid {} {} {} {}",
                        format_model_point(center),
                        format_model_point(first_axis),
                        format_model_point(second_axis),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::MeshEllipsoid {
                mut points,
                vertical_count,
                around_count,
                cap_style,
            } => {
                let point_count = points.iter().flatten().count();
                if point_count == 1 {
                    let Some(center) = points[0] else {
                        self.push_log(
                            "Error: mesh-ellipsoid point state is inconsistent".to_owned(),
                        );
                        self.active_command = None;
                        return false;
                    };
                    if center.is_near(point, self.document.tolerance()) {
                        self.push_log(
                            "Error: mesh-ellipsoid axis point must differ from its center"
                                .to_owned(),
                        );
                        return false;
                    }
                } else if point_count == 2 {
                    let [Some(center), Some(first_axis), _] = points else {
                        self.push_log(
                            "Error: mesh-ellipsoid point state is inconsistent".to_owned(),
                        );
                        self.active_command = None;
                        return false;
                    };
                    if let Err(error) = Frame3::try_from_points(
                        center,
                        first_axis,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(format!("Error: {error}"));
                        return false;
                    }
                }

                if point_count < points.len() {
                    points[point_count] = Some(point);
                    let command = InteractiveCommand::MeshEllipsoid {
                        points,
                        vertical_count,
                        around_count,
                        cap_style,
                    };
                    self.active_command = Some(command);
                    let label = ["Center", "First axis", "Second axis"][point_count];
                    self.push_log(format!("{label}: {}", format_model_point(point)));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(center), Some(first_axis), Some(second_axis)] = points else {
                        self.push_log(
                            "Error: mesh-ellipsoid point state is inconsistent".to_owned(),
                        );
                        self.active_command = None;
                        return false;
                    };
                    if !ellipsoid_third_radius_exceeds_tolerance(
                        center,
                        first_axis,
                        second_axis,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(
                            "Error: mesh-ellipsoid third-axis radius must be positive".to_owned(),
                        );
                        return false;
                    }
                    self.active_command = None;
                    let cap_style = match cap_style {
                        MeshCapFaceStyle::Triangles => "Tri",
                        MeshCapFaceStyle::Quadrilaterals => "Quad",
                    };
                    self.execute_command(&format!(
                        "MeshEllipsoid {} {} {} {} VerticalFaces={} AroundFaces={} CapFaceStyle={}",
                        format_model_point(center),
                        format_model_point(first_axis),
                        format_model_point(second_axis),
                        format_model_point(point),
                        vertical_count,
                        around_count,
                        cap_style
                    ));
                }
            }
            InteractiveCommand::ArcCenter {
                center: None,
                start: None,
                clockwise,
            } => {
                let command = InteractiveCommand::ArcCenter {
                    center: Some(point),
                    start: None,
                    clockwise,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArcCenter {
                center: Some(center),
                start: None,
                clockwise,
            } => {
                if !center
                    .distance_to(point)
                    .is_ok_and(|radius| radius > self.document.tolerance().absolute())
                {
                    self.push_log("Error: arc start must differ from center".into());
                    return false;
                }
                let command = InteractiveCommand::ArcCenter {
                    center: Some(center),
                    start: Some(point),
                    clockwise,
                };
                self.active_command = Some(command);
                self.push_log(format!("Start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArcCenter {
                center: Some(center),
                start: Some(start),
                clockwise,
            } => {
                if let Err(error) = CircularArc3::try_from_center_start_end_on_plane(
                    center,
                    start,
                    point,
                    if clockwise {
                        plane.z_axis().opposite()
                    } else {
                        plane.z_axis()
                    },
                    self.document.tolerance(),
                ) {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
                self.execute_arc_command(&format!(
                    "Arc Center {} {} End={} Direction={}",
                    format_model_point(center),
                    format_model_point(start),
                    format_model_point(point),
                    if clockwise {
                        "Clockwise"
                    } else {
                        "Counterclockwise"
                    }
                ));
            }
            InteractiveCommand::ArcCenter { .. } => {
                self.push_log("Pick an endpoint, enter an angle, or choose Length".into());
                return false;
            }
            InteractiveCommand::ArcCenterLength { .. } => {
                self.push_log("Enter a signed arc length".into());
                return false;
            }
            InteractiveCommand::ArcCenterMidpoint {
                center: None,
                midpoint: None,
                clockwise,
            } => {
                let command = InteractiveCommand::ArcCenterMidpoint {
                    center: Some(point),
                    midpoint: None,
                    clockwise,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArcCenterMidpoint {
                center: Some(center),
                midpoint: None,
                clockwise,
            } => {
                if point.is_near(center, self.document.tolerance()) {
                    self.push_log("Error: arc midpoint must differ from center".into());
                    return false;
                }
                let command = InteractiveCommand::ArcCenterMidpoint {
                    center: Some(center),
                    midpoint: Some(point),
                    clockwise,
                };
                self.active_command = Some(command);
                self.push_log(format!("Midpoint: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArcCenterMidpoint {
                center: Some(center),
                midpoint: Some(midpoint),
                clockwise,
            } => {
                if let Err(error) = CircularArc3::try_from_center_midpoint_end_on_plane(
                    center,
                    midpoint,
                    point,
                    if clockwise {
                        plane.z_axis().opposite()
                    } else {
                        plane.z_axis()
                    },
                    self.document.tolerance(),
                ) {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
                self.execute_arc_command(&format!(
                    "Arc Center {} Midpoint {} End={} Direction={}",
                    format_model_point(center),
                    format_model_point(midpoint),
                    format_model_point(point),
                    if clockwise {
                        "Clockwise"
                    } else {
                        "Counterclockwise"
                    }
                ));
            }
            InteractiveCommand::ArcCenterMidpointLength { .. } => {
                self.push_log("Enter a signed arc length".into());
                return false;
            }
            InteractiveCommand::ArcCenterMidpoint { .. } => {
                self.push_log("Error: arc point state is inconsistent".into());
                self.active_command = None;
                return false;
            }
            InteractiveCommand::ArcStartCenter {
                start: None,
                center: None,
                clockwise,
            } => {
                let command = InteractiveCommand::ArcStartCenter {
                    start: Some(point),
                    center: None,
                    clockwise,
                };
                self.active_command = Some(command);
                self.push_log(format!("Start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArcStartCenter {
                start: Some(start),
                center: None,
                clockwise,
            } => {
                if point.is_near(start, self.document.tolerance()) {
                    self.push_log("Error: arc center and start must differ".into());
                    return false;
                }
                let command = InteractiveCommand::ArcStartCenter {
                    start: Some(start),
                    center: Some(point),
                    clockwise,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArcStartCenter {
                start: Some(start),
                center: Some(center),
                clockwise,
            } => {
                if let Err(error) = CircularArc3::try_from_center_start_end_on_plane(
                    center,
                    start,
                    point,
                    if clockwise {
                        plane.z_axis().opposite()
                    } else {
                        plane.z_axis()
                    },
                    self.document.tolerance(),
                ) {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
                self.execute_arc_command(&format!(
                    "Arc StartPoint {} Center={} End={} Direction={}",
                    format_model_point(start),
                    format_model_point(center),
                    format_model_point(point),
                    if clockwise {
                        "Clockwise"
                    } else {
                        "Counterclockwise"
                    }
                ));
            }
            InteractiveCommand::ArcStartCenterLength { .. } => {
                self.push_log("Enter a signed arc length".into());
                return false;
            }
            InteractiveCommand::ArcStartCenter { .. } => {
                self.push_log("Error: arc point state is inconsistent".into());
                self.active_command = None;
                return false;
            }
            InteractiveCommand::ArcStartPoint {
                start: None,
                end: None,
            } => {
                let command = InteractiveCommand::ArcStartPoint {
                    start: Some(point),
                    end: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArcStartPoint {
                start: Some(start),
                end: None,
            } => {
                if point.is_near(start, self.document.tolerance()) {
                    self.push_log("Error: arc endpoints must differ".into());
                    return false;
                }
                let command = InteractiveCommand::ArcStartPoint {
                    start: Some(start),
                    end: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("End: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArcStartPoint {
                start: Some(start),
                end: Some(end),
            } => {
                if let Err(error) = CircularArc3::try_from_three_points(
                    start,
                    point,
                    end,
                    self.document.tolerance(),
                ) {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
                self.execute_arc_command(&format!(
                    "Arc StartPoint {} {} {}",
                    format_model_point(start),
                    format_model_point(end),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::ArcStartDirection {
                start: None,
                tangent: None,
            } => {
                let command = InteractiveCommand::ArcStartDirection {
                    start: Some(point),
                    tangent: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArcStartDirection {
                start: Some(start),
                tangent: None,
            } => {
                if point.is_near(start, self.document.tolerance()) {
                    self.push_log("Error: arc tangent direction must be nonzero".into());
                    return false;
                }
                let command = InteractiveCommand::ArcStartDirection {
                    start: Some(start),
                    tangent: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Tangent direction: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArcStartDirection {
                start: Some(start),
                tangent: Some(direction),
            } => {
                let tangent = start
                    .vector_to(direction)
                    .and_then(|vector| vector.normalized(self.document.tolerance()));
                let result = tangent.and_then(|tangent| {
                    CircularArc3::try_from_start_tangent_end(
                        start,
                        tangent,
                        point,
                        self.document.tolerance(),
                    )
                });
                if let Err(error) = result {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
                self.execute_arc_command(&format!(
                    "Arc StartPoint {} Direction={} {}",
                    format_model_point(start),
                    format_model_point(direction),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::ArcStartPoint { .. }
            | InteractiveCommand::ArcStartDirection { .. } => {
                self.push_log("Error: arc point state is inconsistent".into());
                self.active_command = None;
                return false;
            }
            InteractiveCommand::Arc { mut points } => {
                let point_count = points.iter().flatten().count();
                if let Some(previous) = points.iter().flatten().next_back()
                    && previous.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: consecutive arc points must differ".to_owned());
                    return false;
                }
                if point_count < points.len() {
                    points[point_count] = Some(point);
                    let command = InteractiveCommand::Arc { points };
                    self.active_command = Some(command);
                    let label = if point_count == 0 { "Start" } else { "Through" };
                    self.push_log(format!("{label}: {}", format_model_point(point)));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(start), Some(through)] = points else {
                        self.push_log("Error: arc point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    if let Err(error) = CircularArc3::try_from_three_points(
                        start,
                        through,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(format!("Error: {error}"));
                        return false;
                    }
                    self.execute_arc_command(&format!(
                        "Arc StartPoint {} ThroughPoint {} {}",
                        format_model_point(start),
                        format_model_point(through),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::Polyline => {
                if let Some(previous) = self.curve_points.last()
                    && !previous
                        .distance_to(point)
                        .is_ok_and(|length| length > self.document.tolerance().absolute())
                {
                    self.push_log(
                        "Error: polyline segment length must be finite and greater than tolerance"
                            .to_owned(),
                    );
                    return false;
                }
                self.curve_points.push(point);
                self.push_log(format!(
                    "Vertex {}: {}",
                    self.curve_points.len(),
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Curve { .. } => {
                if let Some(previous) = self.curve_points.last()
                    && point_input::coincident_curve_controls(*previous, point)
                {
                    self.push_log("Error: adjacent curve control points must differ".to_owned());
                    return false;
                }
                self.curve_points.push(point);
                self.push_log(format!(
                    "Control point {}: {}",
                    self.curve_points.len(),
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::InterpCrv { options } => {
                if let Some(previous) = self.curve_points.last()
                    && point_input::coincident_curve_controls(*previous, point)
                {
                    self.push_log(
                        "Error: adjacent curve interpolation points must differ".to_owned(),
                    );
                    return false;
                }
                if let Some(completed) = self.try_auto_close_interpolation(point, options) {
                    return completed;
                }
                if self.curve_points.len() >= viboceros_geometry::MAX_CURVE_INTERPOLATION_POINTS {
                    self.push_log(format!(
                        "Error: InterpCrv supports at most {} input points; Undo removes the last point",
                        viboceros_geometry::MAX_CURVE_INTERPOLATION_POINTS,
                    ));
                    return false;
                }
                self.curve_points.push(point);
                self.push_log(format!(
                    "Point {}: {}",
                    self.curve_points.len(),
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshPlane {
                first: None,
                x_count,
                y_count,
            } => {
                let command = InteractiveCommand::MeshPlane {
                    first: Some(point),
                    x_count,
                    y_count,
                };
                self.active_command = Some(command);
                self.push_log(format!("First corner: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshPlane {
                first: Some(first),
                x_count,
                y_count,
            } => {
                if !plane_rectangle_exceeds_tolerance(
                    plane,
                    first,
                    point,
                    self.document.tolerance(),
                ) {
                    self.push_log(
                        "Error: mesh-plane width and height must both exceed model tolerance"
                            .to_owned(),
                    );
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "MeshPlane {} {} XCount={} YCount={}",
                    format_model_point(first),
                    format_model_point(point),
                    x_count,
                    y_count
                ));
            }
            InteractiveCommand::MeshBox {
                base: None,
                opposite: None,
                x_count,
                y_count,
                z_count,
            } => {
                let command = InteractiveCommand::MeshBox {
                    base: Some(point),
                    opposite: None,
                    x_count,
                    y_count,
                    z_count,
                };
                self.active_command = Some(command);
                self.push_log(format!("First base corner: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshBox {
                base: Some(base),
                opposite: None,
                x_count,
                y_count,
                z_count,
            } => {
                if !plane_rectangle_exceeds_tolerance(plane, base, point, self.document.tolerance())
                {
                    self.push_log(
                        "Error: mesh-box base width and depth must both exceed model tolerance"
                            .to_owned(),
                    );
                    return false;
                }
                let local_plane = plane.with_origin(base);
                let Ok(opposite) = local_plane
                    .coordinates_of(point)
                    .and_then(|[x, y, _]| local_plane.point_at([x, y, 0.0]))
                else {
                    self.push_log("Error: mesh-box base corner is not finite".to_owned());
                    return false;
                };
                let command = InteractiveCommand::MeshBox {
                    base: Some(base),
                    opposite: Some(opposite),
                    x_count,
                    y_count,
                    z_count,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "Opposite base corner: {}",
                    format_model_point(opposite)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshBox {
                base: Some(base),
                opposite: Some(opposite),
                x_count,
                y_count,
                z_count,
            } => {
                if !plane
                    .with_origin(base)
                    .coordinates_of(point)
                    .is_ok_and(|p| p[2].abs() > self.document.tolerance().absolute())
                {
                    self.push_log("Error: mesh-box height must exceed model tolerance".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "MeshBox {} {} {} XCount={} YCount={} ZCount={}",
                    format_model_point(base),
                    format_model_point(opposite),
                    format_model_point(point),
                    x_count,
                    y_count,
                    z_count
                ));
            }
            InteractiveCommand::MeshBox {
                base: None,
                opposite: Some(_),
                ..
            } => unreachable!("mesh-box opposite corner requires a base corner"),
            InteractiveCommand::Box { base, opposite } => {
                return self.apply_box_point(plane, base, opposite, point, None);
            }
            InteractiveCommand::SelBox {
                base,
                opposite,
                mode,
            } => {
                return self.apply_box_point(plane, base, opposite, point, Some(mode));
            }
            command @ InteractiveCommand::PointGrid { .. } => {
                return self.apply_point_grid_point(plane, command, point);
            }
            InteractiveCommand::MeshCone {
                center: None,
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                let command = InteractiveCommand::MeshCone {
                    center: Some(point),
                    radius_point: None,
                    vertical_count,
                    around_count,
                    solid,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!("Base center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshCone {
                center: Some(center),
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log("Error: mesh-cone radius must exceed model tolerance".to_owned());
                    return false;
                }
                let Ok(radius_point) = Point3::try_new(point.x(), point.y(), center.z()) else {
                    self.push_log("Error: mesh-cone radius point is not finite".to_owned());
                    return false;
                };
                let command = InteractiveCommand::MeshCone {
                    center: Some(center),
                    radius_point: Some(radius_point),
                    vertical_count,
                    around_count,
                    solid,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "Radius point: {}",
                    format_model_point(radius_point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshCone {
                center: Some(center),
                radius_point: Some(radius_point),
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                let height = point.z() - center.z();
                if !height.is_finite() || height.abs() <= self.document.tolerance().absolute() {
                    self.push_log("Error: mesh-cone height must exceed model tolerance".to_owned());
                    return false;
                }
                self.active_command = None;
                let cap_style = match cap_style {
                    MeshCapFaceStyle::Triangles => "Tri",
                    MeshCapFaceStyle::Quadrilaterals => "Quad",
                };
                self.execute_command(&format!(
                    "MeshCone {} {} {height:.6} VerticalFaces={} AroundFaces={} Solid={} CapFaceStyle={}",
                    format_model_point(center),
                    format_model_point(radius_point),
                    vertical_count,
                    around_count,
                    if solid { "Yes" } else { "No" },
                    cap_style
                ));
            }
            InteractiveCommand::MeshCone {
                center: None,
                radius_point: Some(_),
                ..
            } => unreachable!("mesh-cone radius requires a center"),
            InteractiveCommand::MeshTruncatedCone {
                center: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
                ..
            } => {
                let command = InteractiveCommand::MeshTruncatedCone {
                    center: Some(point),
                    base_radius_point: None,
                    end_center: None,
                    vertical_count,
                    around_count,
                    solid,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!("Base center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshTruncatedCone {
                center: Some(center),
                base_radius_point: None,
                end_center: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: mesh truncated-cone base radius must exceed model tolerance"
                            .to_owned(),
                    );
                    return false;
                }
                let Ok(base_radius_point) = Point3::try_new(point.x(), point.y(), center.z())
                else {
                    self.push_log(
                        "Error: mesh truncated-cone base-radius point is not finite".to_owned(),
                    );
                    return false;
                };
                let command = InteractiveCommand::MeshTruncatedCone {
                    center: Some(center),
                    base_radius_point: Some(base_radius_point),
                    end_center: None,
                    vertical_count,
                    around_count,
                    solid,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "Base-radius point: {}",
                    format_model_point(base_radius_point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshTruncatedCone {
                center: Some(center),
                base_radius_point: Some(base_radius_point),
                end_center: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                let height = point.z() - center.z();
                if !height.is_finite() || height.abs() <= self.document.tolerance().absolute() {
                    self.push_log(
                        "Error: mesh truncated-cone height must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                let Ok(end_center) = Point3::try_new(center.x(), center.y(), point.z()) else {
                    self.push_log("Error: mesh truncated-cone end center is not finite".to_owned());
                    return false;
                };
                let command = InteractiveCommand::MeshTruncatedCone {
                    center: Some(center),
                    base_radius_point: Some(base_radius_point),
                    end_center: Some(end_center),
                    vertical_count,
                    around_count,
                    solid,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "End-circle center: {}",
                    format_model_point(end_center)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshTruncatedCone {
                center: Some(center),
                base_radius_point: Some(base_radius_point),
                end_center: Some(end_center),
                vertical_count,
                around_count,
                solid,
                cap_style,
            } => {
                if same_top_point(end_center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: mesh truncated-cone end radius must exceed model tolerance"
                            .to_owned(),
                    );
                    return false;
                }
                let Ok(end_radius_point) = Point3::try_new(point.x(), point.y(), end_center.z())
                else {
                    self.push_log(
                        "Error: mesh truncated-cone end-radius point is not finite".to_owned(),
                    );
                    return false;
                };
                let Ok(end_radius) = end_center.distance_to(end_radius_point) else {
                    self.push_log("Error: mesh truncated-cone end radius is not finite".to_owned());
                    return false;
                };
                let height = end_center.z() - center.z();
                self.active_command = None;
                let cap_style = match cap_style {
                    MeshCapFaceStyle::Triangles => "Tri",
                    MeshCapFaceStyle::Quadrilaterals => "Quad",
                };
                self.execute_command(&format!(
                    "MeshTruncatedCone {} {} {height} {end_radius} VerticalFaces={} AroundFaces={} Solid={} CapFaceStyle={}",
                    format_model_point(center),
                    format_model_point(base_radius_point),
                    vertical_count,
                    around_count,
                    if solid { "Yes" } else { "No" },
                    cap_style
                ));
            }
            InteractiveCommand::MeshTruncatedCone {
                center: Some(_),
                base_radius_point: None,
                end_center: Some(_),
                ..
            } => unreachable!("mesh truncated-cone end center requires a base radius"),
            InteractiveCommand::MeshCylinder {
                center: None,
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                both_sides,
                cap_style,
            } => {
                let command = InteractiveCommand::MeshCylinder {
                    center: Some(point),
                    radius_point: None,
                    vertical_count,
                    around_count,
                    solid,
                    both_sides,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!("Base center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshCylinder {
                center: Some(center),
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                both_sides,
                cap_style,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: mesh-cylinder radius must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                let Ok(radius_point) = Point3::try_new(point.x(), point.y(), center.z()) else {
                    self.push_log("Error: mesh-cylinder radius point is not finite".to_owned());
                    return false;
                };
                let command = InteractiveCommand::MeshCylinder {
                    center: Some(center),
                    radius_point: Some(radius_point),
                    vertical_count,
                    around_count,
                    solid,
                    both_sides,
                    cap_style,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "Radius point: {}",
                    format_model_point(radius_point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshCylinder {
                center: Some(center),
                radius_point: Some(radius_point),
                vertical_count,
                around_count,
                solid,
                both_sides,
                cap_style,
            } => {
                let height = point.z() - center.z();
                if !height.is_finite() || height.abs() <= self.document.tolerance().absolute() {
                    self.push_log(
                        "Error: mesh-cylinder height must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                self.active_command = None;
                let cap_style = match cap_style {
                    MeshCapFaceStyle::Triangles => "Tri",
                    MeshCapFaceStyle::Quadrilaterals => "Quad",
                };
                self.execute_command(&format!(
                    "MeshCylinder {} {} {height:.6} VerticalFaces={} AroundFaces={} Solid={} BothSides={} CapFaceStyle={}",
                    format_model_point(center),
                    format_model_point(radius_point),
                    vertical_count,
                    around_count,
                    if solid { "Yes" } else { "No" },
                    if both_sides { "Yes" } else { "No" },
                    cap_style
                ));
            }
            InteractiveCommand::MeshCylinder {
                center: None,
                radius_point: Some(_),
                ..
            } => unreachable!("mesh-cylinder radius requires a center"),
            InteractiveCommand::MeshSphere {
                center: None,
                topology,
            } => {
                let command = InteractiveCommand::MeshSphere {
                    center: Some(point),
                    topology,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshSphere {
                center: Some(center),
                topology,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: mesh-sphere radius must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                let Ok(radius_point) = Point3::try_new(point.x(), point.y(), center.z()) else {
                    self.push_log("Error: mesh-sphere radius point is not finite".to_owned());
                    return false;
                };
                self.active_command = None;
                let topology_options = match topology {
                    InteractiveMeshSphereTopology::Uv {
                        vertical_count,
                        around_count,
                    } => format!(
                        "Style=UV VerticalFaces={vertical_count} AroundFaces={around_count}"
                    ),
                    InteractiveMeshSphereTopology::Quads { subdivisions } => {
                        format!("Style=Quads Subdivisions={subdivisions}")
                    }
                    InteractiveMeshSphereTopology::Triangles { subdivisions } => {
                        format!("Style=Triangles Subdivisions={subdivisions}")
                    }
                };
                self.execute_command(&format!(
                    "MeshSphere {} {} {}",
                    format_model_point(center),
                    format_model_point(radius_point),
                    topology_options
                ));
            }
            InteractiveCommand::MeshTorus {
                center: None,
                major_point: None,
                vertical_count,
                around_count,
            } => {
                let command = InteractiveCommand::MeshTorus {
                    center: Some(point),
                    major_point: None,
                    vertical_count,
                    around_count,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshTorus {
                center: Some(center),
                major_point: None,
                vertical_count,
                around_count,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: mesh-torus major radius must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                let Ok(major_point) = Point3::try_new(point.x(), point.y(), center.z()) else {
                    self.push_log("Error: mesh-torus major-radius point is not finite".to_owned());
                    return false;
                };
                let command = InteractiveCommand::MeshTorus {
                    center: Some(center),
                    major_point: Some(major_point),
                    vertical_count,
                    around_count,
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "Major-radius point: {}",
                    format_model_point(major_point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::MeshTorus {
                center: Some(center),
                major_point: Some(major_point),
                vertical_count,
                around_count,
            } => {
                let Ok(major_radius) = center.distance_to(major_point) else {
                    self.push_log("Error: mesh-torus major radius is not finite".to_owned());
                    return false;
                };
                let Ok(minor_radius) = major_point.distance_to(point) else {
                    self.push_log("Error: mesh-torus tube radius is not finite".to_owned());
                    return false;
                };
                if minor_radius <= self.document.tolerance().absolute() {
                    self.push_log(
                        "Error: mesh-torus tube radius must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                if minor_radius >= major_radius {
                    self.push_log(
                        "Error: mesh-torus tube radius must be smaller than its major radius"
                            .to_owned(),
                    );
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "MeshTorus {} {} {} VerticalFaces={} AroundFaces={}",
                    format_model_point(center),
                    format_model_point(major_point),
                    minor_radius,
                    vertical_count,
                    around_count
                ));
            }
            InteractiveCommand::MeshTorus {
                center: None,
                major_point: Some(_),
                ..
            } => unreachable!("mesh-torus major radius requires a center"),
            InteractiveCommand::Polygon {
                side_count,
                center: None,
            } => {
                let command = InteractiveCommand::Polygon {
                    side_count,
                    center: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Polygon {
                side_count,
                center: Some(center),
            } => {
                if !plane_radius_exceeds_tolerance(plane, center, point, self.document.tolerance())
                {
                    self.push_log("Error: polygon vertex must differ from its center".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Polygon {side_count} {} {}",
                    format_model_point(center),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::SrfPt { mut corners } => {
                let corner_count = corners.iter().flatten().count();
                if let Some(previous) = corners.iter().flatten().next_back()
                    && previous.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: adjacent surface corners must differ".to_owned());
                    return false;
                }
                if corner_count < corners.len() {
                    corners[corner_count] = Some(point);
                    let command = InteractiveCommand::SrfPt { corners };
                    self.active_command = Some(command);
                    self.push_log(format!(
                        "Corner {}: {}",
                        corner_count + 1,
                        format_model_point(point)
                    ));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(first), Some(second), Some(third)] = corners else {
                        self.push_log("Error: surface corner state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    self.active_command = None;
                    self.execute_command(&format!(
                        "SrfPt {} {} {} {}",
                        format_model_point(first),
                        format_model_point(second),
                        format_model_point(third),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::Curvature { mark } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Curvature MarkCurvature={} {}",
                    if mark { "Yes" } else { "No" },
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Radius {
                diameter,
                mark,
                display_units,
            } => {
                return self.finish_radius(point, diameter, mark, display_units);
            }
            InteractiveCommand::ExtractSrf { .. } => {
                self.pick_extract_face_point(point);
            }
            InteractiveCommand::DupFaceBorder {
                output_on_current_layer,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "DupFaceBorder {} OutputLayer={}",
                    format_model_point(point),
                    if output_on_current_layer {
                        "Current"
                    } else {
                        "Input"
                    },
                ));
            }
            InteractiveCommand::DupEdge {
                output_on_current_layer,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "DupEdge {} OutputLayer={}",
                    format_model_point(point),
                    if output_on_current_layer {
                        "Current"
                    } else {
                        "Input"
                    },
                ));
            }
            InteractiveCommand::ExtractMeshFaces { make_copy } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtractMeshFaces {} MakeCopy={}",
                    format_model_point(point),
                    if make_copy { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::ExtractConnectedMeshFaces {
                angle,
                greater_than,
                make_copy,
                border_only,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtractConnectedMeshFaces FacePoint={} Angle={angle} Compare={} MakeCopy={} BorderOnly={}",
                    format_model_point(point),
                    if greater_than { "Greater" } else { "Less" },
                    if make_copy { "Yes" } else { "No" },
                    if border_only { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::ExtractMeshPart {
                whole_disjoint,
                to_nonmanifold,
                join_output,
                make_copy,
                border_only,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtractMeshPart FacePoint={} ExtractWholeDisjointParts={} ExtractToNonManifoldEdges={} JoinOutput={} MakeCopy={} BorderOnly={}",
                    format_model_point(point),
                    if whole_disjoint { "Yes" } else { "No" },
                    if to_nonmanifold { "Yes" } else { "No" },
                    if join_output { "Yes" } else { "No" },
                    if make_copy { "Yes" } else { "No" },
                    if border_only { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::DeleteFaces => {
                self.active_command = None;
                self.execute_command(&format!("DeleteFaces {}", format_model_point(point)));
            }
            InteractiveCommand::SwapMeshEdge => {
                self.active_command = None;
                self.execute_command(&format!("SwapMeshEdge {}", format_model_point(point)));
            }
            InteractiveCommand::CollapseMeshEdge => {
                self.active_command = None;
                self.execute_command(&format!("CollapseMeshEdge {}", format_model_point(point)));
            }
            InteractiveCommand::SplitMeshEdge { edge_point: None } => {
                let command = InteractiveCommand::SplitMeshEdge {
                    edge_point: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::SplitMeshEdge {
                edge_point: Some(edge_point),
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "SplitMeshEdge {} {}",
                    format_model_point(edge_point),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::FillMeshHole { join_mesh } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "FillMeshHole {} JoinMesh={}",
                    format_model_point(point),
                    if join_mesh { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::WeldEdge => {
                self.active_command = None;
                self.execute_command(&format!("WeldEdge {}", format_model_point(point)));
            }
            InteractiveCommand::WeldVertices => {
                self.active_command = None;
                self.execute_command(&format!("WeldVertices {}", format_model_point(point)));
            }
            InteractiveCommand::UnweldEdge { modify_normals } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "UnweldEdge {} ModifyNormals={}",
                    format_model_point(point),
                    if modify_normals { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::UnweldVertex { modify_normals } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "UnweldVertex {} ModifyNormals={}",
                    format_model_point(point),
                    if modify_normals { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::DupMeshEdge {
                break_angle_degrees,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "DupMeshEdge {} BreakAngle={break_angle_degrees}",
                    format_model_point(point)
                ));
            }
            InteractiveCommand::DupMeshHoleBoundary => {
                self.active_command = None;
                self.execute_command(&format!(
                    "DupMeshHoleBoundary {}",
                    format_model_point(point)
                ));
            }
            InteractiveCommand::ExtractIsocurve {
                direction,
                ignore_trims,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtractIsocurve {} Direction={} IgnoreTrims={}",
                    format_model_point(point),
                    direction.option_value(),
                    if ignore_trims { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::CrvSeam => {
                self.active_command = None;
                self.execute_command(&format!("CrvSeam {}", format_model_point(point)));
            }
            InteractiveCommand::SrfSeam { direction } => {
                self.active_command = None;
                let direction = direction.map_or(String::new(), |direction| {
                    format!(" Direction={}", direction.option_value())
                });
                self.execute_command(&format!("SrfSeam {}{direction}", format_model_point(point)));
            }
            InteractiveCommand::Extend { style, join } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Extend {} Type={} Join={}",
                    format_model_point(point),
                    style.option_value(),
                    join.option_value(),
                ));
            }
            InteractiveCommand::ExtendSrf {
                distance,
                smooth,
                merge,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtendSrf {} Distance={distance} Type={} Merge={}",
                    format_model_point(point),
                    if smooth { "Smooth" } else { "Line" },
                    if merge { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::SubCrv {
                start: None,
                copy,
                delete,
            } => {
                let command = InteractiveCommand::SubCrv {
                    start: Some(point),
                    copy,
                    delete,
                };
                self.active_command = Some(command);
                self.push_log(format!("Start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::SubCrv {
                start: Some(start),
                copy,
                delete,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "{} {} {}{}",
                    if delete { "DeleteSubCrv" } else { "SubCrv" },
                    format_model_point(start),
                    format_model_point(point),
                    if delete {
                        String::new()
                    } else {
                        format!(" Copy={}", if copy { "Yes" } else { "No" })
                    }
                ));
            }
            InteractiveCommand::SplitCurve => {
                self.curve_points.push(point);
                self.push_log(format!(
                    "Split location {}: {}",
                    self.curve_points.len(),
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::SplitCurveWithCutters => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Split CuttingObjects={}",
                    format_model_point(point)
                ));
            }
            InteractiveCommand::SplitSurfaceIsocurve { direction, shrink } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Split Isocurve={} Direction={} Shrink={}",
                    format_model_point(point),
                    direction.option_value(),
                    if shrink { "Yes" } else { "No" },
                ));
            }
            InteractiveCommand::TrimCurve => {
                let normal = self.viewports[self.active_viewport]
                    .apparent_intersection_normal()
                    .to_array();
                self.active_command = None;
                self.execute_command(&format!(
                    "Trim {} ApparentIntersections=Yes ViewNormal={},{},{}",
                    format_model_point(point),
                    normal[0],
                    normal[1],
                    normal[2],
                ));
            }
            InteractiveCommand::Move { start: None } => {
                if self.translation_session.as_ref().is_some_and(|session| {
                    session
                        .normal
                        .as_ref()
                        .is_some_and(|normal| normal.target.is_some())
                }) {
                    return self.accept_move_normal_base(point);
                }
                self.translation_session.as_mut().unwrap().set_base(point);
                self.active_command = Some(InteractiveCommand::Move { start: Some(point) });
                self.push_log(format!("Base: {}", format_model_point(point)));
                self.push_log(
                    InteractiveCommand::Move { start: Some(point) }
                        .prompt()
                        .to_owned(),
                );
            }
            InteractiveCommand::Copy { start: None } => {
                self.translation_session.as_mut().unwrap().set_base(point);
                self.active_command = Some(InteractiveCommand::Copy { start: Some(point) });
                self.push_log(format!("Base: {}", format_model_point(point)));
                self.push_log(
                    InteractiveCommand::Copy { start: Some(point) }
                        .prompt()
                        .to_owned(),
                );
            }
            InteractiveCommand::Move { start: Some(_) } => {
                return self.apply_translation_step(point);
            }
            InteractiveCommand::SetPoint { options } => {
                let succeeded = self.apply_set_point(point, options);
                if !succeeded {
                    self.active_command = Some(InteractiveCommand::SetPoint { options });
                    return false;
                }
                if options.copy {
                    let command = InteractiveCommand::SetPoint { options };
                    self.active_command = Some(command);
                    self.push_log(command.prompt().to_owned());
                }
            }
            InteractiveCommand::Copy { start: Some(_) } => {
                return self.apply_translation_step(point);
            }
            InteractiveCommand::Array {
                counts,
                fill,
                z_distance,
                start: None,
            } => {
                let command = InteractiveCommand::Array {
                    counts,
                    fill,
                    z_distance,
                    start: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("First corner: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Array {
                counts,
                fill,
                z_distance,
                start: Some(start),
            } => {
                let [x_distance, y_distance, _] =
                    match plane.with_origin(start).coordinates_of(point) {
                        Ok(coordinates) => coordinates,
                        Err(error) => {
                            self.push_log(format!("Error: {error}"));
                            return false;
                        }
                    };
                self.active_command = None;
                self.execute_command(&format!(
                    "Array {} {} {} {} {} {} Mode={}",
                    counts[0],
                    counts[1],
                    counts[2],
                    x_distance,
                    y_distance,
                    z_distance,
                    if fill { "Fill" } else { "UnitCell" }
                ));
            }
            InteractiveCommand::Distribute {
                settings,
                start: None,
            } => {
                let command = InteractiveCommand::Distribute {
                    settings,
                    start: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!(
                    "First direction point: {}",
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Distribute {
                settings,
                start: Some(start),
            } => {
                if !start
                    .vector_to(point)
                    .and_then(|v| v.length())
                    .is_ok_and(|length| length > self.document.tolerance().absolute())
                {
                    self.push_log("Error: Distribute direction points must be distinct".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Distribute Direction {} {} Mode={:?} Spacing={}",
                    format_model_point(start),
                    format_model_point(point),
                    settings.mode,
                    settings
                        .spacing
                        .map_or_else(|| "Automatic".to_owned(), |d| d.to_string())
                ));
            }
            InteractiveCommand::ArrayLinear {
                item_count,
                start: None,
            } => {
                let command = InteractiveCommand::ArrayLinear {
                    item_count,
                    start: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("First reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArrayLinear {
                item_count,
                start: Some(start),
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ArrayLinear {item_count} {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::ArrayPolar {
                item_count,
                fill_angle_degrees,
                rotate,
                z_offset,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ArrayPolar {item_count} {} {fill_angle_degrees} Rotate={} ZOffset={z_offset}",
                    format_model_point(point),
                    if rotate { "Yes" } else { "No" }
                ));
            }
            InteractiveCommand::Scale {
                kind, center: None, ..
            } => {
                let command = InteractiveCommand::Scale {
                    kind,
                    center: Some(point),
                    reference: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Scale {
                kind,
                center: Some(center),
                reference: None,
            } => {
                if let Some(result) = self.apply_numeric_scale_direction(command, point) {
                    return result;
                }
                if center.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: scale reference must differ from its center".to_owned());
                    return false;
                }
                let command = InteractiveCommand::Scale {
                    kind,
                    center: Some(center),
                    reference: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Scale {
                kind,
                center: Some(center),
                reference: Some(reference),
            } => {
                if kind != InteractiveScaleKind::OneDimensional
                    && center.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: scale target must differ from its center".to_owned());
                    return false;
                }
                return self.apply_transform_step(
                    &format!(
                        "{} {} {} {}",
                        kind.name(),
                        format_model_point(center),
                        format_model_point(reference),
                        format_model_point(point)
                    ),
                    command,
                );
            }
            InteractiveCommand::Rotate { center: None, .. } => {
                let command = InteractiveCommand::Rotate {
                    center: Some(point),
                    reference: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Rotate {
                center: Some(center),
                reference: None,
            } => {
                if !plane_radius_exceeds_tolerance(plane, center, point, self.document.tolerance())
                {
                    self.push_log("Error: rotate reference must differ from its center".to_owned());
                    return false;
                }
                let command = InteractiveCommand::Rotate {
                    center: Some(center),
                    reference: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Rotate {
                center: Some(center),
                reference: Some(reference),
            } => {
                if !plane_radius_exceeds_tolerance(plane, center, point, self.document.tolerance())
                {
                    self.push_log("Error: rotate target must differ from its center".to_owned());
                    return false;
                }
                return self.apply_transform_step(
                    &format!(
                        "Rotate {} {} {}",
                        format_model_point(center),
                        format_model_point(reference),
                        format_model_point(point)
                    ),
                    command,
                );
            }
            InteractiveCommand::Twist { .. } => return self.accept_twist_point(point),
            InteractiveCommand::Bend { .. } => return self.accept_bend_point(point),
            InteractiveCommand::Stretch { .. } => return self.accept_stretch_point(point),
            InteractiveCommand::Taper { .. } => return self.accept_taper_point(point),
            InteractiveCommand::Maelstrom { .. } => {
                return self.accept_maelstrom_point(point);
            }
            InteractiveCommand::Rotate3D { mut points } => {
                let point_count = points.iter().flatten().count();
                if point_count == 1
                    && points[0]
                        .is_some_and(|start| start.is_near(point, self.document.tolerance()))
                {
                    self.push_log("Error: rotation axis points must differ".to_owned());
                    return false;
                }
                if point_count >= 2 {
                    let [Some(axis_start), Some(axis_end), _] = points else {
                        self.push_log("Error: Rotate3D point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    if point_is_near_axis(axis_start, axis_end, point, self.document.tolerance()) {
                        self.push_log(
                            "Error: Rotate3D reference points must lie off the axis".to_owned(),
                        );
                        return false;
                    }
                }
                if point_count < points.len() {
                    points[point_count] = Some(point);
                    let command = InteractiveCommand::Rotate3D { points };
                    self.active_command = Some(command);
                    let label = ["Axis start", "Axis end", "Reference"][point_count];
                    self.push_log(format!("{label}: {}", format_model_point(point)));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(axis_start), Some(axis_end), Some(reference)] = points else {
                        self.push_log("Error: Rotate3D point state is inconsistent".to_owned());
                        self.active_command = None;
                        return false;
                    };
                    return self.apply_transform_step(
                        &format!(
                            "Rotate3D {} {} {} {}",
                            format_model_point(axis_start),
                            format_model_point(axis_end),
                            format_model_point(reference),
                            format_model_point(point)
                        ),
                        command,
                    );
                }
            }
            InteractiveCommand::Mirror { start: None } => {
                let command = InteractiveCommand::Mirror { start: Some(point) };
                self.active_command = Some(command);
                self.push_log(format!("Axis start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Mirror { start: Some(start) } => {
                if !plane_radius_exceeds_tolerance(plane, start, point, self.document.tolerance()) {
                    self.push_log("Error: mirror axis points must differ".to_owned());
                    return false;
                }
                return self.apply_transform_step(
                    &format!(
                        "Mirror {} {}",
                        format_model_point(start),
                        format_model_point(point)
                    ),
                    command,
                );
            }
            InteractiveCommand::MirrorThreePoint { points } => {
                return self.accept_mirror_three_point(points, point);
            }
            InteractiveCommand::MirrorObject => return false,
            InteractiveCommand::Shear { origin: None, .. } => {
                let command = InteractiveCommand::Shear {
                    origin: Some(point),
                    reference: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Origin: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Shear {
                origin: Some(origin),
                reference: None,
            } => {
                if !plane_radius_exceeds_tolerance(plane, origin, point, self.document.tolerance())
                {
                    self.push_log("Error: shear reference must differ from its origin".to_owned());
                    return false;
                }
                let command = InteractiveCommand::Shear {
                    origin: Some(origin),
                    reference: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Shear {
                origin: Some(origin),
                reference: Some(reference),
            } => {
                if !origin
                    .distance_to(point)
                    .is_ok_and(|d| d > self.document.tolerance().absolute())
                {
                    self.push_log("Error: shear target must differ from its origin".to_owned());
                    return false;
                }
                return self.apply_transform_step(
                    &format!(
                        "Shear {} {} {}",
                        format_model_point(origin),
                        format_model_point(reference),
                        format_model_point(point)
                    ),
                    command,
                );
            }
            InteractiveCommand::ExtrudeCurve {
                base: None,
                both_sides,
                delete_input,
            } => {
                let command = InteractiveCommand::ExtrudeCurve {
                    base: Some(point),
                    both_sides,
                    delete_input,
                };
                self.active_command = Some(command);
                self.push_log(format!("Direction base: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ExtrudeCurve {
                base: Some(base),
                both_sides,
                delete_input,
            } => {
                if base.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: extrusion direction points must differ".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtrudeCrv {} {} BothSides={} DeleteInput={}",
                    format_model_point(base),
                    format_model_point(point),
                    if both_sides { "Yes" } else { "No" },
                    if delete_input { "Yes" } else { "No" }
                ));
            }
            InteractiveCommand::ExtrudeCurveTapered {
                base: None,
                options,
            } => {
                self.active_command = Some(InteractiveCommand::ExtrudeCurveTapered {
                    base: Some(point),
                    options,
                });
                self.push_log("ExtrudeCrvTapered: pick the direction target".into());
            }
            InteractiveCommand::ExtrudeCurveTapered {
                base: Some(base),
                mut options,
            } => {
                options.direction = None;
                return self.finish_tapered_extrusion(
                    &format!("{} {}", format_model_point(base), format_model_point(point)),
                    options,
                );
            }
            InteractiveCommand::ExtrudeSurface {
                base: None,
                options,
            } => {
                self.active_command = Some(InteractiveCommand::ExtrudeSurface {
                    base: Some(point),
                    options,
                });
                self.push_log("ExtrudeSrf: pick the direction target".into());
            }
            InteractiveCommand::ExtrudeSurface {
                base: Some(base),
                mut options,
            } => {
                options.direction = None;
                return self.finish_surface_extrusion(
                    &format!("{} {}", format_model_point(base), format_model_point(point)),
                    options,
                );
            }
            InteractiveCommand::ExtrudeSurfaceTapered {
                base: None,
                options,
            } => {
                self.active_command = Some(InteractiveCommand::ExtrudeSurfaceTapered {
                    base: Some(point),
                    options,
                });
                self.push_log("ExtrudeSrfTapered: pick the direction target".into());
            }
            InteractiveCommand::ExtrudeSurfaceTapered {
                base: Some(base),
                mut options,
            } => {
                options.direction = None;
                return self.finish_tapered_extrusion(
                    &format!("{} {}", format_model_point(base), format_model_point(point)),
                    options,
                );
            }
            InteractiveCommand::ExtrudeSurfaceToPoint { options } => {
                return self.finish_surface_face_command(&format!(
                    "ExtrudeSrfToPoint {} {}",
                    format_model_point(point),
                    options.command_options()
                ));
            }
            InteractiveCommand::ExtrudeCurveToPoint {
                delete_input,
                solid,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtrudeCrvToPoint {} DeleteInput={} Solid={}",
                    format_model_point(point),
                    if delete_input { "Yes" } else { "No" },
                    if solid { "Yes" } else { "No" }
                ));
            }
            InteractiveCommand::Revolve {
                axis_start: None,
                start_angle_degrees,
                sweep_degrees,
                delete_input,
            } => {
                let command = InteractiveCommand::Revolve {
                    axis_start: Some(point),
                    start_angle_degrees,
                    sweep_degrees,
                    delete_input,
                };
                self.active_command = Some(command);
                self.push_log(format!("Axis start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Revolve {
                axis_start: Some(axis_start),
                start_angle_degrees,
                sweep_degrees,
                delete_input,
            } => {
                if axis_start.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: revolve axis points must differ".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Revolve {} {} {} StartAngle={} DeleteInput={}",
                    format_model_point(axis_start),
                    format_model_point(point),
                    sweep_degrees,
                    start_angle_degrees,
                    if delete_input { "Yes" } else { "No" }
                ));
            }
            InteractiveCommand::RailRevolve(mut state) => {
                if let Some(start) = state.axis_start {
                    if start.is_near(point, self.document.tolerance()) {
                        self.push_log("Error: rail revolve axis points must differ".into());
                        return false;
                    }
                    let profiles = self
                        .document
                        .selected_object_ids()
                        .filter(|id| *id != state.rail)
                        .collect::<Vec<_>>();
                    if profiles != [state.profile] {
                        self.push_log("Error: RailRevolve source selection changed".into());
                        return false;
                    }
                    self.active_command = None;
                    self.execute_command(&format!(
                        "RailRevolve {} {} RailId={}{}",
                        format_model_point(start),
                        format_model_point(point),
                        state.rail,
                        state.options.arguments()
                    ));
                } else {
                    state.axis_start = Some(point);
                    self.active_command = Some(InteractiveCommand::RailRevolve(state));
                    self.push_log(format!("Axis start: {}", format_model_point(point)));
                    self.push_log(self.active_command.unwrap().prompt().into());
                }
            }
        }
        true
    }
}
