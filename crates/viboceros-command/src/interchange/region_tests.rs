use super::*;
use crate::CommandRegistry;
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Point3, Vector3};

fn sphere(center: [f64; 3], radius: f64) -> Brep {
    let frame = Frame3::try_from_normal(
        Point3::try_new(center[0], center[1], center[2]).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    Brep::try_surface_grid(
        &NurbsSurface::try_sphere(frame, radius).unwrap(),
        &[],
        &[],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn curved_cavity_file_commands_keep_material_and_replay_one_import() {
    let shape = Brep::try_combine(
        vec![sphere([0.; 3], 2.).reversed(), sphere([0.; 3], 4.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let expected = shape.signed_volume(Tolerance::DEFAULT).unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut source = Document::default();
    source.add_geometry(Geometry::Brep(shape)).unwrap();
    let before = format!("{source:?}");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("curved cavity.step");
    registry
        .execute(
            &mut source,
            &format!("ExportStep Native=Yes \"{}\"", path.display()),
        )
        .unwrap();
    assert_eq!(format!("{source:?}"), before);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("BREP_WITH_VOIDS("));
    let mut target = Document::default();
    registry
        .execute(
            &mut target,
            &format!("ImportStep Native=Yes \"{}\"", path.display()),
        )
        .unwrap();
    assert_eq!(target.objects().len(), 1);
    let geometry = target.objects().next().unwrap().geometry().clone();
    let Geometry::Brep(brep) = &geometry else {
        panic!("editable cavity expected")
    };
    assert_eq!(brep.edge_connected_face_components().len(), 2);
    assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-8);
    registry.execute(&mut target, "Undo").unwrap();
    assert_eq!(target.objects().len(), 0);
    registry.execute(&mut target, "Redo").unwrap();
    assert_eq!(target.objects().next().unwrap().geometry(), &geometry);
}

#[test]
fn failed_shell_classification_preserves_destination_document_and_history() {
    let shape = Brep::try_combine(
        vec![sphere([0.; 3], 2.), sphere([3., 0., 0.], 2.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut source = Document::default();
    source.add_geometry(Geometry::Brep(shape)).unwrap();
    let before = format!("{source:?}");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("original.step");
    std::fs::write(&path, b"original").unwrap();
    let result = registry.execute(
        &mut source,
        &format!("ExportStep Native=Yes \"{}\"", path.display()),
    );
    assert!(matches!(
        result,
        Err(CommandError::Step(
            viboceros_io::StepError::NativeShellClassification { .. }
        ))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    assert_eq!(format!("{source:?}"), before);
}
