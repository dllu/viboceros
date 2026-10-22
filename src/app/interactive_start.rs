//! Interactive command parsing and startup.
use super::*;

impl VibocerosApp {
    pub(super) fn try_start_interactive_command(&mut self, input: &str) -> bool {
        self.try_start_interactive_command_with_sources(input, None)
    }

    pub(super) fn try_start_interactive_command_with_sources(
        &mut self,
        input: &str,
        picked_sources: Option<Vec<ObjectId>>,
    ) -> bool {
        if self.try_start_arc_extension(input) {
            return true;
        }
        if self.try_start_arc_deform(input) {
            return true;
        }
        if self.try_start_circle_deform(input) {
            return true;
        }
        if self.try_start_tangent(input) {
            return true;
        }
        if self.try_start_arc_tilted(input) {
            return true;
        }
        if self.try_start_circle_around(input) {
            return true;
        }
        if self.try_start_ellipse(input) {
            return true;
        }
        if self.try_start_catenary(input) {
            return true;
        }
        if self.try_start_spiral(input) {
            return true;
        }
        if self.try_start_hyperbola(input) {
            return true;
        }
        if self.try_start_parabola(input) {
            return true;
        }
        if self.try_start_conic(input) {
            return true;
        }
        if self.start_subd_ellipsoid(input) {
            return true;
        }
        if self.start_subd_torus(input) {
            return true;
        }
        if self.start_subd_sphere(input) {
            return true;
        }
        if self.start_subd_axial(input) {
            return true;
        }
        if self.start_subd_primitive(input) {
            return true;
        }
        if self.try_start_rectangle(input) {
            return true;
        }
        if self.try_start_surface_tapered_input(input)
            || self.try_start_surface_apex_input(input)
            || self.try_start_surface_path_input(input)
            || self.try_start_surface_extrusion_input(input)
        {
            return true;
        }
        if self.try_start_contour_input(input) {
            return true;
        }
        if self.try_start_section_input(input) {
            return true;
        }
        if self.try_start_orient_surface_input(input, picked_sources.as_deref()) {
            return true;
        }
        if self.try_start_remap_cplane(input, picked_sources.as_deref()) {
            return true;
        }
        if self.try_start_orient_points_input(input, picked_sources.as_deref()) {
            return true;
        }
        if self.try_start_surface_flow_input(input, picked_sources.as_deref()) {
            return true;
        }
        if self.try_start_flow_input(input, picked_sources.as_deref()) {
            return true;
        }
        if self.start_surface_array(input, picked_sources.as_deref()) {
            return true;
        }
        if self.start_curve_array(input, picked_sources.as_deref()) {
            return true;
        }
        if self.try_start_array_curve_surface(input, picked_sources.as_deref()) {
            return true;
        }
        if self.start_face_create(input) {
            return true;
        }
        if self.start_mesh_polyline(input) {
            return true;
        }
        if self.start_surface_interpolation(input) {
            return true;
        }
        if self.start_short_path(input) {
            return true;
        }
        if self.start_surface_offset(input) {
            return true;
        }
        if self.start_orient_edge(input) {
            return true;
        }
        if self.try_start_orient_curve_input(input, picked_sources.as_deref()) {
            return true;
        }
        if self.try_start_block_input(input, picked_sources.as_deref()) {
            return true;
        }
        let mut tokens = input.split_whitespace();
        let Some(name) = tokens.next() else {
            return false;
        };
        let arguments = tokens.collect::<Vec<_>>();
        let has_start_arguments = !arguments.is_empty();
        let normalized = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
        let translation_options = translation_prompt::start_options(&normalized, &arguments);
        let translation_normal = translation_options.is_some()
            && arguments.iter().any(|argument| {
                argument
                    .trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Normal")
            });
        let mirror_mode = if normalized == "mirror" {
            viboceros_command::mirror::start_options(&arguments, false)
                .ok()
                .map(|(option, _)| option)
        } else {
            None
        };
        let transform_copy = transform_prompt::supports_name(&normalized).then(|| {
            transform_prompt::start_copy_option(
                &normalized,
                &arguments,
                self.commands.copy_default(&normalized).unwrap(),
            )
        });
        if matches!(normalized.as_str(), "radius" | "diameter")
            && arguments.is_empty()
            && viboceros_command::preselected_circular_radius(&self.document)
                .ok()
                .flatten()
                .is_some()
        {
            return false;
        }
        if normalized == "angle"
            && arguments.is_empty()
            && self.document.selected_object_ids().next().is_some()
        {
            return false;
        }
        if normalized == "selvolumeobject" {
            let selected = self
                .document
                .selected_object_ids()
                .take(2)
                .collect::<Vec<_>>();
            if let [id] = selected.as_slice()
                && self.document.object(*id).is_some_and(|object| {
                    matches!(
                        object.geometry(),
                        Geometry::Mesh(_) | Geometry::Brep(_) | Geometry::NurbsSurface(_)
                    )
                })
            {
                return false;
            }
        }
        let command = if normalized == "twist" {
            let default = self.commands.twist_options_default();
            let Ok((positional, options)) =
                viboceros_command::twist::TwistOptions::from_arguments(&arguments, default)
            else {
                return false;
            };
            if !positional.is_empty() {
                return false;
            }
            InteractiveCommand::Twist {
                points: [None; 3],
                options,
            }
        } else if normalized == "bend" {
            let default = self.commands.bend_options_default();
            let Ok((positional, options)) =
                viboceros_command::bend::BendOptions::from_arguments(&arguments, default)
            else {
                return false;
            };
            if !positional.is_empty() {
                return false;
            }
            InteractiveCommand::Bend {
                points: [None; 2],
                options,
            }
        } else if normalized == "maelstrom" {
            let Ok((positional, options)) =
                viboceros_command::maelstrom::MaelstromOptions::from_arguments(
                    &arguments,
                    self.commands.maelstrom_options_default(),
                )
            else {
                return false;
            };
            if !positional.is_empty() {
                return false;
            }
            InteractiveCommand::Maelstrom {
                center: None,
                initial: None,
                target: None,
                options,
            }
        } else if normalized == "stretch" {
            let mut options = self.commands.stretch_options_default();
            for word in &arguments {
                if !matches!(options.accept(word), Ok(true)) {
                    return false;
                }
            }
            InteractiveCommand::Stretch {
                points: [None; 2],
                factor: None,
                options,
            }
        } else if normalized == "taper" {
            let Ok((positional, options)) = viboceros_command::taper::TaperOptions::from_arguments(
                &arguments,
                self.commands.taper_options_default(),
            ) else {
                return false;
            };
            if !positional.is_empty() {
                return false;
            }
            InteractiveCommand::Taper {
                points: [None; 2],
                initial: None,
                options,
            }
        } else if normalized == "align" {
            let Some(command) = self.start_align(input) else {
                return false;
            };
            command
        } else if normalized == "evaluateuvpt" {
            let Some(command) = self.start_evaluate_uv(input) else {
                return false;
            };
            command
        } else if normalized == "domain" && arguments.is_empty() && self.domain_needs_face_pick() {
            InteractiveCommand::DomainFace
        } else if normalized == "domain"
            && matches!(arguments.as_slice(), [option] if option.trim_start_matches('_').eq_ignore_ascii_case("SubCrv"))
            && self.domain_has_one_curve()
        {
            InteractiveCommand::DomainSubCrv { start: None }
        } else if matches!(normalized.as_str(), "length" | "len")
            && arguments.first().is_some_and(|option| {
                option
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("SubCrv")
            })
            && self.domain_has_one_curve()
        {
            let Some(display_units) = length::start_subcurve(&arguments) else {
                return false;
            };
            InteractiveCommand::LengthSubCrv {
                start: None,
                display_units,
            }
        } else if normalized == "evaluatept" {
            let Some(command) = evaluate_point::start_command(&arguments) else {
                return false;
            };
            command
        } else if normalized == "distance" {
            let Some(command) = distance::start_command(&arguments, self.last_point) else {
                return false;
            };
            command
        } else if normalized == "setpt" && arguments.iter().all(|argument| argument.contains('=')) {
            let Ok(options) =
                viboceros_command::set_point::SetPointOptions::parse_with_copy_default(
                    &arguments,
                    self.commands.copy_default("SetPt").unwrap(),
                )
            else {
                return false;
            };
            InteractiveCommand::SetPoint { options }
        } else if matches!(normalized.as_str(), "circle" | "c")
            && matches!(arguments.as_slice(), [option] if option.trim_start_matches('_').eq_ignore_ascii_case("2Point"))
        {
            InteractiveCommand::CircleTwoPoint { first: None }
        } else if matches!(normalized.as_str(), "circle" | "c")
            && matches!(arguments.as_slice(), [option] if option.trim_start_matches('_').eq_ignore_ascii_case("3Point"))
        {
            InteractiveCommand::CircleThreePoint { points: [None; 2] }
        } else if normalized == "pointgrid" {
            let Ok(options) = viboceros_command::PointGridOptions::parse(&arguments) else {
                return false;
            };
            InteractiveCommand::PointGrid {
                base: None,
                opposite: None,
                third: None,
                width: None,
                options,
            }
        } else if normalized == "meshellipsoid" {
            let mut vertical_count = DEFAULT_MESH_ELLIPSOID_FACE_COUNT;
            let mut around_count = DEFAULT_MESH_ELLIPSOID_FACE_COUNT;
            let mut cap_style = MeshCapFaceStyle::Triangles;
            let mut seen = [false; 3];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 2 {
                        return false;
                    }
                    vertical_count = count;
                    0
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = count;
                    1
                } else if name.eq_ignore_ascii_case("CapFaceStyle") {
                    cap_style = if value.eq_ignore_ascii_case("Tri")
                        || value.eq_ignore_ascii_case("Triangle")
                        || value.eq_ignore_ascii_case("Triangles")
                    {
                        MeshCapFaceStyle::Triangles
                    } else if value.eq_ignore_ascii_case("Quad")
                        || value.eq_ignore_ascii_case("Quadrilateral")
                        || value.eq_ignore_ascii_case("Quadrilaterals")
                    {
                        MeshCapFaceStyle::Quadrilaterals
                    } else {
                        return false;
                    };
                    2
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            let face_count = if cap_style == MeshCapFaceStyle::Quadrilaterals
                && around_count.is_multiple_of(2)
            {
                vertical_count
                    .checked_sub(1)
                    .and_then(|bands| bands.checked_mul(around_count))
            } else {
                vertical_count.checked_mul(around_count)
            };
            if face_count.is_none_or(|count| count > MAX_MESH_ELLIPSOID_FACES) {
                return false;
            }
            InteractiveCommand::MeshEllipsoid {
                points: [None; 3],
                vertical_count,
                around_count,
                cap_style,
            }
        } else if normalized == "meshsphere" {
            let mut topology = InteractiveMeshSphereTopology::Uv {
                vertical_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
                around_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
            };
            let mut vertical_count = None;
            let mut around_count = None;
            let mut subdivisions = None;
            let mut seen = [false; 4];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("Style") {
                    topology = if value.eq_ignore_ascii_case("UV") {
                        InteractiveMeshSphereTopology::Uv {
                            vertical_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
                            around_count: DEFAULT_MESH_SPHERE_FACE_COUNT,
                        }
                    } else if value.eq_ignore_ascii_case("Quad")
                        || value.eq_ignore_ascii_case("Quads")
                    {
                        InteractiveMeshSphereTopology::Quads {
                            subdivisions: DEFAULT_MESH_SPHERE_SUBDIVISIONS,
                        }
                    } else if value.eq_ignore_ascii_case("Triangle")
                        || value.eq_ignore_ascii_case("Triangles")
                    {
                        InteractiveMeshSphereTopology::Triangles {
                            subdivisions: DEFAULT_MESH_SPHERE_SUBDIVISIONS,
                        }
                    } else {
                        return false;
                    };
                    0
                } else if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 2 {
                        return false;
                    }
                    vertical_count = Some(count);
                    1
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = Some(count);
                    2
                } else if name.eq_ignore_ascii_case("Subdivisions") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    subdivisions = Some(count);
                    3
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            topology = match topology {
                InteractiveMeshSphereTopology::Uv { .. } => {
                    if subdivisions.is_some() {
                        return false;
                    }
                    let vertical_count = vertical_count.unwrap_or(DEFAULT_MESH_SPHERE_FACE_COUNT);
                    let around_count = around_count.unwrap_or(DEFAULT_MESH_SPHERE_FACE_COUNT);
                    if vertical_count
                        .checked_mul(around_count)
                        .is_none_or(|faces| faces > MAX_MESH_SPHERE_FACES)
                    {
                        return false;
                    }
                    InteractiveMeshSphereTopology::Uv {
                        vertical_count,
                        around_count,
                    }
                }
                InteractiveMeshSphereTopology::Quads { .. } => {
                    if vertical_count.is_some() || around_count.is_some() {
                        return false;
                    }
                    let subdivisions = subdivisions.unwrap_or(DEFAULT_MESH_SPHERE_SUBDIVISIONS);
                    if subdivisions > MAX_MESH_SPHERE_QUAD_SUBDIVISIONS {
                        return false;
                    }
                    InteractiveMeshSphereTopology::Quads { subdivisions }
                }
                InteractiveMeshSphereTopology::Triangles { .. } => {
                    if vertical_count.is_some() || around_count.is_some() {
                        return false;
                    }
                    let subdivisions = subdivisions.unwrap_or(DEFAULT_MESH_SPHERE_SUBDIVISIONS);
                    if subdivisions > MAX_MESH_SPHERE_TRIANGLE_SUBDIVISIONS {
                        return false;
                    }
                    InteractiveMeshSphereTopology::Triangles { subdivisions }
                }
            };
            InteractiveCommand::MeshSphere {
                center: None,
                topology,
            }
        } else if normalized == "meshtorus" {
            let mut vertical_count = DEFAULT_MESH_TORUS_FACE_COUNT;
            let mut around_count = DEFAULT_MESH_TORUS_FACE_COUNT;
            let mut seen = [false; 2];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    vertical_count = count;
                    0
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = count;
                    1
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            if vertical_count
                .checked_mul(around_count)
                .is_none_or(|faces| faces > MAX_MESH_TORUS_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshTorus {
                center: None,
                major_point: None,
                vertical_count,
                around_count,
            }
        } else if normalized == "meshtruncatedcone" {
            let mut vertical_count = DEFAULT_MESH_TRUNCATED_CONE_FACE_COUNT;
            let mut around_count = DEFAULT_MESH_TRUNCATED_CONE_FACE_COUNT;
            let mut solid = true;
            let mut cap_style = MeshCapFaceStyle::Triangles;
            let mut seen = [false; 4];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count == 0 {
                        return false;
                    }
                    vertical_count = count;
                    0
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = count;
                    1
                } else if name.eq_ignore_ascii_case("Solid") {
                    solid = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    2
                } else if name.eq_ignore_ascii_case("CapFaceStyle") {
                    cap_style = if value.eq_ignore_ascii_case("Tri")
                        || value.eq_ignore_ascii_case("Triangle")
                        || value.eq_ignore_ascii_case("Triangles")
                    {
                        MeshCapFaceStyle::Triangles
                    } else if value.eq_ignore_ascii_case("Quad")
                        || value.eq_ignore_ascii_case("Quadrilateral")
                        || value.eq_ignore_ascii_case("Quadrilaterals")
                    {
                        MeshCapFaceStyle::Quadrilaterals
                    } else {
                        return false;
                    };
                    3
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            let wall_faces = vertical_count.checked_mul(around_count);
            let cap_faces = if !solid {
                Some(0)
            } else {
                let one_cap = if cap_style == MeshCapFaceStyle::Quadrilaterals
                    && around_count.is_multiple_of(2)
                {
                    if around_count == 4 {
                        1
                    } else {
                        around_count / 2
                    }
                } else {
                    around_count
                };
                one_cap.checked_mul(2)
            };
            if wall_faces
                .and_then(|wall| cap_faces.and_then(|caps| wall.checked_add(caps)))
                .is_none_or(|faces| faces > MAX_MESH_TRUNCATED_CONE_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshTruncatedCone {
                center: None,
                base_radius_point: None,
                end_center: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            }
        } else if normalized == "meshcone" {
            let mut vertical_count = DEFAULT_MESH_CONE_FACE_COUNT;
            let mut around_count = DEFAULT_MESH_CONE_FACE_COUNT;
            let mut solid = true;
            let mut cap_style = MeshCapFaceStyle::Triangles;
            let mut seen = [false; 4];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count == 0 {
                        return false;
                    }
                    vertical_count = count;
                    0
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = count;
                    1
                } else if name.eq_ignore_ascii_case("Solid") {
                    solid = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    2
                } else if name.eq_ignore_ascii_case("CapFaceStyle") {
                    cap_style = if value.eq_ignore_ascii_case("Tri")
                        || value.eq_ignore_ascii_case("Triangle")
                        || value.eq_ignore_ascii_case("Triangles")
                    {
                        MeshCapFaceStyle::Triangles
                    } else if value.eq_ignore_ascii_case("Quad")
                        || value.eq_ignore_ascii_case("Quadrilateral")
                        || value.eq_ignore_ascii_case("Quadrilaterals")
                    {
                        MeshCapFaceStyle::Quadrilaterals
                    } else {
                        return false;
                    };
                    3
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            let wall_faces = vertical_count.checked_mul(around_count);
            let cap_faces = if !solid {
                Some(0)
            } else if cap_style == MeshCapFaceStyle::Quadrilaterals
                && around_count.is_multiple_of(2)
            {
                Some(if around_count == 4 {
                    1
                } else {
                    around_count / 2
                })
            } else {
                Some(around_count)
            };
            if wall_faces
                .and_then(|wall| cap_faces.and_then(|cap| wall.checked_add(cap)))
                .is_none_or(|faces| faces > MAX_MESH_CONE_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshCone {
                center: None,
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                cap_style,
            }
        } else if normalized == "meshcylinder" {
            let mut vertical_count = DEFAULT_MESH_CYLINDER_FACE_COUNT;
            let mut around_count = DEFAULT_MESH_CYLINDER_FACE_COUNT;
            let mut solid = true;
            let mut both_sides = false;
            let mut cap_style = MeshCapFaceStyle::Triangles;
            let mut seen = [false; 5];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let option_index = if name.eq_ignore_ascii_case("VerticalFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count == 0 {
                        return false;
                    }
                    vertical_count = count;
                    0
                } else if name.eq_ignore_ascii_case("AroundFaces") {
                    let Ok(count) = value.parse::<usize>() else {
                        return false;
                    };
                    if count < 3 {
                        return false;
                    }
                    around_count = count;
                    1
                } else if name.eq_ignore_ascii_case("Solid") {
                    solid = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    2
                } else if name.eq_ignore_ascii_case("BothSides") {
                    both_sides = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    3
                } else if name.eq_ignore_ascii_case("CapFaceStyle") {
                    cap_style = if value.eq_ignore_ascii_case("Tri")
                        || value.eq_ignore_ascii_case("Triangle")
                        || value.eq_ignore_ascii_case("Triangles")
                    {
                        MeshCapFaceStyle::Triangles
                    } else if value.eq_ignore_ascii_case("Quad")
                        || value.eq_ignore_ascii_case("Quadrilateral")
                        || value.eq_ignore_ascii_case("Quadrilaterals")
                    {
                        MeshCapFaceStyle::Quadrilaterals
                    } else {
                        return false;
                    };
                    4
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                seen[option_index] = true;
            }
            let wall_faces = vertical_count.checked_mul(around_count);
            let cap_faces = if !solid {
                Some(0)
            } else if cap_style == MeshCapFaceStyle::Quadrilaterals
                && around_count.is_multiple_of(2)
            {
                Some(if around_count == 4 { 2 } else { around_count })
            } else {
                around_count.checked_mul(2)
            };
            if wall_faces
                .and_then(|wall| cap_faces.and_then(|caps| wall.checked_add(caps)))
                .is_none_or(|faces| faces > MAX_MESH_CYLINDER_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshCylinder {
                center: None,
                radius_point: None,
                vertical_count,
                around_count,
                solid,
                both_sides,
                cap_style,
            }
        } else if normalized == "meshbox" {
            let mut x_count = DEFAULT_MESH_BOX_FACE_COUNT;
            let mut y_count = DEFAULT_MESH_BOX_FACE_COUNT;
            let mut z_count = DEFAULT_MESH_BOX_FACE_COUNT;
            let mut seen = [false; 3];
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let Ok(count) = value.parse::<usize>() else {
                    return false;
                };
                if count == 0 {
                    return false;
                }
                let option_index = if name.eq_ignore_ascii_case("XCount") {
                    0
                } else if name.eq_ignore_ascii_case("YCount") {
                    1
                } else if name.eq_ignore_ascii_case("ZCount") {
                    2
                } else {
                    return false;
                };
                if seen[option_index] {
                    return false;
                }
                match option_index {
                    0 => x_count = count,
                    1 => y_count = count,
                    2 => z_count = count,
                    _ => unreachable!("mesh-box option index is in range"),
                }
                seen[option_index] = true;
            }
            if x_count
                .checked_mul(y_count)
                .and_then(|xy| {
                    x_count
                        .checked_mul(z_count)
                        .and_then(|xz| xy.checked_add(xz))
                })
                .and_then(|xy_xz| {
                    y_count
                        .checked_mul(z_count)
                        .and_then(|yz| xy_xz.checked_add(yz))
                })
                .and_then(|half| half.checked_mul(2))
                .is_none_or(|face_count| face_count > MAX_MESH_BOX_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshBox {
                base: None,
                opposite: None,
                x_count,
                y_count,
                z_count,
            }
        } else if normalized == "meshplane" {
            let mut x_count = DEFAULT_MESH_PLANE_FACE_COUNT;
            let mut y_count = DEFAULT_MESH_PLANE_FACE_COUNT;
            let mut x_seen = false;
            let mut y_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let Ok(count) = value.parse::<usize>() else {
                    return false;
                };
                if count == 0 {
                    return false;
                }
                if name.eq_ignore_ascii_case("XCount") && !x_seen {
                    x_count = count;
                    x_seen = true;
                } else if name.eq_ignore_ascii_case("YCount") && !y_seen {
                    y_count = count;
                    y_seen = true;
                } else {
                    return false;
                }
            }
            if x_count
                .checked_mul(y_count)
                .is_none_or(|face_count| face_count > MAX_MESH_PLANE_FACES)
            {
                return false;
            }
            InteractiveCommand::MeshPlane {
                first: None,
                x_count,
                y_count,
            }
        } else if matches!(normalized.as_str(), "interpcrv" | "interpcurve") {
            let Ok(options) = parse_interp_curve_options(&arguments) else {
                return false;
            };
            InteractiveCommand::InterpCrv { options }
        } else if normalized == "curve" {
            let mut degree = 3;
            let mut closure = ControlPointCurveClosure::Open;
            let mut degree_seen = false;
            let mut close_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Degree") && !degree_seen {
                    let Ok(parsed) = parse_curve_degree(value) else {
                        return false;
                    };
                    degree = parsed;
                    degree_seen = true;
                } else if name.eq_ignore_ascii_case("Close") && !close_seen {
                    let Ok(parsed) = parse_curve_closure(value) else {
                        return false;
                    };
                    closure = parsed;
                    close_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::Curve { degree, closure }
        } else if normalized == "extrudecrv" {
            let mut both_sides = false;
            let mut delete_input = false;
            let mut both_sides_seen = false;
            let mut delete_input_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let parsed = if value.eq_ignore_ascii_case("Yes") {
                    true
                } else if value.eq_ignore_ascii_case("No") {
                    false
                } else {
                    return false;
                };
                if name.eq_ignore_ascii_case("BothSides") && !both_sides_seen {
                    both_sides = parsed;
                    both_sides_seen = true;
                } else if name.eq_ignore_ascii_case("DeleteInput") && !delete_input_seen {
                    delete_input = parsed;
                    delete_input_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ExtrudeCurve {
                base: None,
                both_sides,
                delete_input,
            }
        } else if normalized == "extrudecrvtapered" {
            let Ok(options) =
                viboceros_command::extrude_tapered::TaperedExtrusionOptions::parse(&arguments)
            else {
                return false;
            };
            InteractiveCommand::ExtrudeCurveTapered {
                base: None,
                options,
            }
        } else if normalized == "extrudecrvtopoint" {
            let mut delete_input = false;
            let mut solid = false;
            let mut delete_input_seen = false;
            let mut solid_seen = false;
            let mut output_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let yes_no = if value.eq_ignore_ascii_case("Yes") {
                    true
                } else if value.eq_ignore_ascii_case("No") {
                    false
                } else {
                    if name.eq_ignore_ascii_case("Output")
                        && !output_seen
                        && value.eq_ignore_ascii_case("Surface")
                    {
                        output_seen = true;
                        continue;
                    }
                    return false;
                };
                if name.eq_ignore_ascii_case("DeleteInput") && !delete_input_seen {
                    delete_input = yes_no;
                    delete_input_seen = true;
                } else if name.eq_ignore_ascii_case("Solid") && !solid_seen {
                    solid = yes_no;
                    solid_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ExtrudeCurveToPoint {
                delete_input,
                solid,
            }
        } else if normalized == "railrevolve" {
            let options = match self.commands.object_selection_prompt(input) {
                Ok(Some(p)) => viboceros_command::rail_revolve::RailRevolveOptions::from_prompt(&p),
                _ => return false,
            };
            match viboceros_command::rail_revolve::start(&self.document, &arguments, options) {
                Ok(Some(state)) => InteractiveCommand::RailRevolve(state),
                Ok(None) => return false,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
            }
        } else if normalized == "revolve" {
            let mut start_angle_degrees = 0.0;
            let mut sweep_degrees = 360.0;
            let mut delete_input = false;
            let mut start_angle_seen = false;
            let mut sweep_seen = false;
            let mut delete_input_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("StartAngle") && !start_angle_seen {
                    let Ok(angle) = value.parse::<f64>() else {
                        return false;
                    };
                    if !angle.is_finite() {
                        return false;
                    }
                    start_angle_degrees = angle;
                    start_angle_seen = true;
                } else if matches!(name.to_ascii_lowercase().as_str(), "angle" | "sweepangle")
                    && !sweep_seen
                {
                    let Ok(angle) = value.parse::<f64>() else {
                        return false;
                    };
                    if !angle.is_finite() || angle == 0.0 || angle.abs() > 360.0 {
                        return false;
                    }
                    sweep_degrees = angle;
                    sweep_seen = true;
                } else if name.eq_ignore_ascii_case("DeleteInput") && !delete_input_seen {
                    delete_input = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    delete_input_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::Revolve {
                axis_start: None,
                start_angle_degrees,
                sweep_degrees,
                delete_input,
            }
        } else if matches!(normalized.as_str(), "curvature" | "radius" | "diameter") {
            let mut mark = false;
            let mut display_units = None;
            let mut units_seen = false;
            let mark_option = match normalized.as_str() {
                "radius" => "MarkRadius",
                "diameter" => "MarkDiameter",
                _ => "MarkCurvature",
            };
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                if name.eq_ignore_ascii_case(mark_option) {
                    mark = if value.trim_start_matches('_').eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.trim_start_matches('_').eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                } else if name.eq_ignore_ascii_case("Units")
                    && normalized != "curvature"
                    && !units_seen
                    && self.document.selected_object_count() == 0
                {
                    display_units = match viboceros_command::distance_display_units(value) {
                        Ok(units) => units,
                        Err(_) => return false,
                    };
                    units_seen = true;
                } else {
                    return false;
                }
            }
            if normalized == "curvature" {
                InteractiveCommand::Curvature { mark }
            } else {
                InteractiveCommand::Radius {
                    diameter: normalized == "diameter",
                    mark,
                    display_units,
                }
            }
        } else if matches!(normalized.as_str(), "extractsrf" | "extractsurface") {
            let Some((copy, output_on_current_layer)) = extract_surface::options(
                &arguments,
                self.commands.copy_default("ExtractSrf").unwrap(),
                false,
            ) else {
                return false;
            };
            InteractiveCommand::ExtractSrf {
                copy,
                output_on_current_layer,
            }
        } else if matches!(normalized.as_str(), "dupfaceborder" | "duplicatefaceborder") {
            let mut output_on_current_layer = true;
            let mut output_layer_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("OutputLayer") && !output_layer_seen {
                    output_on_current_layer = if value.eq_ignore_ascii_case("Current") {
                        true
                    } else if value.eq_ignore_ascii_case("Input") {
                        false
                    } else {
                        return false;
                    };
                    output_layer_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::DupFaceBorder {
                output_on_current_layer,
            }
        } else if matches!(normalized.as_str(), "dupedge" | "duplicateedge") {
            let mut output_on_current_layer = true;
            let mut output_layer_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("OutputLayer") && !output_layer_seen {
                    output_on_current_layer = if value.eq_ignore_ascii_case("Current") {
                        true
                    } else if value.eq_ignore_ascii_case("Input") {
                        false
                    } else {
                        return false;
                    };
                    output_layer_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::DupEdge {
                output_on_current_layer,
            }
        } else if normalized == "extractmeshfaces" {
            let mut make_copy = false;
            let mut make_copy_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("MakeCopy") && !make_copy_seen {
                    make_copy = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    make_copy_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ExtractMeshFaces { make_copy }
        } else if normalized == "extractconnectedmeshfaces" {
            let Some(command) = mesh_face_prompt::parse_connected(&arguments) else {
                return false;
            };
            command
        } else if normalized == "extractmeshpart" {
            let Some(command) = mesh_face_prompt::parse_part(&arguments) else {
                return false;
            };
            command
        } else if normalized == "deletefaces" {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::DeleteFaces
        } else if normalized == "swapmeshedge" {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::SwapMeshEdge
        } else if normalized == "collapsemeshedge" {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::CollapseMeshEdge
        } else if normalized == "splitmeshedge" {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::SplitMeshEdge { edge_point: None }
        } else if normalized == "fillmeshhole" {
            let mut join_mesh = true;
            let mut join_mesh_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("JoinMesh") && !join_mesh_seen {
                    join_mesh = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    join_mesh_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::FillMeshHole { join_mesh }
        } else if matches!(normalized.as_str(), "weldedge" | "weldmeshedge") {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::WeldEdge
        } else if normalized == "pipe" {
            let mut cap_flat = true;
            let mut cap_round = false;
            let mut blend_global = false;
            let mut fit_rail = None;
            let mut cap_seen = false;
            let mut blend_seen = false;
            let mut thick = None;
            let mut wall_thickness = None;
            for argument in &arguments {
                let Some((name, value)) = argument.split_once('=') else {
                    return false;
                };
                if name.trim_start_matches('_').eq_ignore_ascii_case("Cap") && !cap_seen {
                    cap_flat = if value.trim_start_matches('_').eq_ignore_ascii_case("Flat") {
                        true
                    } else if value.trim_start_matches('_').eq_ignore_ascii_case("None") {
                        false
                    } else if value.trim_start_matches('_').eq_ignore_ascii_case("Round") {
                        cap_round = true;
                        false
                    } else {
                        return false;
                    };
                    cap_seen = true;
                } else if name
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("ShapeBlending")
                    && !blend_seen
                {
                    blend_global = if value.trim_start_matches('_').eq_ignore_ascii_case("Global") {
                        true
                    } else if value.trim_start_matches('_').eq_ignore_ascii_case("Local") {
                        false
                    } else {
                        return false;
                    };
                    blend_seen = true;
                } else if name.trim_start_matches('_').eq_ignore_ascii_case("FitRail")
                    && fit_rail.is_none()
                {
                    fit_rail = if value.trim_start_matches('_').eq_ignore_ascii_case("Yes") {
                        Some(true)
                    } else if value.trim_start_matches('_').eq_ignore_ascii_case("No") {
                        Some(false)
                    } else {
                        return false;
                    };
                } else if name.trim_start_matches('_').eq_ignore_ascii_case("Thick")
                    && thick.is_none()
                {
                    thick = if value.trim_start_matches('_').eq_ignore_ascii_case("Yes") {
                        Some(true)
                    } else if value.trim_start_matches('_').eq_ignore_ascii_case("No") {
                        Some(false)
                    } else {
                        return false;
                    };
                } else if name
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("WallThickness")
                    && wall_thickness.is_none()
                {
                    let Ok(value) = value.parse::<f64>() else {
                        return false;
                    };
                    if !value.is_finite() || value == 0.0 {
                        return false;
                    }
                    wall_thickness = Some(value);
                } else {
                    return false;
                }
            }
            if thick == Some(false) && wall_thickness.is_some() {
                return false;
            }
            let pick_second_radius = thick == Some(true) && wall_thickness.is_none();
            let selected = self
                .document
                .selected_object_ids()
                .take(2)
                .collect::<Vec<_>>();
            let source = match selected.as_slice() {
                [id] if self
                    .document
                    .object(*id)
                    .and_then(|object| object.geometry().curve_ref())
                    .is_some() =>
                {
                    Some(*id)
                }
                _ => None,
            };
            InteractiveCommand::Pipe {
                source,
                cap_flat,
                cap_round,
                blend_global,
                fit_rail: fit_rail.unwrap_or(false),
                wall_thickness,
                pick_second_radius,
                first_radius: None,
            }
        } else if matches!(
            normalized.as_str(),
            "weldvertices" | "weldvertex" | "weldmeshvertex"
        ) {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::WeldVertices
        } else if matches!(normalized.as_str(), "unweldedge" | "unweldmeshedge") {
            let mut modify_normals = true;
            let mut modify_normals_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("ModifyNormals") && !modify_normals_seen {
                    modify_normals = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    modify_normals_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::UnweldEdge { modify_normals }
        } else if matches!(
            normalized.as_str(),
            "unweldvertex" | "unweldvertices" | "unweldmeshvertex"
        ) {
            let mut modify_normals = true;
            let mut modify_normals_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("ModifyNormals") && !modify_normals_seen {
                    modify_normals = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    modify_normals_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::UnweldVertex { modify_normals }
        } else if matches!(normalized.as_str(), "dupmeshedge" | "duplicatemeshedge") {
            let mut break_angle_degrees = 90.0;
            let mut break_angle_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("BreakAngle") && !break_angle_seen {
                    let Ok(angle) = value.parse::<f64>() else {
                        return false;
                    };
                    if !angle.is_finite() || !(0.0..=180.0).contains(&angle) {
                        return false;
                    }
                    break_angle_degrees = angle;
                    break_angle_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::DupMeshEdge {
                break_angle_degrees,
            }
        } else if matches!(
            normalized.as_str(),
            "dupmeshholeboundary" | "duplicatemeshholeboundary"
        ) {
            if !arguments.is_empty() {
                return false;
            }
            InteractiveCommand::DupMeshHoleBoundary
        } else if normalized == "extend" {
            let mut style = InteractiveCurveExtensionStyle::Natural;
            let mut join = InteractiveCurveExtensionJoin::Merge;
            let mut type_seen = false;
            let mut join_seen = false;
            let mut index = 0;
            while index < arguments.len() {
                let argument = arguments[index];
                let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=')
                {
                    (name, value, 1)
                } else {
                    let Some(value) = arguments.get(index + 1) else {
                        return false;
                    };
                    (argument, *value, 2)
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Type") && !type_seen {
                    style = if value.eq_ignore_ascii_case("Natural") {
                        InteractiveCurveExtensionStyle::Natural
                    } else if value.eq_ignore_ascii_case("Arc") {
                        InteractiveCurveExtensionStyle::Arc
                    } else if value.eq_ignore_ascii_case("Line") {
                        InteractiveCurveExtensionStyle::Line
                    } else if value.eq_ignore_ascii_case("Smooth") {
                        InteractiveCurveExtensionStyle::Smooth
                    } else {
                        return false;
                    };
                    type_seen = true;
                } else if name.eq_ignore_ascii_case("Join") && !join_seen {
                    join = if value.eq_ignore_ascii_case("Merge") {
                        InteractiveCurveExtensionJoin::Merge
                    } else if value.eq_ignore_ascii_case("Yes") {
                        InteractiveCurveExtensionJoin::Yes
                    } else if value.eq_ignore_ascii_case("No") {
                        InteractiveCurveExtensionJoin::No
                    } else {
                        return false;
                    };
                    join_seen = true;
                } else {
                    return false;
                }
                index += consumed;
            }
            InteractiveCommand::Extend { style, join }
        } else if normalized == "extendsrf" {
            let mut distance = None;
            let mut smooth = true;
            let mut merge = true;
            let mut type_seen = false;
            let mut merge_seen = false;
            let mut index = 0;
            while index < arguments.len() {
                let argument = arguments[index];
                let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=')
                {
                    (name, value, 1)
                } else {
                    let Some(value) = arguments.get(index + 1) else {
                        return false;
                    };
                    (argument, *value, 2)
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Distance") && distance.is_none() {
                    let Ok(parsed) = value.parse::<f64>() else {
                        return false;
                    };
                    if !parsed.is_finite() || parsed == 0.0 {
                        return false;
                    }
                    distance = Some(parsed);
                } else if name.eq_ignore_ascii_case("Type") && !type_seen {
                    smooth = if value.eq_ignore_ascii_case("Smooth") {
                        true
                    } else if value.eq_ignore_ascii_case("Line") {
                        false
                    } else {
                        return false;
                    };
                    type_seen = true;
                } else if name.eq_ignore_ascii_case("Merge") && !merge_seen {
                    merge = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    merge_seen = true;
                } else {
                    return false;
                }
                index += consumed;
            }
            let Some(distance) = distance else {
                return false;
            };
            if !merge && distance < 0.0 {
                return false;
            }
            InteractiveCommand::ExtendSrf {
                distance,
                smooth,
                merge,
            }
        } else if normalized == "srfseam" {
            let mut direction = None;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if !name.eq_ignore_ascii_case("Direction") || direction.is_some() {
                    return false;
                }
                direction = Some(if value.eq_ignore_ascii_case("U") {
                    InteractiveIsocurveDirection::U
                } else if value.eq_ignore_ascii_case("V") {
                    InteractiveIsocurveDirection::V
                } else if value.eq_ignore_ascii_case("Both") {
                    InteractiveIsocurveDirection::Both
                } else {
                    return false;
                });
            }
            InteractiveCommand::SrfSeam { direction }
        } else if matches!(normalized.as_str(), "deletesubcrv" | "break") && arguments.is_empty() {
            InteractiveCommand::SubCrv {
                start: None,
                copy: false,
                delete: true,
            }
        } else if normalized == "subcrv" {
            let mut copy = self.commands.subcurve_defaults().copy;
            let mut copy_seen = false;
            let mut mode_seen = false;
            let mut midpoint_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("FromMidpoint") {
                    if midpoint_seen
                        || !(value.eq_ignore_ascii_case("Yes") || value.eq_ignore_ascii_case("No"))
                    {
                        return false;
                    }
                    midpoint_seen = true;
                    continue;
                }
                if name.eq_ignore_ascii_case("Mode") {
                    if mode_seen
                        || viboceros_command::subcurve_input::SubcurveMode::parse(value).is_none()
                    {
                        return false;
                    }
                    mode_seen = true;
                    continue;
                }
                if !name.eq_ignore_ascii_case("Copy") || copy_seen {
                    return false;
                }
                copy = if value.eq_ignore_ascii_case("Yes") {
                    true
                } else if value.eq_ignore_ascii_case("No") {
                    false
                } else {
                    return false;
                };
                copy_seen = true;
            }
            InteractiveCommand::SubCrv {
                start: None,
                copy,
                delete: false,
            }
        } else if normalized == "split"
            && matches!(
                arguments.as_slice(),
                [option]
                    if option
                        .trim_start_matches(['_', '-'])
                        .eq_ignore_ascii_case("CuttingObjects")
            )
        {
            InteractiveCommand::SplitCurveWithCutters
        } else if normalized == "split"
            && arguments.first().is_some_and(|option| {
                option
                    .trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Isocurve")
            })
        {
            let mut direction = InteractiveIsocurveDirection::U;
            let mut shrink = true;
            let mut direction_seen = false;
            let mut shrink_seen = false;
            for option in &arguments[1..] {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Direction") && !direction_seen {
                    direction = if value.eq_ignore_ascii_case("U") {
                        InteractiveIsocurveDirection::U
                    } else if value.eq_ignore_ascii_case("V") {
                        InteractiveIsocurveDirection::V
                    } else if value.eq_ignore_ascii_case("Both") {
                        InteractiveIsocurveDirection::Both
                    } else {
                        return false;
                    };
                    direction_seen = true;
                } else if name.eq_ignore_ascii_case("Shrink") && !shrink_seen {
                    shrink = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    shrink_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::SplitSurfaceIsocurve { direction, shrink }
        } else if matches!(normalized.as_str(), "extractisocurve" | "isocurve") {
            let mut direction = InteractiveIsocurveDirection::U;
            let mut ignore_trims = false;
            let mut direction_seen = false;
            let mut ignore_trims_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Direction") && !direction_seen {
                    direction = if value.eq_ignore_ascii_case("U") {
                        InteractiveIsocurveDirection::U
                    } else if value.eq_ignore_ascii_case("V") {
                        InteractiveIsocurveDirection::V
                    } else if value.eq_ignore_ascii_case("Both") {
                        InteractiveIsocurveDirection::Both
                    } else {
                        return false;
                    };
                    direction_seen = true;
                } else if name.eq_ignore_ascii_case("IgnoreTrims") && !ignore_trims_seen {
                    ignore_trims = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    ignore_trims_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ExtractIsocurve {
                direction,
                ignore_trims,
            }
        } else if matches!(normalized.as_str(), "polygon" | "poly") {
            let side_count = match arguments.as_slice() {
                [] => 4,
                [text] => match text.parse::<usize>() {
                    Ok(side_count) if (3..=MAX_REGULAR_POLYGON_SIDES).contains(&side_count) => {
                        side_count
                    }
                    _ => return false,
                },
                _ => return false,
            };
            InteractiveCommand::Polygon {
                side_count,
                center: None,
            }
        } else if matches!(normalized.as_str(), "array" | "arrayrectangular") {
            if arguments.len() < 2 {
                return false;
            }
            let Ok(x_count) = arguments[0].parse::<usize>() else {
                return false;
            };
            let Ok(y_count) = arguments[1].parse::<usize>() else {
                return false;
            };
            if x_count == 0 || y_count == 0 {
                return false;
            }
            let mut z_count = 1;
            let mut remaining = &arguments[2..];
            if let Some(z_count_text) = remaining.first()
                && !z_count_text.contains('=')
            {
                let Ok(parsed) = z_count_text.parse::<usize>() else {
                    return false;
                };
                if parsed == 0 {
                    return false;
                }
                z_count = parsed;
                remaining = &remaining[1..];
            }
            let mut fill = false;
            let mut z_distance = 0.0;
            let mut mode_seen = false;
            let mut z_distance_seen = false;
            for option in remaining {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches('_');
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Mode") && !mode_seen {
                    if value.eq_ignore_ascii_case("UnitCell") {
                        fill = false;
                    } else if value.eq_ignore_ascii_case("Fill") {
                        fill = true;
                    } else {
                        return false;
                    }
                    mode_seen = true;
                } else if name.eq_ignore_ascii_case("ZDistance") && !z_distance_seen {
                    let Ok(distance) = value.parse::<f64>() else {
                        return false;
                    };
                    if !distance.is_finite() {
                        return false;
                    }
                    z_distance = distance;
                    z_distance_seen = true;
                } else {
                    return false;
                }
            }
            if z_count > 1 && !z_distance_seen {
                return false;
            }
            InteractiveCommand::Array {
                counts: [x_count, y_count, z_count],
                fill,
                z_distance,
                start: None,
            }
        } else if normalized == "distribute" {
            let options = if arguments
                .first()
                .is_some_and(|s| s.trim_start_matches('_').eq_ignore_ascii_case("Direction"))
            {
                &arguments[1..]
            } else {
                arguments.as_slice()
            };
            let Ok(settings) = DistributionSettings::parse(options) else {
                return false;
            };
            InteractiveCommand::Distribute {
                settings,
                start: None,
            }
        } else if normalized == "arraylinear" {
            let [item_count] = arguments.as_slice() else {
                return false;
            };
            let Ok(item_count) = item_count.parse::<usize>() else {
                return false;
            };
            if item_count < 2 {
                return false;
            }
            InteractiveCommand::ArrayLinear {
                item_count,
                start: None,
            }
        } else if normalized == "arraypolar" {
            let Some(item_count_text) = arguments.first() else {
                return false;
            };
            let Ok(item_count) = item_count_text.parse::<usize>() else {
                return false;
            };
            if item_count < 2 {
                return false;
            }
            let mut fill_angle_degrees = 360.0;
            let mut rotate = true;
            let mut z_offset = 0.0;
            let mut rotate_seen = false;
            let mut z_offset_seen = false;
            let mut remaining = &arguments[1..];
            if let Some(angle_text) = remaining.first()
                && !angle_text.contains('=')
            {
                let Ok(angle) = angle_text.parse::<f64>() else {
                    return false;
                };
                if !angle.is_finite() || angle == 0.0 {
                    return false;
                }
                fill_angle_degrees = angle;
                remaining = &remaining[1..];
            }
            for option in remaining {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches('_');
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Rotate") && !rotate_seen {
                    if value.eq_ignore_ascii_case("Yes") {
                        rotate = true;
                    } else if value.eq_ignore_ascii_case("No") {
                        rotate = false;
                    } else {
                        return false;
                    }
                    rotate_seen = true;
                } else if name.eq_ignore_ascii_case("ZOffset") && !z_offset_seen {
                    let Ok(offset) = value.parse::<f64>() else {
                        return false;
                    };
                    if !offset.is_finite() {
                        return false;
                    }
                    z_offset = offset;
                    z_offset_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ArrayPolar {
                item_count,
                fill_angle_degrees,
                rotate,
                z_offset,
            }
        } else if matches!(
            normalized.as_str(),
            "selvolumesphere" | "selvolumepipe" | "selvolumeobject" | "selbox"
        ) {
            let mode = match arguments.as_slice() {
                [] => RectSelectionMode::Crossing,
                [option] => {
                    let Some((name, value)) = option.split_once('=') else {
                        return false;
                    };
                    if !name
                        .trim_start_matches('_')
                        .eq_ignore_ascii_case("SelectionMode")
                    {
                        return false;
                    }
                    let Some(mode) = RectSelectionMode::parse(value.trim_start_matches('_')) else {
                        return false;
                    };
                    mode
                }
                _ => return false,
            };
            if normalized == "selbox" {
                InteractiveCommand::SelBox {
                    base: None,
                    opposite: None,
                    mode,
                }
            } else if normalized == "selvolumepipe" {
                let selected = self.document.selected_object_ids().collect::<Vec<_>>();
                let source = match selected.as_slice() {
                    [id] if self
                        .document
                        .object(*id)
                        .and_then(|object| object.geometry().curve_ref())
                        .is_some() =>
                    {
                        Some(*id)
                    }
                    _ => None,
                };
                InteractiveCommand::SelVolumePipe { source, mode }
            } else if normalized == "selvolumeobject" {
                InteractiveCommand::SelVolumeObject { mode }
            } else {
                InteractiveCommand::SelVolumeSphere { center: None, mode }
            }
        } else if matches!(normalized.as_str(), "circle" | "c")
            && arguments.len() == 1
            && arguments[0]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Vertical")
        {
            InteractiveCommand::CircleVertical {
                center: None,
                radius: None,
                mode: CircleSizeMode::Radius,
            }
        } else if matches!(normalized.as_str(), "arc" | "a")
            && arguments.len() == 1
            && arguments[0]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Center")
        {
            InteractiveCommand::ArcCenter {
                center: None,
                start: None,
                clockwise: true,
            }
        } else if matches!(normalized.as_str(), "arc" | "a")
            && arguments.len() == 2
            && arguments[0]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Center")
            && arguments[1]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Midpoint")
        {
            InteractiveCommand::ArcCenterMidpoint {
                center: None,
                midpoint: None,
                clockwise: false,
            }
        } else if matches!(normalized.as_str(), "arc" | "a")
            && arguments.len() == 1
            && arguments[0]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("StartPoint")
        {
            InteractiveCommand::ArcStartPoint {
                start: None,
                end: None,
            }
        } else if matches!(normalized.as_str(), "arc" | "a")
            && arguments.len() == 2
            && arguments[0]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("StartPoint")
            && arguments[1]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("ThroughPoint")
        {
            InteractiveCommand::Arc { points: [None; 2] }
        } else if matches!(normalized.as_str(), "arc" | "a")
            && arguments.len() == 2
            && arguments[0]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("StartPoint")
            && arguments[1]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Center")
        {
            InteractiveCommand::ArcStartCenter {
                start: None,
                center: None,
                clockwise: true,
            }
        } else if matches!(normalized.as_str(), "arc" | "a")
            && arguments.len() == 2
            && arguments[0]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("StartPoint")
            && arguments[1]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Direction")
        {
            InteractiveCommand::ArcStartDirection {
                start: None,
                tangent: None,
            }
        } else {
            if !arguments.is_empty()
                && (!transform_prompt::supports_name(&normalized)
                    || normalized == "rotate3d"
                    || transform_prompt::start_copy_option(&normalized, &arguments, false)
                        .is_none())
                && translation_options.is_none()
            {
                return false;
            }
            match normalized.as_str() {
                "angle" => InteractiveCommand::Angle { points: [None; 3] },
                "point" | "pt" => InteractiveCommand::Point,
                "points" => InteractiveCommand::Points,
                "line" | "l" => InteractiveCommand::Line { start: None },
                "circle" | "c" => InteractiveCommand::Circle { center: None },
                "sphere" | "sph" => InteractiveCommand::Sphere { center: None },
                "ellipsoid" => InteractiveCommand::Ellipsoid { points: [None; 3] },
                "arc" | "a" => InteractiveCommand::ArcCenter {
                    center: None,
                    start: None,
                    clockwise: true,
                },
                "polyline" | "pline" => InteractiveCommand::Polyline,
                "box" => InteractiveCommand::Box {
                    base: None,
                    opposite: None,
                },
                "srfpt" | "surfacefromcorners" => InteractiveCommand::SrfPt { corners: [None; 3] },
                "crvseam" => InteractiveCommand::CrvSeam,
                "split" => InteractiveCommand::SplitCurve,
                "trim" => InteractiveCommand::TrimCurve,
                "move" | "m" => InteractiveCommand::Move { start: None },
                "copy" => InteractiveCommand::Copy { start: None },
                "scale" => InteractiveCommand::Scale {
                    kind: InteractiveScaleKind::Uniform,
                    center: None,
                    reference: None,
                },
                "scale1d" => InteractiveCommand::Scale {
                    kind: InteractiveScaleKind::OneDimensional,
                    center: None,
                    reference: None,
                },
                "scale2d" => InteractiveCommand::Scale {
                    kind: InteractiveScaleKind::TwoDimensional,
                    center: None,
                    reference: None,
                },
                "scalebyplane" => {
                    use viboceros_command::scale_by_plane::{Options, Prompt, start_options};
                    let options = start_options(
                        &arguments,
                        Options {
                            rigid: self.commands.rigid_option_default("ScaleByPlane").unwrap(),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                    InteractiveCommand::ScaleByPlane(Prompt::new(
                        options,
                        self.viewports[self.active_viewport].construction_plane(),
                    ))
                }
                "scalenu" => {
                    use viboceros_command::nonuniform_scale::{
                        ScaleNuOptions, ScaleNuPrompt, start_options,
                    };
                    let options = start_options(
                        &arguments,
                        ScaleNuOptions {
                            rigid: self.commands.rigid_option_default("ScaleNU").unwrap(),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                    InteractiveCommand::ScaleNu(ScaleNuPrompt {
                        rigid: options.rigid,
                        ..ScaleNuPrompt::new(options.world)
                    })
                }
                "scalepositions" => {
                    use viboceros_command::scale_positions::{
                        PositionOptions, ScalePositionsPrompt, start_options,
                    };
                    let mode = self.commands.scale_mode_default("ScalePositions").unwrap();
                    let options =
                        start_options(&arguments, PositionOptions { mode, copy: false }).unwrap();
                    InteractiveCommand::ScalePositions(ScalePositionsPrompt {
                        mode: options.mode,
                        ..ScalePositionsPrompt::new(mode)
                    })
                }
                "rotate" => InteractiveCommand::Rotate {
                    center: None,
                    reference: None,
                },
                "rotate3d" => InteractiveCommand::Rotate3D { points: [None; 3] },
                "mirror" => InteractiveCommand::Mirror { start: None },
                "shear" => InteractiveCommand::Shear {
                    origin: None,
                    reference: None,
                },
                _ => return false,
            }
        };

        let subcurve_edge = if matches!(command, InteractiveCommand::SubCrv { delete: false, .. }) {
            let picks = match self.component_selection.checked_picks(&self.document) {
                Ok(picks) => picks,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
            };
            if !picks.is_empty()
                && (picks.len() != 1
                    || picks[0].kind != viboceros_command::ComponentSelectionKind::BrepEdge
                    || self
                        .document
                        .selected_objects()
                        .any(|o| o.geometry().curve_ref().is_some()))
            {
                self.push_log("Select one curve or one surface edge".into());
                return true;
            }
            match picks.as_slice() {
                [pick] if pick.kind == viboceros_command::ComponentSelectionKind::BrepEdge => {
                    Some(crate::viewport::EdgePick {
                        object: pick.object,
                        edge: pick.index,
                    })
                }
                _ => None,
            }
        } else {
            None
        };
        self.cancel_interactive_command(true);
        self.push_log(format!("> {input}"));
        if let InteractiveCommand::SubCrv { copy, delete, .. } = command {
            if delete {
                self.component_selection.clear();
                self.begin_subcurve_prompt(
                    false,
                    viboceros_command::subcurve_input::SubcurveMode::Shorten,
                    false,
                    true,
                );
                return true;
            }
            let mode = input
                .split_whitespace()
                .skip(1)
                .find_map(|option| {
                    option
                        .split_once('=')
                        .filter(|(name, _)| {
                            name.trim_start_matches(['_', '-'])
                                .eq_ignore_ascii_case("Mode")
                        })
                        .and_then(|(_, value)| {
                            viboceros_command::subcurve_input::SubcurveMode::parse(value)
                        })
                })
                .unwrap_or(self.commands.subcurve_defaults().mode);
            let from_midpoint = input
                .split_whitespace()
                .skip(1)
                .filter_map(|o| o.split_once('='))
                .find(|(name, _)| {
                    name.trim_start_matches(['_', '-'])
                        .eq_ignore_ascii_case("FromMidpoint")
                })
                .map(|(_, value)| value.trim_start_matches('_').eq_ignore_ascii_case("Yes"))
                .unwrap_or(self.commands.subcurve_defaults().from_midpoint);
            self.commands
                .set_subcurve_options(Some(copy), Some(mode), Some(from_midpoint));
            self.begin_subcurve_prompt(copy, mode, from_midpoint, false);
            if let Some(pick) = subcurve_edge {
                self.subcurve_prompt.as_mut().unwrap().source = None;
                self.pick_subcurve_edge(pick);
            }
            return true;
        }
        if let InteractiveCommand::ExtractSrf {
            copy,
            output_on_current_layer,
        } = command
        {
            self.start_extract_faces(copy, output_on_current_layer);
            return true;
        }
        if picked_sources.is_none() {
            self.commands.begin_copy_options(command.name());
        }
        if (transform_prompt::supports(command)
            || translation_prompt::supports(command)
            || matches!(command, InteractiveCommand::SetPoint { .. })
            || matches!(
                command,
                InteractiveCommand::Twist { .. }
                    | InteractiveCommand::Bend { .. }
                    | InteractiveCommand::Stretch { .. }
                    | InteractiveCommand::Taper { .. }
                    | InteractiveCommand::Maelstrom { .. }
            ))
            && self.document.selected_object_count() == 0
            && !(transform_sources::allows_grips(command.name())
                && self.document.selected_control_points().next().is_some())
        {
            self.start_transform_source_prompt(command.name());
            if matches!(command, InteractiveCommand::SetPoint { .. }) {
                self.object_prompt.as_mut().unwrap().command_override = Some(input.to_owned());
            }
            if has_start_arguments {
                self.push_log("Select objects first; Enter continues to transform options".into());
            }
            return true;
        }
        if matches!(
            command,
            InteractiveCommand::Move { .. }
                | InteractiveCommand::SetPoint { .. }
                | InteractiveCommand::Copy { .. }
                | InteractiveCommand::Array { .. }
                | InteractiveCommand::ArrayLinear { .. }
                | InteractiveCommand::Distribute { .. }
                | InteractiveCommand::ArrayPolar { .. }
                | InteractiveCommand::Scale { .. }
                | InteractiveCommand::ScaleByPlane(_)
                | InteractiveCommand::ScaleNu(_)
                | InteractiveCommand::ScalePositions(_)
                | InteractiveCommand::Rotate { .. }
                | InteractiveCommand::Rotate3D { .. }
                | InteractiveCommand::Twist { .. }
                | InteractiveCommand::Bend { .. }
                | InteractiveCommand::Stretch { .. }
                | InteractiveCommand::Taper { .. }
                | InteractiveCommand::Maelstrom { .. }
                | InteractiveCommand::Mirror { .. }
                | InteractiveCommand::MirrorThreePoint { .. }
                | InteractiveCommand::MirrorObject
                | InteractiveCommand::Shear { .. }
                | InteractiveCommand::ExtrudeCurve { .. }
                | InteractiveCommand::ExtrudeCurveTapered { .. }
                | InteractiveCommand::ExtrudeSurface { .. }
                | InteractiveCommand::ExtrudeSurfaceTapered { .. }
                | InteractiveCommand::ExtrudeSurfaceToPoint { .. }
                | InteractiveCommand::ExtrudeCurveToPoint { .. }
                | InteractiveCommand::ExtractSrf { .. }
                | InteractiveCommand::Curvature { .. }
                | InteractiveCommand::DupFaceBorder { .. }
                | InteractiveCommand::DupEdge { .. }
                | InteractiveCommand::ExtractMeshFaces { .. }
                | InteractiveCommand::ExtractConnectedMeshFaces { .. }
                | InteractiveCommand::ExtractMeshPart { .. }
                | InteractiveCommand::DeleteFaces
                | InteractiveCommand::SwapMeshEdge
                | InteractiveCommand::CollapseMeshEdge
                | InteractiveCommand::SplitMeshEdge { .. }
                | InteractiveCommand::FillMeshHole { .. }
                | InteractiveCommand::WeldEdge
                | InteractiveCommand::WeldVertices
                | InteractiveCommand::UnweldEdge { .. }
                | InteractiveCommand::UnweldVertex { .. }
                | InteractiveCommand::DupMeshEdge { .. }
                | InteractiveCommand::DupMeshHoleBoundary
                | InteractiveCommand::ExtractIsocurve { .. }
                | InteractiveCommand::CrvSeam
                | InteractiveCommand::SrfSeam { .. }
                | InteractiveCommand::Extend { .. }
                | InteractiveCommand::ExtendSrf { .. }
                | InteractiveCommand::SubCrv { .. }
                | InteractiveCommand::SplitCurve
                | InteractiveCommand::SplitCurveWithCutters
                | InteractiveCommand::SplitSurfaceIsocurve { .. }
                | InteractiveCommand::TrimCurve
                | InteractiveCommand::Revolve { .. }
        ) && self.document.selected_object_count() == 0
            && !(transform_sources::allows_grips(command.name())
                && self.document.selected_control_points().next().is_some())
        {
            self.push_log("Error: no objects are selected".to_owned());
            return true;
        }
        if matches!(command, InteractiveCommand::Distribute { .. }) {
            let count = viboceros_command::distribution_unit_count(&self.document);
            if count < 3 {
                self.push_log(format!(
                    "Error: Distribute requires at least three objects or groups; found {count}"
                ));
                return true;
            }
        }
        if transform_prompt::supports(command) {
            if !self.start_transform_session(
                command,
                transform_copy.flatten().unwrap(),
                picked_sources,
            ) {
                return true;
            }
        } else if matches!(command, InteractiveCommand::Twist { .. }) {
            if !self.start_twist_session(picked_sources) {
                return true;
            }
        } else if matches!(command, InteractiveCommand::Bend { .. }) {
            if !self.start_bend_session(picked_sources) {
                return true;
            }
        } else if matches!(command, InteractiveCommand::Maelstrom { .. }) {
            if !self.start_maelstrom_session(picked_sources) {
                return true;
            }
        } else if matches!(command, InteractiveCommand::Stretch { .. }) {
            if !self.start_stretch_session(picked_sources) {
                return true;
            }
        } else if matches!(command, InteractiveCommand::Taper { .. }) {
            if !self.start_taper_session(picked_sources) {
                return true;
            }
        } else if matches!(command, InteractiveCommand::SetPoint { .. }) {
            self.start_set_point_sources(picked_sources);
        } else if translation_prompt::supports(command)
            && !self.start_translation_session(
                command,
                picked_sources,
                translation_options.unwrap().0,
            )
        {
            return true;
        }
        if let InteractiveCommand::EvaluateUv { options } = command {
            self.push_log(options.command_line());
        }
        if let InteractiveCommand::SetPoint { options } = command {
            self.push_log(format!("SetPt options: {}", options.command_options()));
        }
        self.push_log(command.prompt().to_owned());
        self.point_filter = None;
        self.point_constraint = None;
        self.active_command = Some(command);
        if translation_normal {
            self.start_move_normal();
        }
        if translation_options.is_some_and(|(_, in_place)| in_place) {
            self.try_continue_translation("InPlace");
        }
        if let Some(token) =
            mirror_mode.and_then(viboceros_command::mirror::MirrorPlaneOption::token)
        {
            self.try_continue_mirror_option(token);
        }
        true
    }
}
