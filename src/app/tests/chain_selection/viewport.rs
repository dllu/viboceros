use super::*;
use crate::viewport::{ViewKind, ViewportInput, ViewportOutput};
use egui::{Context, Event, PointerButton, Pos2, Rect, Vec2};
struct Pointer {
    view: Viewport,
    context: Context,
    time: f64,
    area: Rect,
}
impl Pointer {
    fn new(mode: DisplayMode) -> Self {
        let mut view = Viewport::new(ViewKind::Top);
        view.display_mode = mode;
        Self {
            view,
            context: Context::default(),
            time: 0.,
            area: Rect::NOTHING,
        }
    }
    fn frame(&mut self, app: &VibocerosApp, events: Vec<Event>) -> (ViewportOutput, bool) {
        self.time += 0.4;
        let mut output = ViewportOutput::default();
        let drawn = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ui| {
                self.area = ui.available_rect_before_wrap();
                output = self.view.show(
                    ui,
                    &app.document,
                    ViewportInput {
                        chain_pick: app.chain_prompt.is_some(),
                        chain_highlights: app.chain_prompt.as_ref().map_or(&[], |p| p.highlights()),
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        );
        let highlighted = drawn.shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::LineSegment {stroke,..} if stroke.width == 3. && stroke.color==egui::Color32::from_rgb(245,160,20)));
        drawn.drop_without_applying_deltas();
        (output, highlighted)
    }
    fn click(&mut self, app: &mut VibocerosApp, station: Point3) {
        self.frame(app, vec![]);
        let pos = self.view.project(station, self.area).unwrap();
        self.frame(
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let (output, _) = self.frame(
            app,
            vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(output.chain_click.as_ref().unwrap().len(), 1);
        assert!(output.component_click.is_none());
        assert!(output.picked_point.is_none());
        app.handle_viewport_action(output);
    }
}
const MODES: [DisplayMode; 3] = [
    DisplayMode::Wireframe,
    DisplayMode::Shaded,
    DisplayMode::Ghosted,
];
#[test]
fn real_pointer_chain_seeds_pick_curves_and_surface_edges_in_all_display_modes() {
    for (scene, station) in [
        ("lines", point(4.5, 0., 0.)),
        ("surface", point(1.5, 0., 0.)),
        ("mixed", point(4.5, 0., 0.)),
    ] {
        for mode in MODES {
            let (mut app, ids) = fixture(scene);
            enter(&mut app, "SelChain");
            Pointer::new(mode).click(&mut app, station);
            enter(&mut app, "");
            let selected = app.document.selected_object_ids().collect::<BTreeSet<_>>();
            if scene == "lines" {
                assert_eq!(selected, ids[..3].iter().copied().collect());
            } else {
                assert_eq!(app.commands.component_selection().len(), 1);
                if scene == "mixed" {
                    assert_eq!(selected, BTreeSet::from([ids[1]]));
                }
            }
            assert!(!app.document.can_undo());
        }
    }
}
#[test]
fn real_pointer_branch_preview_requires_confirmation_and_can_choose_an_alternative() {
    for mode in MODES {
        for alternative in [false, true] {
            let (mut app, ids) = fixture("branch");
            enter(&mut app, "SelChain ChainContinuity=Position");
            let mut pointer = Pointer::new(mode);
            pointer.click(&mut app, point(1.5, 0., 0.));
            assert_eq!(
                app.document.selected_object_ids().collect::<Vec<_>>(),
                [ids[0]]
            );
            let prompt = app.chain_prompt.as_ref().unwrap();
            assert_eq!(prompt.highlights(), [Target::Curve(ids[2])]);
            assert!(prompt.hint().contains("branch"));
            let (_, highlighted) = pointer.frame(&app, vec![Event::PointerGone]);
            assert!(highlighted, "branch proposal must be drawn in {mode:?}");
            if alternative {
                pointer.click(&mut app, point(4.5, 0., 0.));
            } else {
                enter(&mut app, "");
            }
            assert!(app.chain_prompt.is_some());
            assert!(app.chain_prompt.as_ref().unwrap().highlights().is_empty());
            assert_eq!(
                app.document.selected_object_ids().collect::<BTreeSet<_>>(),
                BTreeSet::from([ids[0], ids[if alternative { 1 } else { 2 }]])
            );
            enter(&mut app, "");
            assert!(app.chain_prompt.is_none());
            assert!(!app.document.can_undo());
        }
    }
}
