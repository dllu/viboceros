use std::collections::BTreeMap;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, ObjectAttributes, SelectionMode};
use viboceros_geometry::{
    Circle3, CircularArc3, LineSegment, Point3, Polyline3, Tolerance, Vector3,
};

fn p(v: [f64; 3]) -> Point3 {
    Point3::try_from(v).unwrap()
}
fn token(point: Point3) -> String {
    let [x, y, z] = point.to_array();
    format!("{x},{y},{z}")
}

#[test]
fn curve_arrays_keep_rigid_pose_spacing_groups_and_history_across_model_units() {
    let registry = CommandRegistry::with_builtins();
    let mut checked = 0;
    let mut failures = Vec::new();
    for exponent in [-20, 0, 20, 40, 80] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
        let origin = [5. * scale, -7. * scale, 11. * scale];
        let point = |a: [f64; 3]| p(std::array::from_fn(|i| origin[i] + scale * a[i]));
        let normal = Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        let circle = Circle3::try_new(point([0., 0., 0.]), 4. * scale, normal, tolerance).unwrap();
        let arc = CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap();
        for kind in 0..6 {
            let geometry = match kind {
                0 => Geometry::Arc(arc),
                1 | 2 => Geometry::NurbsCurve(
                    arc.to_nurbs()
                        .unwrap()
                        .try_reparameterized(0. ..=2_f64.powi(if kind == 1 { -200 } else { 200 }))
                        .unwrap(),
                ),
                3 => Geometry::Line(
                    LineSegment::try_new(point([0., 0., 0.]), point([0., 0., 12.]), tolerance)
                        .unwrap(),
                ),
                4 => Geometry::Line(
                    LineSegment::try_new(point([0., 0., 0.]), point([4., 8., 8.]), tolerance)
                        .unwrap(),
                ),
                _ => Geometry::Polyline(
                    Polyline3::try_new(
                        vec![
                            point([0., 0., 0.]),
                            point([4., 0., 0.]),
                            point([4., 0., 4.]),
                            point([4., 4., 4.]),
                        ],
                        tolerance,
                    )
                    .unwrap(),
                ),
            };
            let count = if kind == 5 { 7 } else { 5 };
            let length = if kind < 3 {
                2. * std::f64::consts::PI * scale
            } else {
                12. * scale
            };
            let stations = (0..count)
                .map(|i| {
                    let f = i as f64 / (count - 1) as f64;
                    if kind < 3 {
                        let a = f * std::f64::consts::FRAC_PI_2;
                        point([4. * a.cos(), 4. * a.sin(), 0.])
                    } else if kind == 3 {
                        point([0., 0., 12. * f])
                    } else if kind == 4 {
                        point([4. * f, 8. * f, 8. * f])
                    } else if i <= 2 {
                        point([2. * i as f64, 0., 0.])
                    } else if i <= 4 {
                        point([4., 0., 2. * (i - 2) as f64])
                    } else {
                        point([4., 2. * (i - 4) as f64, 4.])
                    }
                })
                .collect::<Vec<_>>();
            for orientation in ["Freeform", "Roadlike", "Stairlike", "NoRotation"] {
                for distance in [false, true] {
                    for explicit in [false, true] {
                        checked += 1;
                        let label = format!(
                            "scale=2^{exponent} kind={kind} {orientation} distance={distance} explicit={explicit}"
                        );
                        let mut doc = Document::default();
                        doc.set_tolerance(tolerance);
                        let layer = doc.current_layer_id();
                        let base = if explicit {
                            stations[0]
                                .translated(
                                    Vector3::try_new(-2. * scale, 3. * scale, -scale).unwrap(),
                                )
                                .unwrap()
                        } else {
                            stations[0]
                        };
                        let names = ["origin", "x", "y", "z"];
                        let sources = std::array::from_fn::<_, 4, _>(|i| {
                            let mut a = base.to_array();
                            if i > 0 {
                                a[i - 1] += 0.2 * scale;
                            }
                            doc.add_geometry_with_attributes(
                                Geometry::Point(p(a)),
                                ObjectAttributes::on_layer(layer).with_name(names[i]),
                            )
                            .unwrap()
                        });
                        let source_group = doc.add_group(Some("source".into()), sources).unwrap();
                        let rail = doc
                            .add_geometry_with_attributes(
                                geometry.clone(),
                                ObjectAttributes::on_layer(layer).with_name("rail"),
                            )
                            .unwrap();
                        doc.select_objects_direct(sources, SelectionMode::Replace)
                            .unwrap();
                        doc.clear_history().unwrap();
                        let before = doc.objects().cloned().collect::<Vec<_>>();
                        let before_groups = doc.groups().cloned().collect::<Vec<_>>();
                        let spacing = if distance {
                            format!("Distance {}", length / (count - 1) as f64)
                        } else {
                            count.to_string()
                        };
                        let base_option = if explicit {
                            format!(" BasePoint={}", token(base))
                        } else {
                            String::new()
                        };
                        let command = format!(
                            "ArrayCrv {spacing} Orientation={orientation} PathName=rail{base_option}"
                        );
                        match registry.execute(&mut doc, &command) {
                            Ok(_) => {
                                let instances = count - usize::from(!explicit);
                                assert_eq!(doc.objects().len(), 5 + 4 * instances, "{label}");
                                assert_eq!(doc.groups().len(), 1 + instances);
                                for object in &before {
                                    assert_eq!(doc.object(object.id()).unwrap(), object);
                                }
                                assert_eq!(
                                    doc.selected_object_ids().collect::<Vec<_>>(),
                                    sources.to_vec()
                                );
                                assert!(!doc.is_selected(rail));
                                let mut groups = BTreeMap::new();
                                for object in doc
                                    .objects()
                                    .filter(|o| !sources.contains(&o.id()) && o.id() != rail)
                                {
                                    assert_eq!(object.group_ids().len(), 1);
                                    let group = object.group_ids()[0];
                                    assert_ne!(group, source_group);
                                    let Geometry::Point(point) = object.geometry() else {
                                        panic!("array point");
                                    };
                                    groups.entry(group).or_insert_with(BTreeMap::new).insert(
                                        object.attributes().name().unwrap().to_owned(),
                                        *point,
                                    );
                                }
                                let mut seen = std::collections::BTreeSet::new();
                                for points in groups.values() {
                                    assert_eq!(points.len(), 4);
                                    let anchor = points["origin"];
                                    let i = stations
                                        .iter()
                                        .position(|q| {
                                            anchor.distance_to(*q).unwrap() / scale < 2e-7
                                        })
                                        .unwrap_or_else(|| {
                                            panic!("{label}: bad station {anchor:?}")
                                        });
                                    assert!(seen.insert(i));
                                    if !explicit {
                                        assert_ne!(i, 0);
                                    }
                                    let axes = ["x", "y", "z"].map(|name| {
                                        anchor
                                            .vector_to(points[name])
                                            .unwrap()
                                            .scaled(1. / (0.2 * scale))
                                            .unwrap()
                                    });
                                    for a in axes {
                                        assert!(
                                            (a.length().unwrap() - 1.).abs() < 2e-10,
                                            "{label}"
                                        );
                                    }
                                    assert!(
                                        axes[0].cross(axes[1]).unwrap().dot(axes[2]).unwrap()
                                            > 1. - 2e-10,
                                        "{label}"
                                    );
                                    for a in 0..3 {
                                        for b in 0..a {
                                            assert!(axes[a].dot(axes[b]).unwrap().abs() < 2e-10);
                                        }
                                    }
                                    if orientation == "NoRotation" || kind == 3 || kind == 4 {
                                        for (axis, j) in axes.into_iter().zip(0..3) {
                                            let values = axis.to_array();
                                            assert!(values[j] > 1. - 2e-10, "{label}");
                                        }
                                    } else if kind < 3 {
                                        let angle = i as f64 / (count - 1) as f64
                                            * std::f64::consts::FRAC_PI_2;
                                        let expected = [
                                            [angle.cos(), angle.sin(), 0.],
                                            [-angle.sin(), angle.cos(), 0.],
                                            [0., 0., 1.],
                                        ];
                                        for (axis, e) in axes.into_iter().zip(expected) {
                                            assert!(
                                                axis.dot(Vector3::try_from(e).unwrap()).unwrap()
                                                    > 1. - 2e-10,
                                                "{label}"
                                            );
                                        }
                                    } else if [1, 3, 5, 6].contains(&i) {
                                        let expected = if i == 1 {
                                            [1., 0., 0.]
                                        } else if i == 3 {
                                            if orientation == "Stairlike" {
                                                [1., 0., 0.]
                                            } else {
                                                [0., 0., 1.]
                                            }
                                        } else {
                                            [0., 1., 0.]
                                        };
                                        assert!(
                                            axes[0]
                                                .dot(Vector3::try_from(expected).unwrap())
                                                .unwrap()
                                                > 1. - 2e-10,
                                            "{label}: wrong tangent/heading"
                                        );
                                    }
                                }
                                assert_eq!(seen.len(), instances);
                                let after = doc.objects().cloned().collect::<Vec<_>>();
                                let after_groups = doc.groups().cloned().collect::<Vec<_>>();
                                doc.undo().unwrap();
                                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
                                assert_eq!(
                                    doc.groups().cloned().collect::<Vec<_>>(),
                                    before_groups
                                );
                                doc.redo().unwrap();
                                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
                                assert_eq!(doc.groups().cloned().collect::<Vec<_>>(), after_groups);
                            }
                            Err(error) => {
                                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
                                assert_eq!(
                                    doc.groups().cloned().collect::<Vec<_>>(),
                                    before_groups
                                );
                                assert!(!doc.can_undo());
                                failures.push(format!("{label}: {error:?}"));
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "CURVE_ARRAY_UNITS cases={checked} failures={}",
        failures.len()
    );
    assert_eq!(checked, 480);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn curve_array_errors_and_single_item_noops_preserve_objects_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut checked = 0;
    for exponent in [-20, 0, 20, 40, 80] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
        let mut doc = Document::default();
        doc.set_tolerance(tolerance);
        let layer = doc.current_layer_id();
        let start = p([0.; 3]);
        let source = doc.add_geometry(Geometry::Point(start)).unwrap();
        doc.add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(start, p([4. * scale, 0., 0.]), tolerance).unwrap(),
            ),
            ObjectAttributes::on_layer(layer).with_name("rail"),
        )
        .unwrap();
        doc.select_object(source, SelectionMode::Replace).unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut doc, "ArrayCrv 1 PathName=rail")
            .unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
        checked += 1;
        doc.add_geometry(Geometry::Point(p([0., scale, 0.])))
            .unwrap();
        doc.undo().unwrap();
        let selected = doc.selected_object_ids().collect::<Vec<_>>();
        for command in [
            "ArrayCrv 0 PathName=rail",
            "ArrayCrv Distance 0 PathName=rail",
            "ArrayCrv 2 Orientation=invalid PathName=rail",
            "ArrayCrv 2 PathName=missing",
        ] {
            assert!(registry.execute(&mut doc, command).is_err());
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
            assert!(!doc.can_undo() && doc.can_redo());
            checked += 1;
        }
    }
    assert_eq!(checked, 25);
    eprintln!("CURVE_ARRAY_ADMISSION cases={checked}");
}
