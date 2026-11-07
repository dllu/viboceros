use super::*;

#[test]
fn contact_hover_cancels_without_changing_the_accepted_surface_jet() {
    use viboceros_command::blend_curve::Handle;
    let data = native::read("blend_curve_surface_high_holdouts");
    for order in [3, 4] {
        let name = format!("surface_high_slide_g{order}");
        let value = &data["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == name)
            .unwrap()["value"];
        let mut app = test_app();
        app.document = edges::setup(value);
        app.commands
            .remember_blend_curve_preferences(edges::preferences(value));
        enter(&mut app, "BlendCrv");
        for (index, a) in value["prescribed_case"]["actions"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            if a.get("click").is_some() {
                break;
            }
            if let Some(source) = a["curve"].as_u64() {
                let input = &app.blend_curve_session.as_ref().unwrap().input;
                let point = blend::source_pick(value, index, a, input);
                app.pick_blend_curve_at(
                    reference_inputs::guide(&app.document, source as usize),
                    blend::source_edge(value, index, a),
                    point,
                );
            } else {
                enter(&mut app, a.as_str().unwrap());
            }
        }
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        let before = input.preview().cloned().unwrap();
        let arguments = input.arguments().unwrap();
        app.select_blend_curve_handle(Handle::Start);
        let display = app
            .blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .adjustment_curve()
            .cloned()
            .unwrap();
        let geometry = Geometry::NurbsCurve(display);
        let t = geometry
            .curve_ref()
            .unwrap()
            .closest_parameter(
                Point3::try_new(0., -1., 0.).unwrap(),
                app.document.tolerance(),
            )
            .unwrap();
        app.hover_blend_curve_parameter(t);
        let session = app.blend_curve_session.as_ref().unwrap();
        assert_eq!(session.input.preview(), Some(&before));
        assert_eq!(session.input.arguments().unwrap(), arguments);
        assert_ne!(session.preview_input().preview(), Some(&before));
        assert!(!app.document.can_undo());
        enter(&mut app, "Cancel");
        assert_eq!(
            app.blend_curve_session.as_ref().unwrap().input.preview(),
            Some(&before)
        );
        app.select_blend_curve_handle(Handle::Start);
        app.hover_blend_curve_parameter(t);
        assert!(app.accept_blend_curve_parameter(t));
        assert!(!app.document.can_undo());
        enter(&mut app, "");
        let (error, domain) = blend::check_high(&app.document, &value["after"], false);
        assert!(
            error <= 1e-6 && domain <= 2e-6,
            "{name}: {error:e}, {domain:e}"
        );
        assert_eq!(app.document.undo_label(), Some("BlendCrv"));
        app.document.undo().unwrap();
        assert!(!app.document.can_undo());
    }
}
