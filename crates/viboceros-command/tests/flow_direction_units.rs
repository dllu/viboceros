use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, ObjectAttributes, SelectionMode};
use viboceros_geometry::{Point3, Tolerance};

fn token(point: Point3) -> String {
    let [x, y, z] = point.to_array();
    format!("{x},{y},{z}")
}
#[test]
fn flow_commands_keep_opposite_line_planes_copy_policy_groups_and_replay_across_units() {
    let registry = CommandRegistry::with_builtins();
    let mut checked = 0;
    for exponent in [-600, -200, -20, 0, 20, 40, 200, 600] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
        let p = |x, y, z| Point3::try_new(x * scale, y * scale, z * scale).unwrap();
        for offset in [[5_f64, 7., 11.], [5., -3., 4.], [5., 0., 0.]] {
            for stretch in [false, true] {
                for copy in [false, true] {
                    let mut doc = Document::default();
                    doc.set_tolerance(tolerance);
                    let layer = doc.current_layer_id();
                    let coordinates = [[1., 0.2, 0.4], [0., -0.3, 0.5], [4., 0.4, -0.2]];
                    let sources = coordinates.map(|[x, y, z]| {
                        doc.add_geometry_with_attributes(
                            Geometry::Point(p(x, y, z)),
                            ObjectAttributes::on_layer(layer)
                                .with_name(format!("point-{x}-{y}-{z}")),
                        )
                        .unwrap()
                    });
                    doc.add_group(Some("source".into()), sources).unwrap();
                    doc.select_objects_direct(sources, SelectionMode::Replace)
                        .unwrap();
                    doc.clear_history().unwrap();
                    let before = doc.objects().cloned().collect::<Vec<_>>();
                    let before_groups = doc.groups().cloned().collect::<Vec<_>>();
                    let command = format!(
                        "Flow BaseLine={};{} TargetLine={};{} Copy={} Rigid=No Stretch={}",
                        token(p(0., 0., 0.)),
                        token(p(3., 0., 0.)),
                        token(p(offset[0], offset[1], offset[2])),
                        token(p(offset[0] - 6., offset[1], offset[2])),
                        if copy { "Yes" } else { "No" },
                        if stretch { "Yes" } else { "No" }
                    );
                    registry.execute(&mut doc, &command).unwrap();
                    assert_eq!(doc.objects().len(), if copy { 6 } else { 3 });
                    let n = if offset[1] == 0. && offset[2] == 0. {
                        [0., 1., 0.]
                    } else {
                        let r = offset[1].hypot(offset[2]);
                        [0., -offset[2] / r, offset[1] / r]
                    };
                    for (([x, y, z], id), original) in
                        coordinates.into_iter().zip(sources).zip(&before)
                    {
                        let object = if copy {
                            doc.objects()
                                .find(|o| {
                                    o.id() != id
                                        && o.attributes().name() == original.attributes().name()
                                })
                                .unwrap()
                        } else {
                            doc.object(id).unwrap()
                        };
                        let Geometry::Point(actual) = object.geometry() else {
                            panic!("flow point");
                        };
                        let dot = n[1] * y + n[2] * z;
                        let expected = p(
                            offset[0] - x * if stretch { 2. } else { 1. },
                            offset[1] + 2. * n[1] * dot - y,
                            offset[2] + 2. * n[2] * dot - z,
                        );
                        assert!(
                            actual.distance_to(expected).unwrap() / scale < 2e-12,
                            "scale=2^{exponent} stretch={stretch} copy={copy}"
                        );
                        assert_eq!(object.group_ids().len(), 1);
                        if copy {
                            assert_eq!(doc.object(id).unwrap(), original);
                        }
                    }
                    let after = doc.objects().cloned().collect::<Vec<_>>();
                    let after_groups = doc.groups().cloned().collect::<Vec<_>>();
                    doc.undo().unwrap();
                    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
                    for group in &before_groups {
                        assert_eq!(doc.group(group.id()).unwrap(), group);
                    }
                    assert_eq!(doc.groups().len(), after_groups.len());
                    for group in doc
                        .groups()
                        .filter(|g| !before_groups.iter().any(|original| original.id() == g.id()))
                    {
                        let created = after_groups
                            .iter()
                            .find(|created| created.id() == group.id())
                            .unwrap();
                        assert_eq!(group.name(), created.name());
                        assert_eq!(group.members().len(), 0);
                    }
                    doc.redo().unwrap();
                    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
                    assert_eq!(doc.groups().cloned().collect::<Vec<_>>(), after_groups);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 96);
    eprintln!("FLOW_COMMAND_UNITS cases={checked}");
}
