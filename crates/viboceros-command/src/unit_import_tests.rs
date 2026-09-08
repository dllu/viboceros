use super::*;
use viboceros_geometry::LengthUnitSystem;

#[test]
fn stl_import_preserves_small_facets_and_document_settings_through_history() {
    let path = TemporaryFile::with_extension("stl");
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1e-12, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1e-12, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap();
    write_stl_file(&path.0, &mesh, StlFormat::Ascii).unwrap();
    let tolerance = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    let mut document = Document::with_units(tolerance, LengthUnitSystem::Inches).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut document, &format!("ImportStl {}", path.0.display()))
        .unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        document.objects().next().unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    let imported = document.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 0);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), imported);
    assert_eq!(document.units(), &LengthUnitSystem::Inches);
    assert_eq!(document.tolerance(), tolerance);
}

struct TemporaryFile(std::path::PathBuf);
impl TemporaryFile {
    fn new() -> Self {
        Self::with_extension("3dm")
    }
    fn with_extension(extension: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "viboceros-unit-import-{}-{unique}.{extension}",
            std::process::id()
        )))
    }
}

#[test]
fn step_export_converts_document_units_without_editing_document_or_history() {
    let output = TemporaryFile::with_extension("step");
    let mut document = Document::with_units(Tolerance::DEFAULT, LengthUnitSystem::Inches).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut document, "Point 9,8,7").unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    let before = format!("{document:?}");
    registry
        .execute(&mut document, &format!("ExportStep {}", output.0.display()))
        .unwrap();
    assert_eq!(format!("{document:?}"), before);
    let imported = read_step_file(&output.0, Tolerance::DEFAULT).unwrap();
    assert_eq!(imported.objects.len(), 1);
    assert!(imported.objects[0].mesh.bounds().max().is_near(
        Point3::try_new(25.4, 50.8, 0.0).unwrap(),
        Tolerance::DEFAULT
    ));
}

#[test]
fn step_export_and_import_preserve_physical_size_in_a_nonmetric_document() {
    let path = TemporaryFile::with_extension("step");
    let registry = CommandRegistry::with_builtins();
    let mut source = Document::with_units(Tolerance::DEFAULT, LengthUnitSystem::Inches).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    source.add_geometry(Geometry::Mesh(mesh)).unwrap();
    registry
        .execute(&mut source, &format!("ExportStep {}", path.0.display()))
        .unwrap();
    let mut target = Document::with_units(Tolerance::DEFAULT, LengthUnitSystem::Inches).unwrap();
    registry
        .execute(&mut target, &format!("ImportStep {}", path.0.display()))
        .unwrap();
    let Geometry::Mesh(mesh) = target.objects().next().unwrap().geometry() else {
        panic!("lost mesh");
    };
    assert!(
        mesh.bounds()
            .max()
            .is_near(Point3::try_new(1.0, 2.0, 0.0).unwrap(), Tolerance::DEFAULT)
    );
    registry.execute(&mut target, "Undo").unwrap();
    assert_eq!(target.objects().len(), 0);
    registry.execute(&mut target, "Redo").unwrap();
    assert_eq!(target.objects().len(), 1);
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
