//! Scalar preferences survive model history and retain numeric input units.
use super::*;

fn selected_point() -> Document {
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::Point(Point3::try_new(2., 3., 4.).unwrap()))
        .unwrap();
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
    document
}

#[test]
fn scalar_memory_is_per_command_and_registry_and_survives_undo_and_copy_resets() {
    let registry = CommandRegistry::with_builtins();
    let independent = CommandRegistry::with_builtins();
    let mut document = selected_point();
    for name in ["Scale", "Scale1D", "Scale2D"] {
        assert_eq!(registry.transform_scalar_default(name), Some(1.));
    }
    for name in ["Rotate", "Rotate3D", "Shear", "missing"] {
        assert_eq!(registry.transform_scalar_default(name), None);
    }
    registry.execute(&mut document, "Scale 0,0,0 -2").unwrap();
    assert_eq!(registry.transform_scalar_default("_-Scale"), Some(2.));
    registry
        .execute(&mut document, "Scale1D 0,0,0 -3 1,0,0")
        .unwrap();
    assert_eq!(registry.transform_scalar_default("Scale1D"), Some(-3.));
    assert_eq!(registry.transform_scalar_default("Scale2D"), Some(1.));
    for script in ["Undo", "Redo", "RememberCopyOptions No"] {
        registry.execute(&mut document, script).unwrap();
        assert_eq!(registry.transform_scalar_default("Scale1D"), Some(-3.));
    }
    let before = format!("{document:?}");
    assert!(!registry.remember_pending_transform_scalar("Scale1D", Real::NAN));
    assert!(!registry.remember_pending_transform_scalar("Scale1D", Real::INFINITY));
    assert!(!registry.remember_pending_transform_scalar("Scale", 9.));
    assert!(!registry.remember_pending_transform_scalar("missing", 9.));
    assert_eq!(registry.transform_scalar_default("Scale1D"), Some(-3.));
    assert!(registry.remember_pending_transform_scalar("Scale1D", 0.));
    assert_eq!(registry.transform_scalar_default("Scale1D"), Some(0.));
    assert_eq!(format!("{document:?}"), before);
    assert_eq!(independent.transform_scalar_default("Scale1D"), Some(1.));
    assert_eq!(independent.transform_scalar_default("Scale"), Some(1.));
}

#[test]
fn remembered_numeric_angles_preserve_degrees_without_a_radian_roundtrip() {
    let registry = CommandRegistry::with_builtins();
    let mut document = selected_point();
    for command in ["Rotate", "Rotate3D"] {
        let axis = if command == "Rotate3D" {
            "0,0,0 0,0,1"
        } else {
            "0,0,0"
        };
        for angle in [30., -22.4, 720.0000009, 0.] {
            registry
                .execute(&mut document, &format!("{command} {axis} {angle}"))
                .unwrap();
            assert_eq!(registry.transform_scalar_default(command), Some(angle));
        }
        let before = format!("{document:?}");
        assert!(
            registry
                .execute(&mut document, &format!("{command} {axis} NaN"))
                .is_err()
        );
        assert_eq!(registry.transform_scalar_default(command), Some(0.));
        assert_eq!(format!("{document:?}"), before);
    }
}
