//! Apply flattened block placements with the native checked affine kernel.
use super::*;
use viboceros_geometry::AffineTransform3;

pub(super) fn apply(
    handle: &ModelHandle,
    index: usize,
    geometry: ThreeDmGeometry,
    tolerance: Tolerance,
    coordinate_scale: f64,
) -> Result<ThreeDmGeometry, ThreeDmError> {
    // SAFETY: the live model handle and validated object index belong to this decode.
    let count = unsafe { ffi::vibo_3dm_object_placement_count(handle.0.as_ptr(), index) };
    if count == 0 {
        return Ok(geometry);
    }
    if count > 64 {
        return Err(ThreeDmError::MalformedBridge(
            "block placement depth exceeds limit",
        ));
    }
    let mut transform = AffineTransform3::identity();
    for placement in (0..count).rev() {
        let mut matrix = [0.; 16];
        // SAFETY: matrix has 16 writable doubles; the live model owns the placement chain.
        let success = unsafe {
            ffi::vibo_3dm_object_placement(handle.0.as_ptr(), index, placement, matrix.as_mut_ptr())
        };
        if success == 0 {
            return Err(ThreeDmError::MalformedBridge("missing block placement"));
        }
        transform = transform.then(affine(matrix)?).map_err(|e| {
            ThreeDmError::InvalidModel(format!("invalid block placement composition: {e}"))
        })?;
    }
    let matching = if matches!(geometry, ThreeDmGeometry::Brep(_)) {
        Tolerance::try_new(
            tolerance.absolute() / coordinate_scale,
            tolerance.relative(),
            tolerance.angular(),
        )
        .map_err(|_| ThreeDmError::UnrepresentableSourceTolerance)?
    } else {
        Tolerance::NUMERICAL_VALIDATION
    };
    crate::three_dm_units::transform_geometry(&geometry, transform, matching)
        .map_err(|e| ThreeDmError::InvalidModel(format!("invalid placed block geometry: {e}")))
}

fn affine(matrix: [f64; 16]) -> Result<AffineTransform3, ThreeDmError> {
    if matrix.iter().any(|x| !x.is_finite()) || matrix[12..] != [0., 0., 0., 1.] {
        return Err(ThreeDmError::InvalidModel(
            "block placement is not finite affine data".into(),
        ));
    }
    Ok(AffineTransform3::try_new(
        std::array::from_fn(|r| std::array::from_fn(|c| matrix[r * 4 + c])),
        Vector3::try_new(matrix[3], matrix[7], matrix[11])?,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_matrix_preserves_noncommuting_order_and_rejects_projective_values() {
        let child = affine([
            2., 0., 0., 1., 0., 3., 0., 2., 0., 0., 4., 3., 0., 0., 0., 1.,
        ])
        .unwrap();
        let parent = affine([
            0., -1., 0., 10., 1., 0., 0., 20., 0., 0., 1., 30., 0., 0., 0., 1.,
        ])
        .unwrap();
        let p = Point3::try_new(1., 2., 3.).unwrap();
        assert_eq!(
            child
                .then(parent)
                .unwrap()
                .transform_point(p)
                .unwrap()
                .to_array(),
            [2., 23., 45.]
        );
        assert_ne!(child.then(parent).unwrap(), parent.then(child).unwrap());
        for coefficient in [0.25, f64::INFINITY, f64::NAN] {
            let mut m = [
                1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
            ];
            m[12] = coefficient;
            assert!(affine(m).is_err());
        }
    }

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/instances")
            .join(name)
    }
    #[test]
    fn nested_and_repeated_blocks_expand_at_placed_coordinates_with_inherited_attributes() {
        let model = read_3dm_file(fixture("nested_blocks.3dm"), Tolerance::DEFAULT).unwrap();
        assert_eq!(model.unsupported_object_count(), 0);
        assert_eq!(model.expanded_instance_count(), 2);
        assert_eq!(model.objects.len(), 9);
        assert_eq!(
            model
                .objects
                .iter()
                .map(|o| o.name.as_deref().unwrap())
                .collect::<Vec<_>>(),
            [
                "point", "line", "cloud", "box", "point", "line", "cloud", "box", "ordinary"
            ]
        );
        assert_eq!(
            model.objects[0].geometry,
            ThreeDmGeometry::Point(Point3::try_new(2., 23., 45.).unwrap())
        );
        assert_eq!(
            model.objects[4].geometry,
            ThreeDmGeometry::Point(Point3::try_new(-9., -18., -27.).unwrap())
        );
        let ThreeDmGeometry::Line(line) = &model.objects[1].geometry else {
            panic!()
        };
        assert_eq!(line.start().to_array(), [8., 21., 33.]);
        assert_eq!(line.end().to_array(), [5., 23., 33.]);
        let ThreeDmGeometry::PointCloud(cloud) = &model.objects[2].geometry else {
            panic!()
        };
        assert_eq!(cloud.points()[1].to_array(), [2., 23., 33.]);
        assert_eq!(
            cloud.channels().colors.as_ref().unwrap(),
            &[[12, 34, 56, 0], [65, 43, 21, 0]]
        );
        let ThreeDmGeometry::Brep(brep) = &model.objects[3].geometry else {
            panic!()
        };
        assert_eq!(brep.faces().len(), 6);
        assert_eq!(brep.edges().len(), 12);
        assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - 24.).abs() < 1e-10);
        for object in &model.objects[..4] {
            assert_eq!(object.object_color, [100, 110, 120]);
            assert_eq!(object.color_source, ThreeDmColorSource::Object);
            assert_eq!(object.group_indices, &[0]);
        }
        for object in &model.objects[4..8] {
            assert_eq!(object.object_color, [5, 10, 15]);
            assert_eq!(object.color_source, ThreeDmColorSource::Object);
        }
        assert_eq!(
            model.objects[8].geometry,
            ThreeDmGeometry::Point(Point3::try_new(99., 98., 97.).unwrap())
        );
        // Definitions are not emitted at their unplaced coordinates.
        assert!(!model.objects.iter().any(|o|o.geometry==ThreeDmGeometry::Point(Point3::try_new(1.,2.,3.).unwrap())));
        let meters = read_3dm_file_in_units(
            fixture("nested_blocks.3dm"),
            &LengthUnitSystem::Meters,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            meters.objects[0].geometry,
            ThreeDmGeometry::Point(Point3::try_new(0.002, 0.023, 0.045).unwrap())
        );
    }
    #[test]
    fn hidden_locked_instances_propagate_to_members_and_bad_graphs_fail_read() {
        let model = read_3dm_file(fixture("hidden_blocks.3dm"), Tolerance::DEFAULT).unwrap();
        assert_eq!(model.objects.len(), 5);
        for o in &model.objects[..4] {
            assert!(!o.visible);
            assert!(o.locked);
        }
        assert!(model.objects[4].visible);
        for name in ["missing_block.3dm", "cyclic_block.3dm"] {
            assert!(
                read_3dm_file(fixture(name), Tolerance::DEFAULT).is_err(),
                "{name}"
            );
        }
    }

    #[test]
    fn reflected_blocks_keep_boundary_correspondence_solid_volume_and_stored_cloud_normals() {
        let source = read_3dm_file(fixture("nested_blocks.3dm"), Tolerance::DEFAULT).unwrap();
        let mirrored = read_3dm_file(fixture("reflected_blocks.3dm"), Tolerance::DEFAULT).unwrap();
        assert_eq!(source.objects.len(), mirrored.objects.len());
        assert_eq!(
            mirrored.objects[0].geometry,
            ThreeDmGeometry::Point(Point3::try_new(-2., 23., 45.).unwrap())
        );
        let ThreeDmGeometry::Brep(b) = &mirrored.objects[3].geometry else {
            panic!()
        };
        let ThreeDmGeometry::Brep(original) = &source.objects[3].geometry else {
            panic!()
        };
        assert!((b.signed_volume(Tolerance::DEFAULT).unwrap() - 24.).abs() < 1e-10);
        for (a, b) in b.vertices().iter().zip(original.vertices()) {
            assert_eq!(
                a.point().to_array(),
                [-b.point().x(), b.point().y(), b.point().z()]
            );
        }
        for face in b.faces() {
            for trim in face.loops().iter().flat_map(|l| l.trims()) {
                if let Some(edge) = trim.edge() {
                    let c = b.edges()[edge].curve();
                    let c = if trim.is_reversed_3d() {
                        c.reversed().unwrap()
                    } else {
                        c.clone()
                    };
                    assert!(
                        face.surface()
                            .parameter_curve_deviation_bound(
                                trim.curve(),
                                &c,
                                Tolerance::DEFAULT.absolute()
                            )
                            .unwrap()
                            .is_some()
                    );
                }
            }
        }
        let ThreeDmGeometry::PointCloud(c) = &mirrored.objects[2].geometry else {
            panic!()
        };
        assert_eq!(
            c.channels()
                .normals
                .as_ref()
                .unwrap()
                .iter()
                .map(|n| n.to_array())
                .collect::<Vec<_>>(),
            [[0., 0., 1.], [0., 1., 0.]]
        );
        assert_eq!(source.objects[4..], mirrored.objects[4..]);
    }
    #[test]
    fn invalid_placements_and_empty_branching_graphs_fail_before_partial_results() {
        for (name, reason) in [
            ("projective_block.3dm", "finite affine"),
            ("singular_block.3dm", "placed block geometry"),
            ("branching_empty_blocks.3dm", "1000000 visits"),
        ] {
            let error = read_3dm_file(fixture(name), Tolerance::DEFAULT).unwrap_err();
            assert!(error.to_string().contains(reason), "{name}: {error}");
        }
    }
}
