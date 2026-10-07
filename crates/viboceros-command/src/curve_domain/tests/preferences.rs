use super::*;
use crate::subcurve_input::{SubcurveDefaults, SubcurveMode};

#[test]
fn subcurve_preferences_are_registry_local_and_ignore_global_copy_resets() {
    let registry = CommandRegistry::with_builtins();
    let fresh = CommandRegistry::with_builtins();
    let mut d = Document::default();
    assert_eq!(registry.subcurve_defaults(), SubcurveDefaults::default());
    registry.set_subcurve_options(Some(true), Some(SubcurveMode::MarkEnds), Some(true));
    registry.execute(&mut d, "RememberCopyOptions No").unwrap();
    assert_eq!(
        registry.subcurve_defaults(),
        SubcurveDefaults {
            copy: true,
            mode: SubcurveMode::MarkEnds,
            from_midpoint: true
        }
    );
    assert_eq!(fresh.subcurve_defaults(), SubcurveDefaults::default());
    registry.execute(&mut d, "RememberCopyOptions Yes").unwrap();
    assert!(registry.subcurve_defaults().copy);
}

#[test]
fn subcurve_omitted_options_use_immediate_memory_outside_document_history() {
    let registry = CommandRegistry::with_builtins();
    let mut d = Document::default();
    registry.execute(&mut d, "Line 0,0 4,6").unwrap();
    let id = d.objects().next().unwrap().id();
    d.select_command_results([id]).unwrap();
    d.clear_history().unwrap();
    registry.set_subcurve_options(Some(true), None, Some(true));
    registry
        .execute(
            &mut d,
            "SubCrv Numeric=3.605551275463989,1,3.605551275463989",
        )
        .unwrap();
    assert_eq!(d.objects().len(), 2);
    assert!(registry.subcurve_defaults().copy);
    assert!(registry.subcurve_defaults().from_midpoint);
    registry.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().len(), 1);
    registry.set_subcurve_options(Some(false), Some(SubcurveMode::MarkEnds), Some(false));
    registry.execute(&mut d, "Redo").unwrap();
    assert_eq!(
        registry.subcurve_defaults(),
        SubcurveDefaults {
            copy: false,
            mode: SubcurveMode::MarkEnds,
            from_midpoint: false
        }
    );
    d.select_command_results([id]).unwrap();
    let before = registry.subcurve_defaults();
    assert!(
        registry
            .execute(&mut d, "SubCrv Numeric=1,2,3 Copy=Maybe")
            .is_err()
    );
    assert_eq!(registry.subcurve_defaults(), before);
    registry.execute(&mut d, "SubCrv 1,1.5 3,4.5").unwrap();
    assert_eq!(d.objects().len(), 4);
    assert!(
        d.objects()
            .skip(2)
            .all(|o| matches!(o.geometry(), Geometry::Point(_)))
    );
}
