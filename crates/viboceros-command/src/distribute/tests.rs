use super::*;
use crate::{CommandRegistry, construction_plane::WorldPlane};
use viboceros_document::{Geometry, SelectionMode};
use viboceros_geometry::{NurbsCurve, PointCloud3, Vector3, WeightedPoint3};

#[test]
fn borrowed_units_preserve_first_appearance_top_membership_and_partial_selection() {
    let mut document = Document::default();
    let ids = (0..4)
        .map(|x| {
            document
                .add_geometry(Geometry::Point(p([x as f64, 0., 0.])))
                .unwrap()
        })
        .collect::<Vec<_>>();
    document.add_group(None, [ids[0], ids[1]]).unwrap();
    document.add_group(None, [ids[1], ids[2], ids[3]]).unwrap();
    for &id in &[ids[2], ids[0], ids[1]] {
        document
            .select_objects_direct([id], SelectionMode::Add)
            .unwrap();
    }
    let before = format!("{document:?}");
    let grouped = units(&document);
    assert_eq!(
        grouped
            .iter()
            .map(|objects| objects.iter().map(|object| object.id()).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        vec![vec![ids[2], ids[1]], vec![ids[0]]]
    );
    for object in grouped.into_iter().flatten() {
        assert!(std::ptr::eq(object, document.object(object.id()).unwrap()));
    }
    assert_eq!(distribution_unit_count(&document), 2);
    assert_eq!(format!("{document:?}"), before);
}

fn p(a: [f64; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}
fn cloud(points: &[[f64; 3]]) -> Geometry {
    Geometry::PointCloud(PointCloud3::try_new(points.iter().copied().map(p).collect()).unwrap())
}
fn selected(sources: Vec<Geometry>) -> Document {
    let mut document = Document::default();
    for source in sources {
        document.add_geometry(source).unwrap();
    }
    document.select_all();
    document
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} != {b}");
}

#[test]
fn nearly_normal_directions_do_not_apply_distance_tolerance_to_unit_frame_guides() {
    for x in [0., 5e-11, 2e-10, 2e-9] {
        let origin = p([0.; 3]);
        let vector = Vector3::try_new(x, 0., 1.).unwrap();
        let frame = direction_frame(
            Direction::Points(origin, origin.translated(vector).unwrap()),
            CommandContext::default().construction_plane,
            origin,
            Tolerance::DEFAULT,
        )
        .unwrap();
        near(
            frame
                .x_axis()
                .as_vector()
                .dot(vector.normalized_nonzero().unwrap().as_vector())
                .unwrap(),
            1.,
        );
        near(
            frame
                .x_axis()
                .as_vector()
                .dot(frame.y_axis().as_vector())
                .unwrap(),
            0.,
        );
    }
}

#[test]
fn ties_use_transverse_minima_in_the_signed_distribution_frame() {
    for plane in WorldPlane::ALL.map(WorldPlane::frame) {
        for axis in 0..3 {
            for sign in [-1., 1.] {
                let direction = plane.axes()[axis].as_vector().scaled(sign).unwrap();
                let origin = p([0.; 3]);
                let frame = direction_frame(
                    Direction::Points(origin, origin.translated(direction).unwrap()),
                    plane,
                    origin,
                    Tolerance::DEFAULT,
                )
                .unwrap();
                for transverse in [[-2., 1.], [1., -2.], [0., -2.]] {
                    // First two objects tie in X, but disagree in Y and Z.
                    // Test reversed creation/selection without deriving the
                    // expected order from the implementation's sort result.
                    let local = [
                        [0., 0., 0.],
                        [0., transverse[0], transverse[1]],
                        [20., 0., 0.],
                    ];
                    for reversed in [false, true] {
                        let creation = if reversed { [2, 1, 0] } else { [0, 1, 2] };
                        let mut document = selected(
                            creation
                                .map(|i| Geometry::Point(frame.point_at(local[i]).unwrap()))
                                .to_vec(),
                        );
                        DistributeCommand
                            .run_in_context(
                                &mut document,
                                &[
                                    "Direction",
                                    "0,0,0",
                                    &format!(
                                        "{},{},{}",
                                        direction.x(),
                                        direction.y(),
                                        direction.z()
                                    ),
                                ],
                                CommandContext {
                                    construction_plane: plane,
                                },
                            )
                            .unwrap();
                        let second_first =
                            transverse[0] < 0. || (transverse[0] == 0. && transverse[1] < 0.);
                        for (object, i) in document.objects().zip(creation) {
                            let Geometry::Point(point) = object.geometry() else {
                                panic!("point")
                            };
                            let expected = if i == 2 {
                                20.
                            } else if (i == 0) == second_first {
                                10.
                            } else {
                                0.
                            };
                            near(frame.coordinates_of(*point).unwrap()[0], expected);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn all_cplane_axes_and_distant_origins_preserve_perpendicular_components() {
    let oblique = Frame3::try_from_directions(
        p([0.; 3]),
        Vector3::try_new(1., 2., 3.).unwrap(),
        Vector3::try_new(-4., 8., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for plane in WorldPlane::ALL
        .map(WorldPlane::frame)
        .into_iter()
        .chain([oblique])
    {
        for (axis, name) in ["XAxis", "YAxis", "ZAxis"].into_iter().enumerate() {
            for origin in [[0.; 3], [1e100, -1e100, 1e100]] {
                let locals = [[0., 1., 2.], [3., 4., 5.], [10., 11., 12.]];
                let points = locals.map(|a| plane.point_at(a).unwrap());
                let mut document = selected(points.map(Geometry::Point).to_vec());
                let ids = document.selected_object_ids().collect::<Vec<_>>();
                CommandRegistry::with_builtins()
                    .execute_in_context(
                        &mut document,
                        &format!("Distribute {name} Mode=Center"),
                        CommandContext {
                            construction_plane: plane.with_origin(p(origin)),
                        },
                    )
                    .unwrap();
                for (i, object) in document.objects().enumerate() {
                    let Geometry::Point(point) = object.geometry() else {
                        panic!("point")
                    };
                    let local = plane.coordinates_of(*point).unwrap();
                    for j in 0..3 {
                        near(
                            local[j],
                            if i == 1 && j == axis {
                                (locals[0][j] + locals[2][j]) / 2.
                            } else {
                                locals[i][j]
                            },
                        );
                    }
                    assert_eq!(object.id(), ids[i]);
                    assert!(document.is_selected(object.id()));
                }
                assert_eq!(document.undo_label(), Some("Distribute"));
                document.undo().unwrap();
                for (object, point) in document.objects().zip(points) {
                    assert_eq!(object.geometry(), &Geometry::Point(point));
                }
                document.redo().unwrap();
                assert_eq!(document.objects().len(), 3);
            }
        }
    }
}

#[test]
fn typed_spatial_direction_uses_all_three_world_components() {
    let mut document = selected(vec![
        cloud(&[[0., 0., 0.], [2., 1., 3.]]),
        cloud(&[[5., 2., 0.], [9., 4., 2.]]),
        cloud(&[[16., 8., 5.], [17., 10., 8.]]),
    ]);
    CommandRegistry::with_builtins()
        .execute(
            &mut document,
            "Distribute Direction 1,2,3 3,5,10 Mode=Center",
        )
        .unwrap();
    let Geometry::PointCloud(c) = document.objects().nth(1).unwrap().geometry() else {
        panic!("cloud")
    };
    for (value, expected) in c.points()[0].to_array().into_iter().zip([
        5.959677419354839,
        3.439516129032258,
        3.358870967741936,
    ]) {
        near(value, expected);
    }
}

#[test]
fn positive_rational_curve_extents_replace_control_boxes() {
    let curve = NurbsCurve::try_new(
        2,
        [[0., 0., 0.], [10., 1., 0.], [0., 2., 0.]].map(p).to_vec(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let mut document = selected(vec![
        Geometry::Point(p([-10., 0., 0.])),
        Geometry::NurbsCurve(curve.clone()),
        Geometry::Point(p([20., 0., 0.])),
    ]);
    CommandRegistry::with_builtins()
        .execute(&mut document, "Distribute XAxis Mode=Gap")
        .unwrap();
    // Width is 5, not the control-net width 10. Equal gaps are 12.5.
    let Geometry::NurbsCurve(copy) = document.objects().nth(1).unwrap().geometry() else {
        panic!("curve")
    };
    for i in 0..=100 {
        let t = i as f64 / 100.;
        let a = curve.evaluate(t).unwrap();
        let b = copy.evaluate(t).unwrap();
        near(b.x() - a.x(), 2.5);
        near(b.y(), a.y());
    }
    assert_eq!(copy.knots(), curve.knots());
}

#[test]
fn invalid_settings_minimum_groups_and_poles_are_atomic() {
    let pole = NurbsCurve::try_new_rational(
        2,
        [[0., 0., 0.], [1., 3., 0.], [2., 0., 0.]]
            .into_iter()
            .zip([1., -1., 1.])
            .map(|(p0, w)| WeightedPoint3::try_new(p(p0), w).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let mut document = selected(vec![
        Geometry::Point(p([-10., 0., 0.])),
        Geometry::NurbsCurve(pole),
        Geometry::Point(p([20., 0., 0.])),
    ]);
    let originals = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "Distribute XAxis",
        "Distribute XAxis Mode=No",
        "Distribute XAxis XAxis",
        "Distribute XAxis Spacing=NaN",
        "Distribute XAxis Mode=Gap Mode=Center",
        "Distribute Direction 1,2,3 1,2,3",
        "Distribute XAxis Spacing=3 Spacing=Automatic",
    ] {
        assert!(
            CommandRegistry::with_builtins()
                .execute(&mut document, command)
                .is_err(),
            "{command}"
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), originals);
        assert_eq!(document.selected_object_count(), 3);
        assert_eq!(document.undo_label(), history.as_deref());
    }
    for count in 0..3 {
        let mut document = selected(
            (0..count)
                .map(|i| Geometry::Point(p([i as f64, 0., 0.])))
                .collect(),
        );
        assert!(
            matches!(CommandRegistry::with_builtins().execute(&mut document,"Distribute XAxis"),Err(CommandError::InsufficientDistributionObjects{actual}) if actual==count)
        );
        assert_eq!(document.objects().len(), count);
    }
}

#[test]
fn groups_are_rigid_without_losing_identity_attributes_or_membership() {
    let mut document = selected(
        [0., 2., 10., 13., 30., 40.]
            .into_iter()
            .enumerate()
            .map(|(i, x)| Geometry::Point(p([x, i as f64, 0.])))
            .collect(),
    );
    let ids = document.selected_object_ids().collect::<Vec<_>>();
    document
        .add_group(Some("first".into()), [ids[0], ids[1]])
        .unwrap();
    document
        .add_group(Some("middle".into()), [ids[2], ids[3]])
        .unwrap();
    let original = document.objects().cloned().collect::<Vec<_>>();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Distribute XAxis")
        .unwrap();
    let expected = [0., 2., 2. + 35. / 3., 5. + 35. / 3., 40. - 35. / 3., 40.];
    for ((object, old), x) in document.objects().zip(&original).zip(expected) {
        let Geometry::Point(p) = object.geometry() else {
            panic!("point")
        };
        near(p.x(), x);
        assert_eq!(object.id(), old.id());
        assert_eq!(object.attributes(), old.attributes());
    }
    assert_eq!(document.groups().len(), 2);
    document.undo().unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
    document
        .select_objects_direct(ids[..2].iter().copied(), SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        CommandRegistry::with_builtins().execute(&mut document, "Distribute XAxis"),
        Err(CommandError::InsufficientDistributionObjects { actual: 1 })
    ));
}

#[test]
fn latest_overlapping_group_membership_defines_each_rigid_unit() {
    for reverse in [false, true] {
        let mut document = selected(
            [0., 2., 10., 13., 30., 40.]
                .into_iter()
                .map(|x| Geometry::Point(p([x, 0., 0.])))
                .collect(),
        );
        let ids = document.selected_object_ids().collect::<Vec<_>>();
        let mut groups = [[ids[0], ids[1]], [ids[1], ids[2]]];
        if reverse {
            groups.reverse();
        }
        for (i, group) in groups.into_iter().enumerate() {
            document.add_group(Some(i.to_string()), group).unwrap();
        }
        CommandRegistry::with_builtins()
            .execute(&mut document, "Distribute XAxis")
            .unwrap();
        let expected = if reverse {
            [0., 2., 11.5, 21., 30.5, 40.]
        } else {
            [0., 8., 16., 24., 32., 40.]
        };
        for (object, x) in document.objects().zip(expected) {
            let Geometry::Point(p) = object.geometry() else {
                panic!("point")
            };
            near(p.x(), x);
        }
        assert_eq!(document.groups().len(), 2);
    }
}

#[test]
fn partial_groups_do_not_pull_unselected_members_into_the_edit() {
    let mut document = selected(
        [0., 2., 10., 13., 30., 40.]
            .into_iter()
            .map(|x| Geometry::Point(p([x, 0., 0.])))
            .collect(),
    );
    let ids = document.selected_object_ids().collect::<Vec<_>>();
    document
        .add_group(Some("first".into()), [ids[0], ids[1]])
        .unwrap();
    document
        .add_group(Some("second".into()), [ids[2], ids[3]])
        .unwrap();
    document
        .select_objects_direct(
            [ids[0], ids[2], ids[3], ids[4], ids[5]],
            SelectionMode::Replace,
        )
        .unwrap();
    let unselected = document.object(ids[1]).unwrap().clone();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Distribute XAxis")
        .unwrap();
    assert_eq!(document.object(ids[1]).unwrap(), &unselected);
    assert!(!document.is_selected(ids[1]));
    let Geometry::Point(p) = document.object(ids[2]).unwrap().geometry() else {
        panic!("point")
    };
    near(p.x(), 37. / 3.);
}
