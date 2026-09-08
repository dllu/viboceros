use super::*;
use viboceros_geometry::LengthUnitSystem;

#[test]
fn extreme_unit_point_import_preserves_attributes_and_exact_undo_redo() {
    let registry = CommandRegistry::with_builtins();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("extreme point import.3dm");
    for (factor, coordinate, absolute) in [(1e-320, 1.0, 1e-9), (1e300, 1e-300, 1e-100)] {
        let mut source = Document::new(Tolerance::DEFAULT);
        let id = source
            .add_geometry(Geometry::Point(
                Point3::try_new(coordinate, 0.0, 0.0).unwrap(),
            ))
            .unwrap();
        source.add_group(Some("source group".into()), [id]).unwrap();
        let mut model = document_3dm_model(&source).unwrap();
        model.units = LengthUnitSystem::Custom {
            name: "extreme units".into(),
            meters_per_unit: factor,
        };
        model.objects[0].name = Some("extreme point".into());
        model.objects[0].object_color = [12, 34, 56];
        model.objects[0].color_source = viboceros_io::ThreeDmColorSource::Object;
        write_3dm_file(&path, &model).unwrap();
        let source_bytes = std::fs::read(&path).unwrap();
        let tolerance = Tolerance::try_new(absolute, 1e-12, 1e-10).unwrap();
        let mut target = Document::with_units(tolerance, LengthUnitSystem::Meters).unwrap();
        registry.execute(&mut target, "Point 7,8,9").unwrap();
        registry.execute(&mut target, "SelAll").unwrap();
        let before = document_3dm_model(&target).unwrap();
        let selection = target.selected_object_ids().collect::<Vec<_>>();
        registry
            .execute(&mut target, &format!("Import3dm {}", path.display()))
            .unwrap();
        let imported = target
            .objects()
            .find(|object| object.attributes().name() == Some("extreme point"))
            .unwrap();
        let imported_id = imported.id();
        assert_eq!(
            imported.geometry(),
            &Geometry::Point(Point3::try_new(coordinate * factor, 0.0, 0.0).unwrap())
        );
        assert_eq!(
            imported.attributes().object_color(),
            ColorRgb::new(12, 34, 56)
        );
        assert_eq!(
            imported.attributes().color_source(),
            ObjectColorSource::Object
        );
        assert_eq!(imported.group_ids().len(), 1);
        assert_eq!(
            target.group(imported.group_ids()[0]).unwrap().name(),
            Some("source group")
        );
        assert_eq!(target.units(), &LengthUnitSystem::Meters);
        assert_eq!(target.tolerance(), tolerance);
        let after = document_3dm_model(&target).unwrap();
        registry.execute(&mut target, "Undo").unwrap();
        assert_eq!(document_3dm_model(&target).unwrap(), before);
        assert_eq!(target.selected_object_ids().collect::<Vec<_>>(), selection);
        assert!(target.object(imported_id).is_none());
        registry.execute(&mut target, "Redo").unwrap();
        assert_eq!(document_3dm_model(&target).unwrap(), after);
        assert!(target.object(imported_id).is_some());
        assert_eq!(std::fs::read(&path).unwrap(), source_bytes);
    }
}

#[test]
fn unrepresentable_brep_import_tolerance_preserves_document_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mixed extreme import.3dm");
    let mut source = Document::new(Tolerance::DEFAULT);
    registry.execute(&mut source, "Point 1,2,3").unwrap();
    registry.execute(&mut source, "Box 0,0,0 1,1,0 1").unwrap();
    let mut model = document_3dm_model(&source).unwrap();
    assert!(
        model
            .objects
            .iter()
            .any(|object| matches!(object.geometry, ThreeDmGeometry::Brep(_)))
    );
    for (factor, absolute) in [(1e-320, 1e-9), (1e300, 1e-100)] {
        model.units = LengthUnitSystem::Custom {
            name: "extreme units".into(),
            meters_per_unit: factor,
        };
        write_3dm_file(&path, &model).unwrap();
        let source_bytes = std::fs::read(&path).unwrap();
        let mut target = Document::with_units(
            Tolerance::try_new(absolute, 1e-12, 1e-10).unwrap(),
            LengthUnitSystem::Meters,
        )
        .unwrap();
        registry.execute(&mut target, "Point 7,8,9").unwrap();
        registry.execute(&mut target, "Point 10,11,12").unwrap();
        registry.execute(&mut target, "Undo").unwrap();
        let before = format!("{target:?}");
        assert!(matches!(
            registry.execute(&mut target, &format!("Import3dm {}", path.display())),
            Err(CommandError::ThreeDm(
                ThreeDmError::UnrepresentableSourceTolerance
            ))
        ));
        assert_eq!(format!("{target:?}"), before);
        registry.execute(&mut target, "Redo").unwrap();
        assert_eq!(target.objects().len(), 2);
        assert_eq!(std::fs::read(&path).unwrap(), source_bytes);
    }
}

#[test]
fn mesh_export_commands_preserve_small_geometry_without_changing_history() {
    let mut document = Document::new(Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap());
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1e-5, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1e-5, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap();
    document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut document, "Point 1,2,3").unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    let before = format!("{document:?}");
    for (command, extension) in [
        ("ExportStl Ascii", "stl"),
        ("ExportStl Binary", "stl"),
        ("ExportStep", "step"),
    ] {
        let path = TemporaryFile::with_extension(extension);
        registry
            .execute(&mut document, &format!("{command} {}", path.path.display()))
            .unwrap();
        assert_eq!(format!("{document:?}"), before);
        assert!(std::fs::metadata(&path.path).unwrap().len() > 0);
        let mesh = if extension == "stl" {
            read_stl_file(&path.path).unwrap()
        } else {
            let mut imported = read_step_file(&path.path, Tolerance::NUMERICAL_VALIDATION).unwrap();
            assert_eq!(imported.objects.len(), 1);
            imported.objects.remove(0).mesh
        };
        assert_eq!(mesh.triangles().len(), 1);
        assert!(mesh.bounds().max().is_near(
            Point3::try_new(1e-5, 1e-5, 0.0).unwrap(),
            Tolerance::try_new(1e-11, 1e-12, 1e-10).unwrap()
        ));
    }
}

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
    write_stl_file(&path.path, &mesh, StlFormat::Ascii).unwrap();
    let tolerance = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    let mut document = Document::with_units(tolerance, LengthUnitSystem::Inches).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut document, &format!("ImportStl {}", path.path.display()))
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

struct TemporaryFile {
    path: std::path::PathBuf,
    _directory: tempfile::TempDir,
}
impl TemporaryFile {
    fn new() -> Self {
        Self::with_extension("3dm")
    }
    fn with_extension(extension: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        Self {
            path: directory.path().join(format!("unit import.{extension}")),
            _directory: directory,
        }
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
        .execute(
            &mut document,
            &format!("ExportStep {}", output.path.display()),
        )
        .unwrap();
    assert_eq!(format!("{document:?}"), before);
    let imported = read_step_file(&output.path, Tolerance::DEFAULT).unwrap();
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
        .execute(&mut source, &format!("ExportStep {}", path.path.display()))
        .unwrap();
    let mut target = Document::with_units(Tolerance::DEFAULT, LengthUnitSystem::Inches).unwrap();
    registry
        .execute(&mut target, &format!("ImportStep {}", path.path.display()))
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

#[test]
fn unit_aware_import_is_undoable_and_export_retains_target_units() {
    let input = TemporaryFile::new();
    let output = TemporaryFile::new();
    let mut source = Document::with_units(Tolerance::DEFAULT, LengthUnitSystem::Inches).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut source, "Point 1,2,3").unwrap();
    write_3dm_file(&input.path, &document_3dm_model(&source).unwrap()).unwrap();
    let mut target =
        Document::with_units(Tolerance::DEFAULT, LengthUnitSystem::Centimeters).unwrap();
    registry
        .execute(&mut target, &format!("Import3dm {}", input.path.display()))
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
        .execute(&mut target, &format!("Export3dm {}", output.path.display()))
        .unwrap();
    let exported = read_3dm_file(&output.path, Tolerance::DEFAULT).unwrap();
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
    write_3dm_file(&input.path, &source_model).unwrap();
    let mut empty =
        Document::with_units(Tolerance::DEFAULT, LengthUnitSystem::Centimeters).unwrap();
    // IDs are document-specific; compare this document against its own state.
    let empty_before = format!("{empty:?}");
    assert!(
        registry
            .execute(&mut empty, &format!("Import3dm {}", input.path.display()))
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
    write_3dm_file(&input.path, &source_model).unwrap();
    assert!(
        registry
            .execute(&mut empty, &format!("Import3dm {}", input.path.display()))
            .is_err()
    );
    assert_eq!(format!("{empty:?}"), empty_before);
}
