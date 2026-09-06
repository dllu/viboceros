use super::*;
use crate::construction_plane::WorldPlane;

fn p(a: [f64; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}
fn near(actual: Point3, expected: Point3) {
    assert!(
        actual.distance_to(expected).unwrap() < 1e-8,
        "{actual:?} != {expected:?}"
    );
}
fn quadratic() -> NurbsCurve {
    NurbsCurve::try_new(
        2,
        [[1., 2., 3.], [8., 16., 9.], [10., 4., 5.]].map(p).to_vec(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn unit_cells_follow_all_plane_axes_without_using_the_plane_origin() {
    let registry = CommandRegistry::with_builtins();
    let oblique = Frame3::try_from_directions(
        p([1e100, -1e100, 1e100]),
        Vector3::try_new(1., 2., 3.).unwrap(),
        Vector3::try_new(-4., 8., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for frame in WorldPlane::ALL
        .map(WorldPlane::frame)
        .into_iter()
        .chain([oblique])
    {
        let mut document = Document::default();
        let original = document
            .add_geometry(Geometry::Point(p([1., 2., 3.])))
            .unwrap();
        document.select_all();
        registry
            .execute_in_context(
                &mut document,
                "Array 2 3 2 3 -4 5",
                CommandContext {
                    construction_plane: frame,
                },
            )
            .unwrap();
        let mut expected = Vec::new();
        for z in 0..2 {
            for y in 0..3 {
                for x in 0..2 {
                    expected.push(
                        p([1., 2., 3.])
                            .translated(
                                frame
                                    .vector_at([
                                        3. * f64::from(x),
                                        -4. * f64::from(y),
                                        5. * f64::from(z),
                                    ])
                                    .unwrap(),
                            )
                            .unwrap(),
                    );
                }
            }
        }
        for (actual, expected) in document.objects().zip(expected) {
            let Geometry::Point(actual) = actual.geometry() else {
                panic!("point")
            };
            near(*actual, expected);
        }
        assert_eq!(document.objects().len(), 12);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            [original]
        );
        document.undo().unwrap();
        assert_eq!(document.objects().len(), 1);
        document.redo().unwrap();
        assert_eq!(document.objects().len(), 12);
    }
}

#[test]
fn fill_uses_tight_curve_extents_and_observed_signed_length_rules() {
    let bounds = BoundingBox3::from_points([p([0.; 3]), p([5., 4., 3.])]).unwrap();
    assert_eq!(
        rectangular_fill_spacing(bounds, [3, 3, 3], [40., 50., 60.]).unwrap(),
        [17.5, 23., 28.5]
    );
    assert_eq!(
        rectangular_fill_spacing(bounds, [3, 3, 3], [-40., -50., -60.]).unwrap(),
        [-22.5, -27., -31.5]
    );
    assert_eq!(
        rectangular_fill_spacing(bounds, [3, 3, 3], [2., 2., 2.]).unwrap(),
        [1.5, 1., 0.5]
    );
    assert_eq!(
        rectangular_fill_spacing(bounds, [3, 3, 3], [5., 4., 3.]).unwrap(),
        [0.; 3]
    );
    let mut document = Document::default();
    document
        .add_geometry(Geometry::NurbsCurve(quadratic()))
        .unwrap();
    document.select_all();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Array 1 2 1 0 20 0 Mode=Fill")
        .unwrap();
    let Geometry::NurbsCurve(copy) = document.objects().nth(1).unwrap().geometry() else {
        panic!("curve")
    };
    near(copy.evaluate(0.).unwrap(), p([1., 22. - 98. / 13., 3.]));
}

#[test]
fn nonrotating_polar_arrays_use_world_tight_bounds_but_plane_normal_offsets() {
    let mut document = Document::default();
    let source = quadratic();
    let original = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    document.select_all();
    let frame = WorldPlane::Front.frame().with_origin(p([100., 200., 300.]));
    CommandRegistry::with_builtins()
        .execute_in_context(
            &mut document,
            "ArrayPolar 3 0,0,0 180 Rotate=No ZOffset=2",
            CommandContext {
                construction_plane: frame,
            },
        )
        .unwrap();
    let anchor = p([5.5, 75. / 13., 4.8]);
    for (i, obj) in document.objects().enumerate() {
        let angle = std::f64::consts::FRAC_PI_2 * i as f64;
        let rotation = AffineTransform3::try_rotation(p([0.; 3]), frame.z_axis(), angle).unwrap();
        let destination = rotation
            .transform_point(anchor)
            .unwrap()
            .translated(frame.z_axis().as_vector().scaled(2. * i as f64).unwrap())
            .unwrap();
        let delta = anchor.vector_to(destination).unwrap();
        let Geometry::NurbsCurve(actual) = obj.geometry() else {
            panic!("curve")
        };
        for t in [0., 0.1, 0.5, 0.8, 1.] {
            near(
                actual.evaluate(t).unwrap(),
                source.evaluate(t).unwrap().translated(delta).unwrap(),
            );
        }
    }
    assert!(document.is_selected(original));
}

#[test]
fn rectangular_arrays_omit_zero_displacement_cells_without_deduplicating_other_copies() {
    let registry = CommandRegistry::with_builtins();
    for (command, count) in [
        ("Array 3 2 2 0 0 0", 2),
        ("Array 3 2 2 5 4 5 Mode=Fill", 2),
        ("Array 3 2 2 0 40 50", 20),
        ("Array 3 2 2 5 40 50 Mode=Fill", 20),
        ("Array 3 2 2 5 4 50 Mode=Fill", 14),
    ] {
        let mut document = Document::default();
        for point in [[0.; 3], [5., 4., 5.]] {
            document.add_geometry(Geometry::Point(p(point))).unwrap();
        }
        document.select_all();
        let originals = document.selected_object_ids().collect::<Vec<_>>();
        let before = document.clone();
        registry.execute(&mut document, command).unwrap();
        assert_eq!(document.objects().len(), count, "{command}");
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            originals
        );
        if count == 2 {
            assert_eq!(
                document.objects().collect::<Vec<_>>(),
                before.objects().collect::<Vec<_>>()
            );
            assert_eq!(document.undo_label(), before.undo_label());
            assert_eq!(document.redo_label(), before.redo_label());
        } else {
            document.undo().unwrap();
            assert_eq!(document.objects().len(), 2);
            document.redo().unwrap();
            assert_eq!(document.objects().len(), count);
        }
    }
}

#[test]
fn single_source_arrays_leave_copies_ungrouped_but_allocate_empty_definitions() {
    for command in [
        "Array 3 1 1 4 0 0",
        "ArrayLinear 3 0,0,0 4,5,6",
        "ArrayPolar 3 0,0,0 180",
    ] {
        for count in [1, 2] {
            let mut document = Document::default();
            let mut originals = Vec::new();
            for i in 0..count {
                let id = document
                    .add_geometry(Geometry::Point(p([i as f64, 2., 3.])))
                    .unwrap();
                document
                    .add_group(Some(format!("single-{i}")), [id])
                    .unwrap();
                originals.push(id);
            }
            if count == 2 {
                document
                    .add_group(Some("both".into()), originals.iter().copied())
                    .unwrap();
            }
            document.select_all();
            let before = document.groups().len();
            CommandRegistry::with_builtins()
                .execute(&mut document, command)
                .unwrap();
            assert_eq!(document.groups().len(), before * 3);
            for group in document.groups().skip(before) {
                assert_eq!(group.members().len() == 0, count == 1);
            }
            if count == 1 {
                assert!(
                    document
                        .objects()
                        .skip(count)
                        .all(|object| object.group_ids().is_empty())
                );
            }
            assert_eq!(document.objects().len(), count * 3);
            for id in &originals {
                assert!(document.is_selected(*id));
            }
            document.undo().unwrap();
            assert_eq!(document.groups().len(), before);
            assert_eq!(document.objects().len(), count);
            document.redo().unwrap();
            assert_eq!(document.groups().len(), before * 3);
        }
    }
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::Point(p([1., 2., 3.])))
        .unwrap();
    document
        .add_group(Some("ordinary copy".into()), [id])
        .unwrap();
    document
        .copy_objects_transformed(
            [id],
            AffineTransform3::from_translation(Vector3::try_new(1., 0., 0.).unwrap()),
        )
        .unwrap();
    assert_eq!(document.groups().len(), 2);
}

#[test]
fn failed_layout_bounds_cannot_partially_copy_the_document() {
    let curve = NurbsCurve::try_new_rational(
        2,
        [[0., 0., 0.], [1., 10., 0.], [2., 0., 0.]]
            .into_iter()
            .zip([1., -1., 1.])
            .map(|(coordinates, w)| WeightedPoint3::try_new(p(coordinates), w).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::NurbsCurve(curve.clone()))
        .unwrap();
    document.add_group(Some("source".into()), [id]).unwrap();
    document.select_all();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "Array 2 1 1 10 0 0 Mode=Fill",
        "ArrayPolar 3 0,0,0 180 Rotate=No",
    ] {
        assert!(
            CommandRegistry::with_builtins()
                .execute(&mut document, command)
                .is_err()
        );
        assert_eq!(document.objects().len(), 1);
        assert_eq!(document.groups().len(), 1);
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::NurbsCurve(curve.clone())
        );
        assert_eq!(document.undo_label(), history.as_deref());
        assert!(document.is_selected(id));
    }
}

#[test]
fn surface_arrays_use_interior_surface_extrema_and_fail_atomically_at_poles() {
    let surface = NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    p([
                        u as f64 / 2.,
                        v as f64 / 2.,
                        [0., 2., 0.][u] + [0., 4., 0.][v],
                    ])
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let context = CommandContext {
        construction_plane: WorldPlane::Front.frame(),
    };
    for (command, expected) in [
        ("Array 1 2 1 0 20 0 Mode=Fill", [0., 0., 17.]),
        (
            "ArrayPolar 3 0,0,0 180 Rotate=No ZOffset=2",
            [-2., -2., -1.],
        ),
    ] {
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::NurbsSurface(surface.clone()))
            .unwrap();
        document.select_all();
        CommandRegistry::with_builtins()
            .execute_in_context(&mut document, command, context)
            .unwrap();
        let Geometry::NurbsSurface(copy) = document.objects().nth(1).unwrap().geometry() else {
            panic!("surface")
        };
        near(copy.evaluate(0., 0.).unwrap(), p(expected));
        assert_eq!(copy.domain_u(), surface.domain_u());
        assert_eq!(copy.domain_v(), surface.domain_v());
        document.undo().unwrap();
        assert_eq!(document.objects().len(), 1);
        assert!(document.is_selected(id));
    }
    let controls = (0..2)
        .flat_map(|v| {
            (0..3).map(move |u| {
                WeightedPoint3::try_new(p([u as f64, v as f64, 0.]), [1., -1., 1.][u]).unwrap()
            })
        })
        .collect();
    let pole = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        controls,
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let mut document = Document::default();
    document.add_geometry(Geometry::NurbsSurface(pole)).unwrap();
    document.select_all();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "Array 2 1 1 10 0 0 Mode=Fill",
        "ArrayPolar 3 0,0,0 180 Rotate=No",
    ] {
        assert!(
            CommandRegistry::with_builtins()
                .execute_in_context(&mut document, command, context)
                .is_err()
        );
        assert_eq!(document.objects().len(), 1);
        assert_eq!(document.undo_label(), history.as_deref());
    }
}

#[test]
fn trimmed_brep_arrays_use_face_extrema_and_preserve_underlying_surfaces() {
    let surface = NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    p([
                        u as f64 / 2.,
                        v as f64 / 2.,
                        [0., 2., 0.][u] + [0., 4., 0.][v],
                    ])
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let brep = Brep::try_rectangular_surface_face(
        surface.clone(),
        0.2..=0.8,
        0.25..=0.75,
        Tolerance::DEFAULT,
    )
    .unwrap();
    for (command, expected) in [
        ("Array 1 2 1 0 20 0 Mode=Fill", [0., 0., 19.14]),
        (
            "ArrayPolar 3 0,0,0 180 Rotate=No ZOffset=2",
            [-3.07, -2., -2.07],
        ),
    ] {
        let mut document = Document::default();
        let id = document.add_geometry(Geometry::Brep(brep.clone())).unwrap();
        document.select_all();
        CommandRegistry::with_builtins()
            .execute_in_context(
                &mut document,
                command,
                CommandContext {
                    construction_plane: WorldPlane::Front.frame(),
                },
            )
            .unwrap();
        let Geometry::Brep(copy) = document.objects().nth(1).unwrap().geometry() else {
            panic!("B-rep")
        };
        near(
            copy.faces()[0].surface().evaluate(0., 0.).unwrap(),
            p(expected),
        );
        assert_eq!(copy.faces()[0].surface().domain_u(), surface.domain_u());
        assert_eq!(copy.faces()[0].surface().domain_v(), surface.domain_v());
        assert_eq!(copy.faces()[0].loops(), brep.faces()[0].loops());
        document.undo().unwrap();
        assert_eq!(document.objects().len(), 1);
        assert!(document.is_selected(id));
        document.redo().unwrap();
        assert!(document.objects().len() > 1);
    }
}

#[test]
fn ambiguous_tolerance_closed_brep_arrays_leave_geometry_selection_and_history_unchanged() {
    use viboceros_geometry::{BrepLoop, BrepTrim, NurbsCurve2, Point2};
    let surface = NurbsSurface::try_bilinear([
        p([0., 0., 0.]),
        p([1., 0., 0.]),
        p([1., 1., 0.]),
        p([0., 1., 0.]),
    ])
    .unwrap();
    let original = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
    let boundary = &original.faces()[0].loops()[0];
    let mut trims = boundary.trims().to_vec();
    let trim = &trims[0];
    let start = trim.curve().start_point().unwrap();
    let end = trim.curve().end_point().unwrap();
    let curve =
        NurbsCurve2::try_line(Point2::try_new(start.x() + 1e-12, start.y()).unwrap(), end).unwrap();
    trims[0] = BrepTrim::try_new(
        trim.vertices(),
        trim.edge(),
        trim.is_reversed_3d(),
        curve,
        trim.trim_type(),
        trim.iso(),
        trim.tolerance(),
    )
    .unwrap();
    let face = BrepFace::try_new(
        surface,
        false,
        vec![BrepLoop::try_new(boundary.loop_type(), trims).unwrap()],
    )
    .unwrap();
    let brep = Brep::try_new(
        original.vertices().to_vec(),
        original.edges().to_vec(),
        vec![face],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut document = Document::default();
    let id = document.add_geometry(Geometry::Brep(brep)).unwrap();
    document.select_all();
    let before = document.clone();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "Array 2 1 1 10 0 0 Mode=Fill",
        "ArrayPolar 3 0,0,0 180 Rotate=No",
    ] {
        assert!(
            CommandRegistry::with_builtins()
                .execute(&mut document, command)
                .is_err()
        );
        assert_eq!(document.objects().len(), 1);
        assert_eq!(document.object(id).unwrap(), before.object(id).unwrap());
        assert!(document.is_selected(id));
        assert_eq!(document.undo_label(), history.as_deref());
    }
}
