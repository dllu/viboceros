use super::*;
use crate::viewport::{DisplayMode, ViewKind, ViewportInput, ViewportOutput};
use egui::{Context, Event, PointerButton, Pos2, Rect, Vec2};
use std::collections::BTreeSet;

type Segment = [i32; 4];
fn segment([a, b]: [Pos2; 2]) -> Segment {
    let a = [(a.x * 100.).round() as i32, (a.y * 100.).round() as i32];
    let b = [(b.x * 100.).round() as i32, (b.y * 100.).round() as i32];
    let [a, b] = if a < b { [a, b] } else { [b, a] };
    [a[0], a[1], b[0], b[1]]
}
struct Frames {
    context: Context,
    view: Viewport,
    time: f64,
}
impl Frames {
    fn new(mode: DisplayMode) -> Self {
        let mut view = Viewport::new(ViewKind::Top);
        view.display_mode = mode;
        Self {
            context: Context::default(),
            view,
            time: 0.,
        }
    }
    fn run(
        &mut self,
        app: &VibocerosApp,
        events: Vec<Event>,
    ) -> (ViewportOutput, Rect, BTreeSet<Segment>, BTreeSet<Segment>) {
        self.time += 0.4;
        let mut area = Rect::NOTHING;
        let mut output = ViewportOutput::default();
        let highlights = app.commands.component_selection().live_picks(&app.document);
        let raw = app.component_selection.raw_face_picks(&app.document);
        let drawn = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ui| {
                area = ui.available_rect_before_wrap();
                output = self.view.show(
                    ui,
                    &app.document,
                    ViewportInput {
                        object_filter: app.viewport_object_filter(),
                        component_pick: Some(
                            crate::viewport::ComponentPickFilter::MeshAndSubDEdges,
                        ),
                        component_highlights: &highlights,
                        raw_mesh_face_highlights: Some(&raw),
                        component_previews: app
                            .edge_pattern_prompt
                            .as_ref()
                            .map_or(&[], |p| p.preview()),
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        );
        let lines = |width, color| {
            drawn
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::LineSegment { points, stroke }
                        if stroke.width == width && stroke.color == color =>
                    {
                        Some(segment(*points))
                    }
                    _ => None,
                })
                .collect()
        };
        let dim = lines(1.8, egui::Color32::from_rgb(165, 120, 35));
        let bright = lines(3., egui::Color32::from_rgb(245, 160, 20));
        drawn.drop_without_applying_deltas();
        (output, area, dim, bright)
    }
    fn click(&mut self, app: &mut VibocerosApp, pos: Pos2, modifiers: egui::Modifiers) {
        self.run(
            app,
            vec![
                Event::ModifiersChanged(modifiers),
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers,
                },
            ],
        );
        let (output, ..) = self.run(
            app,
            vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers,
            }],
        );
        assert!(output.component_click.is_some());
        app.handle_viewport_action(output);
    }
}
fn outlines(
    app: &VibocerosApp,
    id: ObjectId,
    faces: &[usize],
    view: &Viewport,
    area: Rect,
) -> BTreeSet<Segment> {
    let (points, polygons) = match app.document.object(id).unwrap().geometry() {
        Geometry::Mesh(m) => (
            m.vertices().to_vec(),
            faces
                .iter()
                .map(|&i| m.faces()[i].indices().to_vec())
                .collect::<Vec<_>>(),
        ),
        Geometry::SubD(s) => (
            s.control_points().to_vec(),
            faces.iter().map(|&i| s.faces()[i].clone()).collect(),
        ),
        _ => panic!(),
    };
    polygons
        .iter()
        .flat_map(|f| (0..f.len()).map(|i| [f[i], f[(i + 1) % f.len()]]))
        .map(|e| segment(e.map(|i| view.project(points[i as usize], area).unwrap())))
        .collect()
}
#[test]
fn face_loop_pointer_previews_and_partial_ngon_acceptance_draw_exact_faces_in_all_modes() {
    for family in ["face_loop_discovery", "component_pattern_holdouts"] {
        let (request, observed) = faces::data(family);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            if op["op"] != "face_loop_workflow" || op["steps"].as_array().unwrap().len() != 1 {
                continue;
            }
            let v = &row["value"];
            let pair = indices(&op["pair"]);
            let station = Point3::try_from(std::array::from_fn(|i| {
                (v["input"]["points"][pair[0]][i].as_f64().unwrap()
                    + v["input"]["points"][pair[1]][i].as_f64().unwrap())
                    * 0.5
            }))
            .unwrap();
            for mode in [
                DisplayMode::Wireframe,
                DisplayMode::Shaded,
                DisplayMode::Ghosted,
            ] {
                let (mut app, id) = fixture(v);
                let source = app.document.object(id).unwrap().clone();
                enter(&mut app, "SelFaceLoop");
                let mut frames = Frames::new(mode);
                let (_, area, ..) = frames.run(&app, vec![]);
                let pos = frames.view.project(station, area).unwrap();
                frames.click(&mut app, pos, egui::Modifiers::NONE);
                assert_eq!(app.commands.component_selection().len(), 1);
                let expected = outlines(
                    &app,
                    id,
                    &indices(&v["after"]["face_ordinals"]),
                    &frames.view,
                    area,
                );
                let (_, _, dim, bright) = frames.run(&app, vec![]);
                assert_eq!(dim, expected, "{} {mode:?}", row["id"]);
                assert_eq!(bright.len(), 1);
                frames.click(&mut app, pos, egui::Modifiers::CTRL);
                let (_, _, dim, bright) = frames.run(&app, vec![]);
                assert!(
                    dim.is_empty(),
                    "{} {mode:?}: preview remains after Ctrl",
                    row["id"]
                );
                assert!(app.commands.component_selection().is_empty());
                // The hovered edge can remain bright after its seed is removed.
                assert!(bright.len() <= 1);
                frames.click(&mut app, pos, egui::Modifiers::NONE);
                let (_, _, _, seeds) = frames.run(&app, vec![]);
                enter(&mut app, "");
                let (_, _, dim, bright) = frames.run(&app, vec![]);
                assert!(dim.is_empty());
                assert_eq!(
                    bright,
                    expected.union(&seeds).copied().collect(),
                    "{} {mode:?}",
                    row["id"]
                );
                assert_eq!(app.document.object(id), Some(&source));
                assert!(!app.document.can_undo());
            }
        }
    }
}
