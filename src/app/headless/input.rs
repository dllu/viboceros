//! Scripting inputs feed the application's existing command and viewport events.
use super::*;
use crate::viewport::{ComponentClick, ControlPointSelection, EdgePick, ViewKind};
use viboceros_command::{ComponentSelectionKind, component_selection::stage_sources};

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum ViewRef {
    Index(usize),
    Name(String),
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Step {
    Text {
        text: String,
    },
    Point {
        point: [f64; 3],
        world: bool,
    },
    Pick {
        id: String,
        remove: bool,
    },
    PickComponent {
        component: ComponentRef,
        point: Option<[f64; 3]>,
        remove: bool,
    },
    PickGrip {
        grip: GripRef,
        remove: bool,
    },
    PickPoint {
        point: [f64; 3],
        view: Option<ViewRef>,
        modifiers: Modifiers,
    },
    Pointer {
        position: [f32; 2],
        action: PointerAction,
        button: Button,
        modifiers: Modifiers,
    },
    Key {
        key: String,
        modifiers: Modifiers,
    },
    ActivateView {
        view: ViewRef,
    },
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub(super) struct Modifiers {
    shift: bool,
    ctrl: bool,
    alt: bool,
    command: bool,
}

impl From<Modifiers> for egui::Modifiers {
    fn from(value: Modifiers) -> Self {
        Self {
            shift: value.shift,
            ctrl: value.ctrl,
            alt: value.alt,
            command: value.command || (!cfg!(target_os = "macos") && value.ctrl),
            mac_cmd: cfg!(target_os = "macos") && value.command,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PointerAction {
    Move,
    Press,
    Release,
    Click,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Button {
    Primary,
    Secondary,
    Middle,
}

impl From<Button> for egui::PointerButton {
    fn from(value: Button) -> Self {
        match value {
            Button::Primary => Self::Primary,
            Button::Secondary => Self::Secondary,
            Button::Middle => Self::Middle,
        }
    }
}

impl Session {
    pub(super) fn frame(&mut self, mut events: Vec<egui::Event>, modifiers: egui::Modifiers) {
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        self.context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN_SIZE)),
                    events,
                    ..Default::default()
                },
                |ui| self.app.show_frame(ui, None),
            )
            .drop_without_applying_deltas();
    }

    pub(super) fn step(&mut self, step: Step) -> Result<()> {
        match step {
            Step::Text { text } => {
                if text.contains(['\r', '\n', '\0']) {
                    return Err("submit one command line or prompt answer per step".into());
                }
                self.submit(text);
            }
            Step::Point { point, world } => {
                let [x, y, z] = Point3::try_from(point)?.to_array();
                let prefix = if world { "w" } else { "" };
                self.submit(format!("{prefix}{x},{y},{z}"));
            }
            Step::Pick { id, remove } => {
                let id = model::object_id(&id)?;
                if !self.app.document.is_object_selectable(id) {
                    return Err(format!("object {id} is unavailable for picking").into());
                }
                let mode = if remove {
                    SelectionMode::Remove
                } else {
                    SelectionMode::Add
                };
                self.app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode,
                });
                self.app.command_file_user_step();
            }
            Step::PickComponent {
                component,
                point,
                remove,
            } => {
                let pick = component.pick()?;
                stage_sources(&self.app.document, vec![pick])?;
                let point = point.map(Point3::try_from).transpose()?;
                let mut output = ViewportOutput::default();
                if !remove
                    && matches!(
                        pick.kind,
                        ComponentSelectionKind::BrepFace | ComponentSelectionKind::MeshFace
                    )
                    && self.pending()
                    && self.app.shell_prompt.is_none()
                    && !self.app.picking_nonplanar_threshold()
                {
                    output.face_click = Some((pick.object, pick.index));
                    output.face_hit_point = point;
                } else if !remove
                    && pick.kind == ComponentSelectionKind::BrepEdge
                    && (self.app.edge_prompt.is_some() || self.app.blend_surface_prompt.is_some())
                {
                    let edge = EdgePick {
                        object: pick.object,
                        edge: pick.index,
                    };
                    output.edge_click = Some(vec![edge]);
                    output.edge_contacts =
                        point.map(|point| vec![(edge, point)]).unwrap_or_default();
                } else {
                    output.component_click = Some(ComponentClick {
                        picks: vec![pick],
                        preselection: !self.pending(),
                        modifiers: egui::Modifiers {
                            ctrl: remove,
                            ..Default::default()
                        },
                    });
                }
                self.app.handle_viewport_action(output);
                self.app.command_file_user_step();
            }
            Step::PickGrip { grip, remove } => {
                let mode = if remove {
                    SelectionMode::Remove
                } else {
                    SelectionMode::Add
                };
                self.app.handle_viewport_action(ViewportOutput {
                    control_point_selection: Some(ControlPointSelection {
                        picks: vec![grip.pick()?],
                        mode,
                    }),
                    ..Default::default()
                });
                self.app.command_file_user_step();
            }
            Step::PickPoint {
                point,
                view,
                modifiers,
            } => {
                let pos = self.project_point(point, view.as_ref())?;
                self.pointer(
                    pos,
                    PointerAction::Click,
                    egui::PointerButton::Primary,
                    modifiers.into(),
                );
            }
            Step::Pointer {
                position: [x, y],
                action,
                button,
                modifiers,
            } => {
                let pos = egui::pos2(x, y);
                if !pos.is_finite() {
                    return Err("pointer coordinates must be finite".into());
                }
                self.pointer(pos, action, button.into(), modifiers.into());
            }
            Step::Key { key, modifiers } => {
                let key = egui::Key::from_name(&key).ok_or("unknown keyboard key")?;
                let modifiers = modifiers.into();
                for pressed in [true, false] {
                    self.frame(
                        vec![egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed,
                            repeat: false,
                            modifiers,
                        }],
                        modifiers,
                    );
                }
            }
            Step::ActivateView { view } => {
                let index = self.viewport_index(Some(&view))?;
                self.app.activate_model_viewport(index);
                self.frame(vec![], egui::Modifiers::NONE);
            }
        }
        Ok(())
    }

    fn submit(&mut self, text: String) {
        self.app.command_input = text;
        self.app.run_command();
        while self.app.drain_command_file() {}
    }

    fn pointer(
        &mut self,
        pos: egui::Pos2,
        action: PointerAction,
        button: egui::PointerButton,
        modifiers: egui::Modifiers,
    ) {
        self.frame(vec![egui::Event::PointerMoved(pos)], modifiers);
        let transitions: &[bool] = match action {
            PointerAction::Move => &[],
            PointerAction::Press => &[true],
            PointerAction::Release => &[false],
            PointerAction::Click => &[true, false],
        };
        for &pressed in transitions {
            self.frame(
                vec![egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                }],
                modifiers,
            );
        }
    }

    fn viewport_index(&self, view: Option<&ViewRef>) -> Result<usize> {
        match view {
            None => Ok(self.app.active_viewport),
            Some(ViewRef::Index(index)) if *index < self.app.viewports.len() => Ok(*index),
            Some(ViewRef::Index(index)) => Err(format!("no viewport at index {index}").into()),
            Some(ViewRef::Name(name)) if name.eq_ignore_ascii_case("active") => {
                Ok(self.app.active_viewport)
            }
            Some(ViewRef::Name(name)) => self
                .app
                .viewports
                .iter()
                .position(|view| view.view_label().eq_ignore_ascii_case(name))
                .ok_or_else(|| format!("unknown viewport {name:?}").into()),
        }
    }

    pub(super) fn viewports(&mut self) -> Value {
        self.frame(vec![], egui::Modifiers::NONE);
        json!(self.app.viewports.iter().enumerate().map(|(index, view)| {
            let rect = self.visible_viewport_rect(index).ok();
            json!({
                "index": index,
                "name": view.view_label(),
                "kind": view.kind().label(),
                "mode": view.display_mode.label(),
                "active": index == self.app.active_viewport,
                "rectangle": rect.map(|rect| [rect.min.x, rect.min.y, rect.max.x, rect.max.y]),
            })
        }).collect::<Vec<_>>())
    }

    fn visible_viewport_rect(&self, index: usize) -> Result<egui::Rect> {
        if self
            .app
            .maximized_viewport
            .is_some_and(|shown| index != shown)
        {
            return Err("viewport is hidden by a maximized view".into());
        }
        self.app.viewports[index]
            .screen_rect()
            .ok_or_else(|| "viewport has no layout".into())
    }

    pub(super) fn project_point(
        &mut self,
        point: [f64; 3],
        view: Option<&ViewRef>,
    ) -> Result<egui::Pos2> {
        self.frame(vec![], egui::Modifiers::NONE);
        let index = self.viewport_index(view)?;
        let rect = self.visible_viewport_rect(index)?;
        self.app.viewports[index]
            .project(Point3::try_from(point)?, rect)
            .filter(|position| rect.contains(*position))
            .ok_or_else(|| "world point is outside the viewport".into())
    }

    pub(super) fn capture_view(&self, view: Option<&ViewRef>) -> Result<Viewport> {
        if let Ok(index) = self.viewport_index(view) {
            let viewport = &self.app.viewports[index];
            return Ok(viewport.duplicate_for_layout(viewport.view_label()));
        }
        if let Some(ViewRef::Name(name)) = view {
            if let Ok(snapshot) = self.app.named_views.get(name) {
                let mut viewport = Viewport::default();
                viewport.restore_named_view(*snapshot);
                return Ok(viewport);
            }
            let presets = [
                ViewKind::Top,
                ViewKind::Bottom,
                ViewKind::Front,
                ViewKind::Back,
                ViewKind::Right,
                ViewKind::Left,
                ViewKind::Perspective,
                ViewKind::Plan,
            ];
            if let Some(kind) = presets
                .into_iter()
                .find(|kind| kind.label().eq_ignore_ascii_case(name))
            {
                return Ok(Viewport::new_for_layout(
                    &self.app.viewports[self.app.active_viewport],
                    kind,
                ));
            }
        }
        Err(self.viewport_index(view).unwrap_err())
    }
}
