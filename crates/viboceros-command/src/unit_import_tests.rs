use super::*;
use viboceros_geometry::LengthUnitSystem;

struct TemporaryFile(std::path::PathBuf);
impl TemporaryFile {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "viboceros-unit-import-{}-{unique}.3dm",
            std::process::id()
        )))
    }
}
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn unit_aware_import_is_undoable_and_export_retains_target_units() {
    let input = TemporaryFile::new();
    let output = TemporaryFile::new();
    let mut source = Document::with_units(Tolerance::DEFAULT, LengthUnitSystem::Inches).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut source, "Point 1,2,3").unwrap();
    write_3dm_file(&input.0, &document_3dm_model(&source).unwrap()).unwrap();
    let mut target =
        Document::with_units(Tolerance::DEFAULT, LengthUnitSystem::Centimeters).unwrap();
    registry
        .execute(&mut target, &format!("Import3dm {}", input.0.display()))
        .unwrap();
    assert_eq!(target.units(), &LengthUnitSystem::Centimeters);
    assert_eq!(target.objects().len(), 1);
    let Geometry::Point(point) = target.objects().next().unwrap().geometry() else {
        panic!("lost point");
    };
    let point = *point;
    assert!(point.is_near(
        Point3::try_new(2.54, 5.08, 7.62).unwrap(),
        Tolerance::DEFAULT
    ));
    registry
        .execute(&mut target, &format!("Export3dm {}", output.0.display()))
        .unwrap();
    let exported = read_3dm_file(&output.0, Tolerance::DEFAULT).unwrap();
    assert_eq!(exported.units, LengthUnitSystem::Centimeters);
    assert_eq!(exported.objects[0].geometry, ThreeDmGeometry::Point(point));
    registry.execute(&mut target, "Undo").unwrap();
    assert_eq!(target.objects().len(), 0);
    assert_eq!(target.layers().len(), 1);
    registry.execute(&mut target, "Redo").unwrap();
    assert_eq!(target.objects().len(), 1);
    // Unset source units are rejected before document edits, including history.
    let mut source_model = document_3dm_model(&source).unwrap();
    source_model.units = LengthUnitSystem::Unset;
    write_3dm_file(&input.0, &source_model).unwrap();
    let mut empty =
        Document::with_units(Tolerance::DEFAULT, LengthUnitSystem::Centimeters).unwrap();
    // IDs are document-specific; compare this document against its own state.
    let empty_before = format!("{empty:?}");
    assert!(
        registry
            .execute(&mut empty, &format!("Import3dm {}", input.0.display()))
            .is_err()
    );
    assert_eq!(format!("{empty:?}"), empty_before);
    // Finite file-space coordinates can overflow after unit conversion.
    // Reject that import without leaving layers, objects, or history behind.
    source_model.units = LengthUnitSystem::Custom {
        name: "huge".into(),
        meters_per_unit: 1e300,
    };
    source_model.objects[0].geometry =
        ThreeDmGeometry::Point(Point3::try_new(1e100, 0.0, 0.0).unwrap());
    write_3dm_file(&input.0, &source_model).unwrap();
    assert!(
        registry
            .execute(&mut empty, &format!("Import3dm {}", input.0.display()))
            .is_err()
    );
    assert_eq!(format!("{empty:?}"), empty_before);
}
