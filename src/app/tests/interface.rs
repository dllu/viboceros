use super::*;
use viboceros_command::interface::{self, InterfaceCommand, SwitchAction, ViewportTabAlignment};

mod command_state;
mod drafting;
mod end_analysis;
mod navigation;
mod primitive_prompts;
mod selection_paths;
mod selection_regions;
mod shortcuts;
mod toolbar;
mod viewport_layout;

fn enter(app: &mut VibocerosApp, command: &str) {
    app.command_input = command.into();
    app.run_command();
}

fn zoom_window_frame(
    context: &egui::Context,
    app: &mut VibocerosApp,
    events: Vec<egui::Event>,
) -> ViewportOutput {
    let mut output = ViewportOutput::default();
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                output = app.viewports[0].show(
                    ui,
                    &app.document,
                    ViewportInput {
                        zoom_window: true,
                        drafting: DraftingInput {
                            active: true,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    output
}

fn zoom_target_frame(
    context: &egui::Context,
    app: &mut VibocerosApp,
    mode: ZoomTargetInput,
    events: Vec<egui::Event>,
) -> ViewportOutput {
    let mut output = ViewportOutput::default();
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                output = app.viewports[0].show(
                    ui,
                    &app.document,
                    ViewportInput {
                        zoom_target: Some(mode),
                        drafting: DraftingInput {
                            active: true,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    output
}

fn selection_capture_frame(
    context: &egui::Context,
    app: &mut VibocerosApp,
    events: Vec<egui::Event>,
) -> ViewportOutput {
    let mut output = ViewportOutput::default();
    let object_filter = if app
        .fence_selection
        .as_ref()
        .is_some_and(|state| state.curve_pick)
        || app.boundary_selection.is_some()
    {
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    } else {
        app.viewport_object_filter()
    };
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                output = app.viewports[0].show(
                    ui,
                    &app.document,
                    ViewportInput {
                        object_filter,
                        rect_selection_mode: app.selection_window_override,
                        circular_selection: match app.circular_selection {
                            Some(CircularSelectionState::PickCenter(_)) => {
                                Some(CircularSelectionInput::PickCenter)
                            }
                            Some(CircularSelectionState::PickRadius {
                                mode,
                                center,
                                viewport: 0,
                            }) => Some(CircularSelectionInput::PickRadius { mode, center }),
                            Some(CircularSelectionState::PickRadius { .. }) => {
                                Some(CircularSelectionInput::Waiting)
                            }
                            None => None,
                        },
                        fence_selection: match app.fence_selection.as_ref() {
                            Some(state) if state.curve_pick => None,
                            Some(state) if state.viewport.is_none() => {
                                Some(FenceSelectionInput::PickFirst)
                            }
                            Some(state) if state.viewport == Some(0) => {
                                Some(FenceSelectionInput::Continue(&state.points))
                            }
                            Some(_) => Some(FenceSelectionInput::Waiting),
                            None => None,
                        },
                        lasso_selection: match app.lasso_selection.as_ref() {
                            Some(state)
                                if state.viewport.is_none() || state.viewport == Some(0) =>
                            {
                                Some(LassoSelectionInput::Capture {
                                    points: &state.points,
                                    mode: state.mode,
                                })
                            }
                            Some(_) => Some(LassoSelectionInput::Waiting),
                            None => None,
                        },
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    output
}

fn key(key: egui::Key, modifiers: egui::Modifiers, pressed: bool, repeat: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat,
        modifiers,
    }
}

fn frame(
    context: &egui::Context,
    app: &mut VibocerosApp,
    width: f32,
    events: Vec<egui::Event>,
) -> (f32, egui::FullOutput) {
    let mut height = 0.0;
    let output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 600.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            app.handle_interface_shortcuts(ui);
            app.show_toolbar(ui);
            height = ui.available_rect_before_wrap().top();
            app.capture_global_command_typing(ui);
            app.show_command_line(ui);
        },
    );
    (height, output)
}

fn label_position(shapes: &[egui::epaint::ClippedShape], label: &str) -> egui::Pos2 {
    fn find(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                // Wrapped horizontal labels include leading indentation in
                // their layout rect. Aim at the painted glyphs, not that rect.
                Some(text.pos + text.galley.mesh_bounds.center().to_vec2())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
            _ => None,
        }
    }
    shapes
        .iter()
        .find_map(|shape| find(&shape.shape, label))
        .unwrap_or_else(|| panic!("missing label {label}"))
}

fn click(context: &egui::Context, app: &mut VibocerosApp, position: egui::Pos2) {
    for pressed in [true, false] {
        frame(
            context,
            app,
            1000.0,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .1
        .drop_without_applying_deltas();
    }
}

pub(super) fn layout_viewports(context: &egui::Context, app: &mut VibocerosApp) {
    for index in 0..app.viewports.len() {
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    app.viewports[index].show(
                        ui,
                        &app.document,
                        ViewportInput::default(),
                        &[],
                        index,
                        true,
                    );
                },
            )
            .drop_without_applying_deltas();
    }
}
