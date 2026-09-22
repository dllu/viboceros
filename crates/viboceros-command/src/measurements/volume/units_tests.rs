use super::tests::mesh;
use super::*;
use crate::{CommandContext, CommandRegistry};
use viboceros_document::SelectionMode;
use viboceros_geometry::LengthUnitSystem;

#[test]
fn choices_are_remembered_outside_model_history_and_registries_are_isolated() {
    let mut d = Document::default();
    let r = CommandRegistry::with_builtins();
    let id = d.add_geometry(mesh(0., 1., false)).unwrap();
    r.execute(&mut d, "Point 9,8,7").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_object(id, SelectionMode::Replace).unwrap();
    let before = format!("{d:?}");
    r.accept_object_selection_input("Volume Units=Liter")
        .unwrap();
    assert!(
        r.execute(&mut d, "Volume")
            .unwrap()
            .ends_with("total volume 0.00001 liters")
    );
    assert_eq!(format!("{d:?}"), before);
    for invalid in [
        "Volume Units=Foot Continue=bad",
        "Volume Units=Inch Units=Foot",
        "Volume Units=Unset",
    ] {
        assert!(r.accept_object_selection_input(invalid).is_err());
        assert_eq!(
            r.object_selection_prompt("Volume")
                .unwrap()
                .unwrap()
                .choices[0]
                .value,
            "Liter"
        );
    }
    assert!(r.execute(&mut d, "VolumeCentroid Units=Meter").is_err());
    assert!(
        CommandRegistry::with_builtins()
            .execute(&mut d, "Volume")
            .unwrap()
            .ends_with("total volume 10")
    );
    assert_eq!(format!("{d:?}"), before);
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.objects().len(), 2);
}

#[test]
fn postselection_conversion_and_model_units_follow_metadata_without_scaling_geometry() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let id = d.add_geometry(mesh(0., 1., false)).unwrap();
    let geometry = d.object(id).unwrap().geometry().clone();
    for (model, choice, expected) in [
        (LengthUnitSystem::Millimeters, "Micron", 1e10),
        (LengthUnitSystem::Inches, "Foot", 10. / 1728.),
        (LengthUnitSystem::None, "Meter", 10.),
        (LengthUnitSystem::Meters, "ModelUnits", 10.),
    ] {
        d.set_units(model.clone(), false).unwrap();
        d.select_object(id, SelectionMode::Replace).unwrap();
        let undo = d.undo_label().map(str::to_owned);
        let report = r
            .execute_postselected(
                &mut d,
                &format!("Volume Units={choice}"),
                CommandContext::default(),
            )
            .unwrap();
        let actual: f64 = report
            .split("total volume ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(d.units(), &model);
        assert_eq!(d.object(id).unwrap().geometry(), &geometry);
        assert_eq!(d.selected_object_count(), 0);
        assert_eq!(d.undo_label(), undo.as_deref());
    }
}

#[test]
fn converted_query_can_succeed_when_model_coordinate_volume_overflows() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::with_units(
        Tolerance::DEFAULT,
        LengthUnitSystem::Custom {
            name: "small unit".into(),
            meters_per_unit: 2_f64.powi(-600),
        },
    )
    .unwrap();
    let id = d.add_geometry(mesh(0., 2_f64.powi(600), false)).unwrap();
    d.select_object(id, SelectionMode::Replace).unwrap();
    assert!(r.execute(&mut d, "Volume").is_err());
    assert!(
        r.execute(&mut d, "Volume Units=Meter")
            .unwrap()
            .ends_with("total volume 10 cubic Metres")
    );
}
