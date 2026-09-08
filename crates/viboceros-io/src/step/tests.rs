use std::io::Cursor;

use super::*;

fn unit_test_mesh() -> TriangleMesh {
    TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn unit_aware_step_export_matches_its_millimetre_declaration() {
    let mesh = unit_test_mesh();
    let original = mesh.clone();
    for (units, factor) in [
        (LengthUnitSystem::Millimeters, 1.0),
        (LengthUnitSystem::Meters, 1000.0),
        (LengthUnitSystem::Inches, 25.4),
        (
            LengthUnitSystem::Custom {
                name: "eighth-metre".into(),
                meters_per_unit: 0.125,
            },
            125.0,
        ),
    ] {
        let mut bytes = Vec::new();
        write_step_in_units(
            &mut bytes,
            std::slice::from_ref(&mesh),
            &units,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("SI_UNIT(.MILLI.,.METRE.)"));
        let imported = read_step(Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
        assert_eq!(imported.objects.len(), 1);
        let mesh = &imported.objects[0].mesh;
        assert_eq!(mesh.triangles().len(), 1);
        assert!(
            mesh.bounds()
                .min()
                .is_near(Point3::try_new(0.0, 0.0, 0.0).unwrap(), Tolerance::DEFAULT)
        );
        assert!(mesh.bounds().max().is_near(
            Point3::try_new(factor, factor * 2.0, 0.0).unwrap(),
            Tolerance::DEFAULT
        ));
    }
    assert_eq!(mesh, original);
}

#[test]
fn invalid_export_units_preserve_the_destination_and_stream() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.step");
    std::fs::write(&path, b"original").unwrap();
    let mesh = unit_test_mesh();
    for units in [
        LengthUnitSystem::None,
        LengthUnitSystem::Unset,
        LengthUnitSystem::Custom {
            name: "invalid".into(),
            meters_per_unit: f64::NAN,
        },
        LengthUnitSystem::Custom {
            name: "overflow".into(),
            meters_per_unit: f64::MAX,
        },
    ] {
        assert!(
            write_step_file_in_units(
                &path,
                std::slice::from_ref(&mesh),
                &units,
                Tolerance::DEFAULT
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        let mut stream = b"original".to_vec();
        assert!(
            write_step_in_units(
                &mut stream,
                std::slice::from_ref(&mesh),
                &units,
                Tolerance::DEFAULT
            )
            .is_err()
        );
        assert_eq!(stream, b"original");
    }
}

#[test]
fn unit_aware_export_scales_validation_and_declared_accuracy() {
    let mesh = unit_test_mesh()
        .transformed(
            AffineTransform3::try_uniform_scale(Point3::try_new(0.0, 0.0, 0.0).unwrap(), 1e-5)
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
    let mut bytes = Vec::new();
    write_step_in_units(
        &mut bytes,
        &[mesh],
        &LengthUnitSystem::Nanometers,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let table = Table::from_step(text).unwrap();
    assert!(table.cartesian_point.values().any(|point| {
        point
            .coordinates
            .first()
            .is_some_and(|x| (*x - 1e-11).abs() < 1e-26)
    }));
    // The uncertainty value is in the same millimetres as the coordinates.
    let accuracy = text
        .split("LENGTH_MEASURE(")
        .nth(1)
        .unwrap()
        .split(')')
        .next()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((accuracy - 1e-15).abs() < 1e-30);
}
use monstertruck::modeling::{BoundingBox, Point3 as TruckPoint3, primitive};
use monstertruck::step::save::{
    CompleteStepDisplay, StepHeaderDescriptor, StepModel as TruckStepModel,
};

fn cube_step() -> String {
    let cube: monstertruck::modeling::Solid = primitive::cuboid(BoundingBox::from_iter([
        TruckPoint3::new(-1.0, -2.0, -3.0),
        TruckPoint3::new(4.0, 5.0, 6.0),
    ]));
    let compressed = cube.compress();
    CompleteStepDisplay::new(
        TruckStepModel::from(&compressed),
        StepHeaderDescriptor {
            organization_system: "Viboceros test".to_owned(),
            ..Default::default()
        },
    )
    .to_string()
}

#[test]
fn imports_an_analytic_step_solid_as_a_validated_mesh() {
    let model = read_step(Cursor::new(cube_step()), Tolerance::DEFAULT).unwrap();

    assert_eq!(model.objects.len(), 1);
    let mesh = &model.objects[0].mesh;
    assert_eq!(mesh.triangles().len(), 12);
    assert!(mesh.bounds().min().is_near(
        Point3::try_new(-1.0, -2.0, -3.0).unwrap(),
        Tolerance::DEFAULT
    ));
    assert!(
        mesh.bounds()
            .max()
            .is_near(Point3::try_new(4.0, 5.0, 6.0).unwrap(), Tolerance::DEFAULT)
    );
    assert_eq!(model.report.swallowed_entity_count, 0);
    assert_eq!(model.report.lost_topology_item_count, 0);
}

fn assembly_step(parent_transform: Option<Matrix4>) -> String {
    use monstertruck::assembly::assy::{Assembly, EdgeEntity, NodeEntity};
    use monstertruck::modeling::Vector3;
    use monstertruck::step::{common::PartAttributes, save::StepDesign};

    // Adapt the standalone writer's data section into an indexed shape.
    // The assembly emitter requires the solid entity at its first index.
    struct IndexedSolid {
        data: String,
        solid: usize,
        length: usize,
    }
    impl monstertruck::step::save::StepLength for IndexedSolid {
        fn step_length(&self) -> usize {
            self.length
        }
    }
    impl monstertruck::step::save::StepFormat for IndexedSolid {
        fn fmt(&self, idx: usize, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let mut pieces = self.data.split('#');
            write!(f, "{}", pieces.next().unwrap())?;
            for piece in pieces {
                let digits = piece.bytes().take_while(u8::is_ascii_digit).count();
                let original: usize = piece[..digits].parse().unwrap();
                let mapped = if original == self.solid { 0 } else { original };
                write!(f, "#{}{}", idx + mapped, &piece[digits..])?;
            }
            Ok(())
        }
    }
    let cube = cube_step();
    let table = Table::from_step(&cube).unwrap();
    let solid = *table.manifold_solid_brep.keys().next().unwrap() as usize;
    let data = cube
        .split("DATA;")
        .nth(1)
        .unwrap()
        .split("ENDSEC;")
        .next()
        .unwrap()
        .to_owned();
    // Discard the standalone product/context: only the geometry belongs
    // in the new assembly, otherwise it creates an extra root instance.
    let data = data[data.find(&format!("#{solid} =")).unwrap()..].to_owned();
    let length = data
        .split('#')
        .skip(1)
        .map(|piece| {
            let digits = piece.bytes().take_while(u8::is_ascii_digit).count();
            piece[..digits].parse::<usize>().unwrap()
        })
        .max()
        .unwrap()
        + 1;
    let mut assembly = Assembly::new();
    let root = assembly.create_node(NodeEntity {
        shape: None,
        attrs: PartAttributes::default(),
    });
    let parent = if let Some(matrix) = parent_transform {
        let parent = assembly.create_node(NodeEntity {
            shape: None,
            attrs: PartAttributes::default(),
        });
        assembly.create_edge(
            root,
            parent,
            EdgeEntity {
                matrix,
                attrs: PartAttributes {
                    name: "parent".to_owned(),
                    ..Default::default()
                },
            },
        );
        parent
    } else {
        root
    };
    let part = assembly.create_node(NodeEntity {
        shape: Some(IndexedSolid {
            data,
            solid,
            length,
        }),
        attrs: PartAttributes::default(),
    });
    for index in 0..3 {
        assembly.create_edge(
            parent,
            part,
            EdgeEntity {
                matrix: Matrix4::from_translation(Vector3::new(10.0 * index as f64, 0.0, 0.0)),
                attrs: PartAttributes {
                    name: format!("instance {index}"),
                    ..Default::default()
                },
            },
        );
    }
    CompleteStepDisplay::new(StepDesign::new(assembly), StepHeaderDescriptor::default()).to_string()
}

#[test]
fn repeated_assembly_instances_preserve_names_and_transforms() {
    let text = assembly_step(None);
    let table = Table::from_step(&text).unwrap();
    assert_eq!(table.manifold_solid_brep.len(), 1);
    let imported = read_step(Cursor::new(text), Tolerance::DEFAULT).unwrap();
    assert_eq!(imported.objects.len(), 3);
    assert_eq!(imported.report.unplaced_shape_count, 0);
    assert!(imported.report.assembly_warning.is_none());
    assert_eq!(imported.report.swallowed_entity_count, 0);
    assert_eq!(imported.report.lost_topology_item_count, 0);
    for index in 0..3 {
        let name = format!("instance {index}");
        let mesh = &imported
            .objects
            .iter()
            .find(|object| object.name.as_deref() == Some(&name))
            .unwrap()
            .mesh;
        assert_eq!(mesh.triangles().len(), 12);
        assert!(mesh.bounds().min().is_near(
            Point3::try_new(-1.0 + 10.0 * index as f64, -2.0, -3.0).unwrap(),
            Tolerance::DEFAULT
        ));
        assert!(mesh.bounds().max().is_near(
            Point3::try_new(4.0 + 10.0 * index as f64, 5.0, 6.0).unwrap(),
            Tolerance::DEFAULT
        ));
    }
}

#[test]
fn nested_assembly_composes_noncommuting_transforms_parent_first() {
    // Parent maps (x, y, z) to (100-y, 200+x, 300+z).
    // Child translates by (10*i, 0, 0) in that parent's local frame.
    let parent = Matrix4::new(
        0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 100.0, 200.0, 300.0, 1.0,
    );
    let text = assembly_step(Some(parent));
    let imported = read_step(Cursor::new(text), Tolerance::DEFAULT).unwrap();
    assert_eq!(imported.objects.len(), 3);
    assert_eq!(imported.report.unplaced_shape_count, 0);
    assert!(imported.report.assembly_warning.is_none());
    for index in 0..3 {
        let name = format!("instance {index}");
        let mesh = &imported
            .objects
            .iter()
            .find(|object| object.name.as_deref() == Some(&name))
            .unwrap()
            .mesh;
        let offset = 10.0 * index as f64;
        assert!(mesh.bounds().min().is_near(
            Point3::try_new(95.0, 199.0 + offset, 297.0).unwrap(),
            Tolerance::DEFAULT
        ));
        assert!(mesh.bounds().max().is_near(
            Point3::try_new(102.0, 204.0 + offset, 306.0).unwrap(),
            Tolerance::DEFAULT
        ));
        assert_eq!(mesh.triangles().len(), 12);
        // Every vertex must remain a transformed corner, not merely lie in
        // a correct bounding box. Expected coordinates use no matrix code.
        for point in mesh.vertices() {
            for (coordinate, choices) in [
                (point.x(), [95.0, 102.0]),
                (point.y(), [199.0 + offset, 204.0 + offset]),
                (point.z(), [297.0, 306.0]),
            ] {
                assert!(
                    choices
                        .iter()
                        .any(|expected| (coordinate - expected).abs() < 1.0e-8)
                );
            }
        }
    }
}

#[test]
fn cached_source_is_reused_but_each_instance_is_validated() {
    let mut table = Table::from_step(&cube_step()).unwrap();
    let shape = *table.manifold_solid_brep.keys().next().unwrap();
    let mut report = StepImportReport::default();
    let mut cache = BTreeMap::new();
    let original = import_shape(
        &table,
        shape,
        Matrix4::identity(),
        Tolerance::DEFAULT,
        &mut report,
        &mut cache,
    )
    .unwrap()
    .unwrap();
    // Removing the source makes any accidental second tessellation observable.
    table.manifold_solid_brep.remove(&shape);
    let enlarged = import_shape(
        &table,
        shape,
        Matrix4::from_scale(2.0),
        Tolerance::DEFAULT,
        &mut report,
        &mut cache,
    )
    .unwrap()
    .unwrap();
    assert_eq!(enlarged.triangles(), original.triangles());
    for (actual, source) in enlarged.vertices().iter().zip(original.vertices()) {
        assert_eq!(
            *actual,
            Point3::try_new(2.0 * source.x(), 2.0 * source.y(), 2.0 * source.z()).unwrap()
        );
    }
    assert!(
        import_shape(
            &table,
            shape,
            Matrix4::from_scale(0.0),
            Tolerance::DEFAULT,
            &mut report,
            &mut cache
        )
        .is_err()
    );
    assert!(
        import_shape(
            &table,
            shape,
            Matrix4::identity(),
            Tolerance::DEFAULT,
            &mut report,
            &mut cache
        )
        .unwrap()
        .is_some()
    );
}

#[test]
fn exported_mesh_round_trips_as_a_shared_edge_step_shell() {
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 3.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut bytes = Vec::new();
    write_step(&mut bytes, std::slice::from_ref(&mesh)).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.starts_with("ISO-10303-21;"));
    assert!(text.contains("SHELL_BASED_SURFACE_MODEL"));
    assert_eq!(text.matches("EDGE_CURVE(").count(), 5);
    assert_eq!(text.matches("ADVANCED_FACE(").count(), 2);

    let imported = read_step(Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
    assert_eq!(imported.objects.len(), 1);
    assert_eq!(imported.objects[0].mesh.triangles().len(), 2);
    assert_eq!(imported.objects[0].mesh.bounds(), mesh.bounds());
}

#[test]
fn failed_step_export_does_not_replace_an_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.step");
    std::fs::write(&path, b"keep me").unwrap();

    assert!(matches!(
        write_step_file(&path, &[]),
        Err(StepError::NoMeshesToWrite)
    ));
    assert_eq!(std::fs::read(path).unwrap(), b"keep me");
}

#[test]
fn rejects_invalid_and_geometry_free_step_data() {
    assert!(matches!(
        read_step(Cursor::new(b"not STEP"), Tolerance::DEFAULT),
        Err(StepError::Load(_))
    ));

    let empty = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('','','',(''),(''),'','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";
    assert!(matches!(
        read_step(Cursor::new(empty), Tolerance::DEFAULT),
        Err(StepError::NoSupportedGeometry)
    ));
}
