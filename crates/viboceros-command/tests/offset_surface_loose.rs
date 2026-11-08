#[path = "support/brep_comparison.rs"]
#[allow(dead_code)]
mod brep_comparison;
#[path = "support/offset_surface.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/offset_surface_loose.rs"]
#[allow(dead_code)]
mod loose;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
use viboceros_command::{CommandRegistry, offset_surface};

#[test]
fn loose_offsets_match_native_geometry_metadata_history_and_archives() {
    let mut count = 0;
    for (id, v) in loose::rows() {
        if loose::diagnostic(&id, &v) {
            continue;
        }
        println!("LOOSE_OFFSET_CASE {id}");
        let mut d = loose::setup(&v);
        let state = inputs::State::capture(&d);
        let before = loose::check(&d, &v["before"], false);
        inputs::select_sources(&mut d);
        if inputs::success(&v) {
            let text = inputs::command(&v);
            let r = CommandRegistry::with_builtins();
            let result = if inputs::postselected(&v) {
                r.execute_postselected(&mut d, &text, viboceros_command::CommandContext::default())
            } else {
                r.execute(&mut d, &text)
            };
            result.unwrap_or_else(|e| panic!("{id}: {text}: {e}"));
        } else {
            let unchanged = format!("{d:?}");
            let _ = offset_surface::prepare(&d, inputs::initial(&v), &Default::default());
            assert_eq!(format!("{d:?}"), unchanged);
            if inputs::postselected(&v) {
                inputs::clear_command_selection(&mut d);
            }
        }
        let mut record = state.verify_with(&mut d, &v, loose::check);
        record["id"] = id.into();
        record["maximum_geometry_error"] = before
            .max(record["maximum_geometry_error"].as_f64().unwrap())
            .into();
        println!("LOOSE_OFFSET_COMMAND {record}");
        count += 1;
    }
    println!("LOOSE_OFFSET_COMMAND_COUNT {count}");
}

#[test]
fn rational_loose_walls_follow_original_boundary_parameters() {
    let (id, v) = loose::rows()
        .into_iter()
        .find(|(id, _)| id == "offset_l_solid_delete")
        .unwrap();
    let mut d = loose::setup(&v);
    inputs::select_sources(&mut d);
    CommandRegistry::with_builtins()
        .execute(&mut d, &inputs::command(&v))
        .unwrap();
    println!("LOOSE_RATIONAL_WALL_PREPARED {id}");
    loose::check(&d, &v["after"], false);
}

#[test]
fn unavailable_loose_inputs_decline_atomically_and_preserve_redo() {
    use viboceros_document::{Document, Geometry};
    use viboceros_geometry::{Brep, NurbsSurface, Point3, Tolerance};
    let t = Tolerance::DEFAULT;
    let crease = NurbsSurface::try_new(
        1,
        1,
        3,
        2,
        [
            [0., 0., 0.],
            [1., 0., 0.],
            [2., 0., 1.],
            [0., 1., 0.],
            [1., 1., 0.],
            [2., 1., 1.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let box_brep = Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[0., 2.], [0., 2.], [0., 2.]],
        t,
    )
    .unwrap();
    for geometry in [Geometry::NurbsSurface(crease), Geometry::Brep(box_brep)] {
        assert!(!offset_surface::loose_available(&geometry, t));
        let mut d = Document::new(t);
        let id = d.add_geometry(geometry).unwrap();
        d.set_transient_selection([id]).unwrap();
        d.clear_history().unwrap();
        let r = CommandRegistry::with_builtins();
        r.execute(&mut d, "Point 90,91,92").unwrap();
        d.undo().unwrap();
        let before = format!("{d:?}");
        assert!(r.execute(&mut d, "OffsetSrf 0.5 Loose=Yes").is_err());
        assert_eq!(format!("{d:?}"), before);
        assert!(d.can_redo());
        assert!(!d.can_undo());
    }
}
