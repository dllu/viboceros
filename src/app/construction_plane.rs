//! A small, separate prompt stack for transparent construction-plane editing.
//! Accepted model points, the model's latched plane, and model undo stay intact.
use super::*;
use viboceros_command::construction_plane::{
    self as cplane, PlaneAction, PlaneCommandError, PlanePromptKind,
};
use viboceros_drafting::PointInput;

#[derive(Clone, Debug)]
pub(super) struct PlanePrompt {
    kind: PlanePromptKind,
    pub(super) viewport: usize,
    frame: Frame3,
    pub(super) points: Vec<Point3>,
    previous: Option<Point3>,
    surface_target: Option<(viboceros_document::ObjectId, Option<usize>)>,
    surface_frame: Option<Frame3>,
    surface_flip: bool,
}

impl PlanePrompt {
    pub(super) fn requests_point(&self) -> bool {
        !matches!(
            self.kind,
            PlanePromptKind::Object | PlanePromptKind::SurfaceSelect
        )
    }
    pub(super) fn requests_object(&self) -> bool {
        matches!(
            self.kind,
            PlanePromptKind::Object | PlanePromptKind::SurfaceSelect
        )
    }
    pub(super) fn requests_surface(&self) -> bool {
        self.kind == PlanePromptKind::SurfaceSelect
    }
    pub(super) fn anchor(&self) -> Option<Point3> {
        self.points.first().copied().or_else(|| {
            (self.kind == PlanePromptKind::SurfaceX)
                .then(|| self.surface_frame.map(Frame3::origin))
                .flatten()
        })
    }
    fn message(&self) -> &'static str {
        match (self.kind, self.points.len()) {
            (PlanePromptKind::Origin, _) => {
                "CPlane: pick a new origin (Enter keeps the current origin)"
            }
            (PlanePromptKind::AllOrigin, _) => "CPlane All: pick the new origin for every viewport",
            (PlanePromptKind::ThreePoint, 0) => {
                "CPlane 3Point: pick the origin (Enter keeps the current origin)"
            }
            (PlanePromptKind::ThreePoint, 1) => {
                "CPlane 3Point: pick a point on the positive X axis"
            }
            (PlanePromptKind::ThreePoint, _) => {
                "CPlane 3Point: pick a point in the positive XY half-plane"
            }
            (PlanePromptKind::ThreePointVertical, _) => {
                "CPlane 3Point Vertical: pick the positive X direction"
            }
            (PlanePromptKind::ThreePointZAxis, _) => {
                "CPlane 3Point ZAxis: pick the positive Z direction"
            }
            (PlanePromptKind::Elevation, _) => {
                "CPlane Elevation: type an offset distance or pick a height point"
            }
            (PlanePromptKind::Through, _) => {
                "CPlane Through: pick a point for the plane to pass through"
            }
            (PlanePromptKind::ThroughAll, _) => {
                "CPlane Through All: pick a point for every plane to pass through"
            }
            (PlanePromptKind::Rotate, 0) => "CPlane Rotate: pick the rotation axis start",
            (PlanePromptKind::Rotate, 1) => "CPlane Rotate: pick the rotation axis end",
            (PlanePromptKind::Rotate, 2) => {
                "CPlane Rotate: type an angle or pick the first reference point"
            }
            (PlanePromptKind::Rotate, _) => "CPlane Rotate: pick the second reference point",
            (PlanePromptKind::Object, _) => {
                "CPlane Object: select a curve, surface, mesh face, or polysurface face"
            }
            (PlanePromptKind::SurfaceSelect, _) => {
                "CPlane Surface: select a surface or polysurface face"
            }
            (PlanePromptKind::SurfaceOrigin, _) => {
                "CPlane Surface: pick an origin (Enter uses UV midpoint; Flip=Yes|No)"
            }
            (PlanePromptKind::SurfaceX, _) => {
                "CPlane Surface: pick an X direction (Enter uses surface U)"
            }
        }
    }
}

impl VibocerosApp {
    pub(super) fn try_run_synchronize_cplanes_command(&mut self, input: &str) -> bool {
        let (command, argument) = input.split_once(char::is_whitespace).unwrap_or((input, ""));
        if !command
            .trim_start_matches(['\'', '_', '-'])
            .eq_ignore_ascii_case("SynchronizeCPlanes")
        {
            return false;
        }
        self.push_log(format!("> {input}"));
        let result = (|| -> Result<String, String> {
            let mut set_view = true;
            let mut source_words = Vec::new();
            for word in argument.split_whitespace() {
                if let Some(value) = word.strip_prefix("SetView=").or_else(|| {
                    word.get(..8)
                        .filter(|prefix| prefix.eq_ignore_ascii_case("SetView="))
                        .map(|_| &word[8..])
                }) {
                    set_view = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return Err("SetView must be Yes or No".into());
                    };
                } else {
                    source_words.push(word);
                }
            }
            let source_name = source_words.join(" ");
            let source_name = source_name.trim_matches('"');
            let source_index = if source_name.is_empty() {
                self.active_viewport
            } else {
                self.resolve_viewport_reference(source_name)?
            };
            let source = self.viewports[source_index].construction_plane();
            let mut updated = 0;
            for viewport in &mut self.viewports {
                if let Some((named_role, plane_role)) = viewport.synchronization_plane_role() {
                    viewport.synchronize_cplane(source, named_role, plane_role, set_view);
                    updated += 1;
                }
            }
            Ok(format!(
                "Synchronized {updated} standard viewports from viewport {} ({})",
                source_index + 1,
                self.viewports[source_index].view_label()
            ))
        })();
        match result {
            Ok(message) => {
                self.push_log(message);
                self.command_input.clear();
            }
            Err(message) => self.push_log(format!("Error: {message}")),
        }
        true
    }

    pub(super) fn try_run_copy_cplane_command(&mut self, input: &str) -> bool {
        let (command, argument) = input.split_once(char::is_whitespace).unwrap_or((input, ""));
        let command = command.trim_start_matches(['\'', '_', '-']);
        let copy_plane = command.eq_ignore_ascii_case("CopyCPlaneToAll");
        if !copy_plane && !command.eq_ignore_ascii_case("CopyCPlaneSettingsToAll") {
            return false;
        }
        self.push_log(format!("> {input}"));
        let result = (|| -> Result<String, String> {
            let argument = argument.trim();
            let source = if argument.is_empty() {
                self.active_viewport
            } else {
                let argument = if argument.starts_with('"')
                    && argument.ends_with('"')
                    && argument.len() >= 2
                {
                    &argument[1..argument.len() - 1]
                } else {
                    argument
                };
                self.resolve_viewport_reference(argument)?
            };
            if copy_plane {
                let frame = self.viewports[source].construction_plane();
                for (index, viewport) in self.viewports.iter_mut().enumerate() {
                    if index != source {
                        viewport.plane.set(frame);
                    }
                }
            } else {
                let grid = self.viewports[source].grid_settings();
                for (index, viewport) in self.viewports.iter_mut().enumerate() {
                    if index != source {
                        viewport.set_grid_settings(grid);
                    }
                }
            }
            Ok(format!(
                "Copied {} from viewport {} ({}) to all viewports",
                if copy_plane {
                    "construction plane"
                } else {
                    "grid and snap settings"
                },
                source + 1,
                self.viewports[source].view_label()
            ))
        })();
        match result {
            Ok(message) => {
                self.push_log(message);
                self.command_input.clear();
            }
            Err(message) => self.push_log(format!("Error: {message}")),
        }
        true
    }

    pub(super) fn handle_plane_shortcuts(&mut self, ui: &mut egui::Ui) {
        // Shift+Home/End select text in editors. Keep those editing operations
        // intact; CPlane Undo/Redo can also be entered at any command prompt.
        if ui.ctx().text_edit_focused() {
            return;
        }
        let actions = ui.input_mut(|input| {
            let mut actions = Vec::new();
            input.events.retain(|event| {
                if let egui::Event::Key {
                    key,
                    modifiers,
                    pressed,
                    repeat,
                    ..
                } = event
                    && modifiers.matches_exact(egui::Modifiers::SHIFT)
                    && matches!(key, egui::Key::Home | egui::Key::End)
                {
                    if *pressed && !*repeat {
                        actions.push(if *key == egui::Key::Home {
                            PlaneAction::Undo
                        } else {
                            PlaneAction::Redo
                        });
                    }
                    false
                } else {
                    true
                }
            });
            actions
        });
        for action in actions {
            self.apply_plane_action(action, self.active_viewport);
        }
    }

    pub(super) fn try_run_plane_command(&mut self, input: &str) -> bool {
        let frame = self.viewports[self.active_viewport].construction_plane();
        let Some(parsed) = cplane::parse_with_options(
            input,
            frame,
            self.last_point,
            self.document.tolerance(),
            self.cplane_options,
        ) else {
            return false;
        };
        self.cplane_options = parsed.options;
        self.push_log(format!("> {input}"));
        match parsed.action {
            Ok(action) => {
                self.plane_prompt = None;
                if self.apply_plane_action(action, self.active_viewport) {
                    self.command_input.clear();
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn apply_plane_action(&mut self, action: PlaneAction, viewport: usize) -> bool {
        if let PlaneAction::Prompt(kind) = action {
            if kind == PlanePromptKind::Object {
                let selected = self.document.selected_object_ids().collect::<Vec<_>>();
                if let [id] = selected.as_slice() {
                    let needs_face = self.document.object(*id).is_some_and(|object| {
                        matches!(object.geometry(), Geometry::Mesh(_))
                            || matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() > 1)
                    });
                    if !needs_face {
                        return self.apply_plane_action(PlaneAction::Object(*id), viewport);
                    }
                }
            }
            if kind == PlanePromptKind::SurfaceSelect {
                let selected = self.document.selected_object_ids().collect::<Vec<_>>();
                if let [id] = selected.as_slice() {
                    let can_start = self.document.object(*id).is_some_and(|object| {
                        matches!(object.geometry(), Geometry::NurbsSurface(_))
                            || matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() == 1)
                    });
                    if can_start {
                        return self.begin_surface_prompt(*id, None, None, false, viewport);
                    }
                }
            }
            self.snaps.plane_override = None;
            let prompt = PlanePrompt {
                kind,
                viewport,
                frame: self.viewports[viewport].construction_plane(),
                points: Vec::new(),
                previous: self.last_point,
                surface_target: None,
                surface_frame: None,
                surface_flip: false,
            };
            self.push_log(prompt.message().into());
            self.plane_prompt = Some(prompt);
            return true;
        }
        if let PlaneAction::SetAllOrigin(point) | PlaneAction::SetThroughAll(point) = action {
            let frames = self
                .viewports
                .iter()
                .map(|view| {
                    let frame = view.construction_plane();
                    if matches!(action, PlaneAction::SetAllOrigin(_)) {
                        Ok(frame.with_origin(point))
                    } else {
                        cplane::through(frame, point)
                    }
                })
                .collect::<Result<Vec<_>, _>>();
            let frames = match frames {
                Ok(frames) => frames,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
            };
            for (view, frame) in self.viewports.iter_mut().zip(frames) {
                view.plane.set(frame);
            }
            self.push_log(format!(
                "Updated construction planes in {} viewports",
                self.viewports.len()
            ));
            return true;
        }
        let action = match action {
            PlaneAction::AlignToView => self.viewports[viewport]
                .construction_plane_aligned_to_view()
                .map(PlaneAction::Set)
                .map_err(|error| error.to_string()),
            PlaneAction::Object(id) => self
                .document
                .object(id)
                .filter(|_| self.document.is_object_selectable(id))
                .ok_or_else(|| format!("object {id} is missing or cannot be selected"))
                .and_then(|object| {
                    cplane::frame_from_object(object.geometry(), self.document.tolerance())
                        .map(PlaneAction::Set)
                        .map_err(|error| error.to_string())
                }),
            PlaneAction::ObjectFace(id, face) => self
                .document
                .object(id)
                .filter(|_| self.document.is_object_selectable(id))
                .ok_or_else(|| format!("object {id} is missing or cannot be selected"))
                .and_then(|object| match object.geometry() {
                    Geometry::Mesh(mesh) => {
                        cplane::frame_from_mesh_face(mesh, face, self.document.tolerance())
                            .map(PlaneAction::Set)
                            .map_err(|error| error.to_string())
                    }
                    Geometry::Brep(brep) => {
                        cplane::frame_from_brep_face(brep, face, self.document.tolerance())
                            .map(PlaneAction::Set)
                            .map_err(|error| error.to_string())
                    }
                    _ => Err("Face=index requires a mesh or polysurface object".into()),
                }),
            PlaneAction::Surface {
                id,
                face,
                origin,
                x_point,
                flip,
            } => {
                if x_point.is_none() {
                    return self.begin_surface_prompt(id, face, origin, flip, viewport);
                }
                self.surface_target_frame(id, face, origin, x_point, flip)
                    .map(PlaneAction::Set)
            }
            other => Ok(other),
        };
        let action = match action {
            Ok(action) => action,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        let state = &mut self.viewports[viewport].plane;
        let changed = match action {
            PlaneAction::Set(frame) => state.set(frame),
            PlaneAction::Undo => state.undo(),
            PlaneAction::Redo => state.redo(),
            PlaneAction::Prompt(_) => unreachable!(),
            PlaneAction::SetAllOrigin(_) | PlaneAction::SetThroughAll(_) => unreachable!(),
            PlaneAction::AlignToView => unreachable!(),
            PlaneAction::Object(_) => unreachable!(),
            PlaneAction::ObjectFace(_, _) => unreachable!(),
            PlaneAction::Surface { .. } => unreachable!(),
        };
        self.push_log(
            if changed {
                "Construction plane updated"
            } else {
                "Construction plane unchanged"
            }
            .into(),
        );
        true
    }

    pub(super) fn cancel_plane_prompt(&mut self) {
        self.snaps.plane_override = None;
        if self.plane_prompt.take().is_some() {
            self.command_input.clear();
            self.push_log("CPlane cancelled; previous modeling prompt retained".into());
        }
    }

    fn surface_target_frame(
        &self,
        id: viboceros_document::ObjectId,
        face_index: Option<usize>,
        origin: Option<Point3>,
        x_point: Option<Point3>,
        flip: bool,
    ) -> Result<Frame3, String> {
        let object = self
            .document
            .object(id)
            .filter(|_| self.document.is_object_selectable(id))
            .ok_or_else(|| format!("object {id} is missing or cannot be selected"))?;
        let (surface, reversed) = match object.geometry() {
            Geometry::NurbsSurface(surface) if face_index.is_none_or(|face| face == 0) => {
                (surface, false)
            }
            Geometry::Brep(brep) => {
                let index = match (face_index, brep.faces().len()) {
                    (Some(index), _) => index,
                    (None, 1) => 0,
                    (None, _) => {
                        return Err(
                            "a multi-face polysurface requires Face=index or a face pick".into(),
                        );
                    }
                };
                let face = brep.faces().get(index).ok_or_else(|| {
                    format!(
                        "face {index} is outside the {} B-rep faces",
                        brep.faces().len()
                    )
                })?;
                (face.surface(), face.is_reversed())
            }
            _ => return Err("CPlane Surface requires a surface or polysurface face".into()),
        };
        cplane::surface_frame_with_flip(
            surface,
            reversed,
            origin,
            x_point,
            flip,
            self.document.tolerance(),
        )
        .map_err(|error| error.to_string())
    }

    fn begin_surface_prompt(
        &mut self,
        id: viboceros_document::ObjectId,
        face: Option<usize>,
        origin: Option<Point3>,
        flip: bool,
        viewport: usize,
    ) -> bool {
        let frame = match self.surface_target_frame(id, face, origin, None, flip) {
            Ok(frame) => frame,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        self.snaps.plane_override = None;
        let prompt = PlanePrompt {
            kind: if origin.is_some() {
                PlanePromptKind::SurfaceX
            } else {
                PlanePromptKind::SurfaceOrigin
            },
            viewport,
            frame: self.viewports[viewport].construction_plane(),
            points: Vec::new(),
            previous: origin.or(self.last_point),
            surface_target: Some((id, face)),
            surface_frame: Some(frame),
            surface_flip: flip,
        };
        self.push_log(prompt.message().into());
        self.plane_prompt = Some(prompt);
        true
    }

    pub(super) fn try_continue_plane_prompt(&mut self, input: &str) -> bool {
        let Some(prompt) = &self.plane_prompt else {
            return false;
        };
        if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
            && !(prompt.kind == PlanePromptKind::SurfaceOrigin
                && input.trim_start_matches('_').eq_ignore_ascii_case("Flip"))
        {
            self.cancel_plane_prompt();
            return false;
        }
        if prompt.requests_object() {
            let surface = prompt.requests_surface();
            let command = format!(
                "CPlane {} {input}",
                if surface { "Surface" } else { "Object" }
            );
            match cplane::parse(&command, prompt.frame, None, self.document.tolerance()) {
                Some(Ok(PlaneAction::Surface {
                    id,
                    face,
                    origin,
                    x_point,
                    flip,
                })) if surface => {
                    if let Some(x_point) = x_point {
                        match self.surface_target_frame(id, face, origin, Some(x_point), flip) {
                            Ok(frame) => {
                                let viewport = prompt.viewport;
                                self.plane_prompt = None;
                                self.apply_plane_action(PlaneAction::Set(frame), viewport);
                                self.command_input.clear();
                            }
                            Err(error) => self.push_log(format!("Error: {error}")),
                        }
                    } else {
                        self.begin_surface_prompt(id, face, origin, flip, prompt.viewport);
                    }
                }
                Some(Ok(PlaneAction::Object(id))) => {
                    self.accept_plane_prompt_object(id);
                }
                Some(Ok(PlaneAction::ObjectFace(id, face))) => {
                    self.accept_plane_prompt_object_face(id, face);
                }
                _ => self.push_log(
                    "Error: enter an object ID, object ID with Face=index, or click an object or face"
                        .into(),
                ),
            }
            return true;
        }
        if matches!(
            prompt.kind,
            PlanePromptKind::Origin | PlanePromptKind::AllOrigin
        ) && input.trim_start_matches('_').eq_ignore_ascii_case("View")
        {
            let viewport = prompt.viewport;
            self.plane_prompt = None;
            self.apply_plane_action(PlaneAction::AlignToView, viewport);
            self.command_input.clear();
            return true;
        }
        if matches!(
            prompt.kind,
            PlanePromptKind::Origin | PlanePromptKind::AllOrigin
        ) && input.trim_start_matches('_').eq_ignore_ascii_case("Object")
        {
            let viewport = prompt.viewport;
            self.plane_prompt = None;
            self.apply_plane_action(PlaneAction::Prompt(PlanePromptKind::Object), viewport);
            self.command_input.clear();
            return true;
        }
        if matches!(
            prompt.kind,
            PlanePromptKind::Origin | PlanePromptKind::AllOrigin
        ) && input
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Surface")
        {
            let viewport = prompt.viewport;
            self.plane_prompt = None;
            self.apply_plane_action(
                PlaneAction::Prompt(PlanePromptKind::SurfaceSelect),
                viewport,
            );
            self.command_input.clear();
            return true;
        }
        if prompt.kind == PlanePromptKind::SurfaceOrigin {
            let option = input.trim_start_matches('_');
            let flip = if option.eq_ignore_ascii_case("Flip") {
                Some(!prompt.surface_flip)
            } else if let Some((name, value)) = option.split_once('=') {
                if name.eq_ignore_ascii_case("Flip") {
                    if value.trim_start_matches('_').eq_ignore_ascii_case("Yes") {
                        Some(true)
                    } else if value.trim_start_matches('_').eq_ignore_ascii_case("No") {
                        Some(false)
                    } else {
                        self.push_log("Error: Flip must be Yes or No".into());
                        return true;
                    }
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(flip) = flip {
                self.plane_prompt.as_mut().unwrap().surface_flip = flip;
                self.push_log(format!(
                    "CPlane Surface Flip={}",
                    if flip { "Yes" } else { "No" }
                ));
                self.command_input.clear();
                return true;
            }
        }
        if input.is_empty() && prompt.kind == PlanePromptKind::SurfaceOrigin {
            let prompt = self.plane_prompt.as_mut().unwrap();
            prompt.kind = PlanePromptKind::SurfaceX;
            self.push_log("CPlane Surface: pick an X direction (Enter uses surface U)".into());
            self.command_input.clear();
            return true;
        }
        if input.is_empty() && prompt.kind == PlanePromptKind::SurfaceX {
            let frame = prompt.surface_frame.unwrap();
            let viewport = prompt.viewport;
            self.plane_prompt = None;
            self.apply_plane_action(PlaneAction::Set(frame), viewport);
            self.command_input.clear();
            return true;
        }
        if prompt.kind == PlanePromptKind::ThreePoint && prompt.points.len() == 1 {
            let kind = if input
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Vertical")
            {
                Some(PlanePromptKind::ThreePointVertical)
            } else if input.trim_start_matches('_').eq_ignore_ascii_case("ZAxis") {
                Some(PlanePromptKind::ThreePointZAxis)
            } else {
                None
            };
            if let Some(kind) = kind {
                let message = {
                    let prompt = self.plane_prompt.as_mut().unwrap();
                    prompt.kind = kind;
                    prompt.message()
                };
                self.push_log(message.into());
                self.command_input.clear();
                return true;
            }
        }
        if input.is_empty()
            && prompt.points.is_empty()
            && matches!(
                prompt.kind,
                PlanePromptKind::Origin | PlanePromptKind::AllOrigin | PlanePromptKind::ThreePoint
            )
        {
            self.accept_plane_prompt_point(prompt.frame.origin());
            return true;
        }
        if (prompt.kind == PlanePromptKind::Elevation
            || (prompt.kind == PlanePromptKind::Rotate && prompt.points.len() == 2))
            && let Ok(value) = input.parse::<f64>()
        {
            let frame = if !value.is_finite() {
                Err(PlaneCommandError::Number)
            } else if prompt.kind == PlanePromptKind::Elevation {
                cplane::elevated(prompt.frame, value).map_err(PlaneCommandError::from)
            } else {
                cplane::rotated(
                    prompt.frame,
                    prompt.points[0],
                    prompt.points[1],
                    value,
                    self.document.tolerance(),
                )
                .map_err(PlaneCommandError::from)
            };
            match frame {
                Ok(frame) => {
                    self.snaps.plane_override = None;
                    let viewport = prompt.viewport;
                    self.plane_prompt = None;
                    self.apply_plane_action(PlaneAction::Set(frame), viewport);
                    self.command_input.clear();
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return true;
        }
        let point = PointInput::parse_with_units(input, self.document.units())
            .ok_or(PlaneCommandError::Usage)
            .and_then(|p| p.map_err(PlaneCommandError::from))
            .and_then(|p| {
                p.resolve(
                    self.viewports[self.active_viewport].construction_plane(),
                    prompt.previous,
                )
                .map_err(PlaneCommandError::from)
            });
        match point {
            Ok(point) => {
                self.accept_plane_prompt_point(point);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn accept_plane_prompt_point(&mut self, point: Point3) -> bool {
        let Some(mut prompt) = self.plane_prompt.take() else {
            return false;
        };
        if prompt.kind == PlanePromptKind::SurfaceOrigin {
            let (id, face) = prompt.surface_target.unwrap();
            match self.surface_target_frame(id, face, Some(point), None, prompt.surface_flip) {
                Ok(frame) => {
                    prompt.surface_frame = Some(frame);
                    prompt.kind = PlanePromptKind::SurfaceX;
                    prompt.previous = Some(point);
                    self.snaps.plane_override = None;
                    self.push_log(prompt.message().into());
                    self.plane_prompt = Some(prompt);
                    self.command_input.clear();
                    return true;
                }
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    self.plane_prompt = Some(prompt);
                    return false;
                }
            }
        }
        let tolerance = self.document.tolerance();
        let result = (|| -> Result<Option<PlaneAction>, PlaneCommandError> {
            Ok(match prompt.kind {
                PlanePromptKind::Origin => Some(PlaneAction::Set(prompt.frame.with_origin(point))),
                PlanePromptKind::AllOrigin => Some(PlaneAction::SetAllOrigin(point)),
                PlanePromptKind::ThroughAll => Some(PlaneAction::SetThroughAll(point)),
                PlanePromptKind::Through | PlanePromptKind::Elevation => {
                    Some(PlaneAction::Set(cplane::through(prompt.frame, point)?))
                }
                PlanePromptKind::ThreePoint => match prompt.points.as_slice() {
                    [] => {
                        prompt.points.push(point);
                        None
                    }
                    [origin] => {
                        origin.vector_to(point)?.normalized(tolerance)?;
                        prompt.points.push(point);
                        None
                    }
                    [origin, x] => Some(PlaneAction::Set(Frame3::try_from_points(
                        *origin, *x, point, tolerance,
                    )?)),
                    _ => unreachable!(),
                },
                PlanePromptKind::ThreePointVertical => {
                    let [origin] = prompt.points.as_slice() else {
                        unreachable!()
                    };
                    Some(PlaneAction::Set(cplane::vertical(
                        prompt.frame,
                        *origin,
                        point,
                        tolerance,
                    )?))
                }
                PlanePromptKind::ThreePointZAxis => {
                    let [origin] = prompt.points.as_slice() else {
                        unreachable!()
                    };
                    Some(PlaneAction::Set(cplane::three_point_z_axis(
                        *origin, point, tolerance,
                    )?))
                }
                PlanePromptKind::Rotate => match prompt.points.as_slice() {
                    [] => {
                        prompt.points.push(point);
                        None
                    }
                    [start] => {
                        start.vector_to(point)?.normalized(tolerance)?;
                        prompt.points.push(point);
                        None
                    }
                    [start, end] => {
                        cplane::rotated_by_reference_points(
                            prompt.frame,
                            *start,
                            *end,
                            point,
                            point,
                            tolerance,
                        )?;
                        prompt.points.push(point);
                        None
                    }
                    [start, end, reference] => {
                        Some(PlaneAction::Set(cplane::rotated_by_reference_points(
                            prompt.frame,
                            *start,
                            *end,
                            *reference,
                            point,
                            tolerance,
                        )?))
                    }
                    _ => unreachable!(),
                },
                PlanePromptKind::SurfaceOrigin => unreachable!(),
                PlanePromptKind::SurfaceX => Some(PlaneAction::Set(cplane::surface_frame_with_x(
                    prompt.surface_frame.unwrap(),
                    point,
                    tolerance,
                )?)),
                PlanePromptKind::Object | PlanePromptKind::SurfaceSelect => {
                    return Err(PlaneCommandError::Usage);
                }
            })
        })();
        match result {
            Ok(Some(action)) => {
                self.snaps.plane_override = None;
                if self.apply_plane_action(action, prompt.viewport) {
                    self.command_input.clear();
                    true
                } else {
                    self.plane_prompt = Some(prompt);
                    false
                }
            }
            Ok(None) => {
                self.snaps.plane_override = None;
                prompt.previous = Some(point);
                self.push_log(prompt.message().into());
                self.plane_prompt = Some(prompt);
                self.command_input.clear();
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.plane_prompt = Some(prompt);
                false
            }
        }
    }

    pub(super) fn accept_plane_prompt_object(&mut self, id: viboceros_document::ObjectId) -> bool {
        self.accept_plane_prompt_object_action(PlaneAction::Object(id))
    }

    pub(super) fn accept_plane_prompt_object_face(
        &mut self,
        id: viboceros_document::ObjectId,
        face: usize,
    ) -> bool {
        self.accept_plane_prompt_object_action(PlaneAction::ObjectFace(id, face))
    }

    pub(super) fn accept_plane_prompt_surface(
        &mut self,
        id: viboceros_document::ObjectId,
        face: Option<usize>,
    ) -> bool {
        let Some(prompt) = self
            .plane_prompt
            .as_ref()
            .filter(|prompt| prompt.requests_surface())
        else {
            return false;
        };
        self.begin_surface_prompt(id, face, None, false, prompt.viewport)
    }

    fn accept_plane_prompt_object_action(&mut self, action: PlaneAction) -> bool {
        let Some(prompt) = self
            .plane_prompt
            .as_ref()
            .filter(|prompt| prompt.requests_object())
        else {
            return false;
        };
        let viewport = prompt.viewport;
        if self.apply_plane_action(action, viewport) {
            self.plane_prompt = None;
            self.command_input.clear();
            true
        } else {
            false
        }
    }
}
