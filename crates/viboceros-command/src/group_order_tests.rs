use super::*;

fn point(x: f64) -> Geometry {
    Geometry::Point(Point3::try_new(x, 0., 0.).unwrap())
}

#[test]
fn distribution_uses_a_membership_added_later_to_an_older_group() {
    let mut document = Document::default();
    let ids = [0., 2., 10., 13., 30., 40.].map(|x| document.add_geometry(point(x)).unwrap());
    let older = document
        .add_group(Some("Older".into()), [ids[0], ids[1]])
        .unwrap();
    let newer = document
        .add_group(Some("Newer".into()), [ids[1], ids[2]])
        .unwrap();
    document.add_group_members(older, [ids[2]]).unwrap();
    document.select_all();
    let original = document.objects().cloned().collect::<Vec<_>>();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut document, "Distribute XAxis").unwrap();
    for (object, x) in document.objects().zip([0., 17.5, 10., 25., 32.5, 40.]) {
        assert_eq!(object.geometry(), &point(x));
    }
    assert_eq!(
        document.object(ids[1]).unwrap().group_ids(),
        &[older, newer]
    );
    assert_eq!(
        document.object(ids[2]).unwrap().group_ids(),
        &[newer, older]
    );
    let changed = document.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), changed);
}

#[test]
fn ungroup_commands_change_only_selected_memberships_and_retain_empty_definitions() {
    for all in [false, true] {
        for partial in [false, true] {
            let mut document = Document::default();
            let ids = [0., 1., 2.].map(|x| document.add_geometry(point(x)).unwrap());
            let inner = document
                .add_group(Some("Inner".into()), [ids[0], ids[1]])
                .unwrap();
            let outer = document.add_group(Some("Outer".into()), ids).unwrap();
            document
                .select_objects_direct(
                    if partial { vec![ids[0]] } else { ids.to_vec() },
                    SelectionMode::Replace,
                )
                .unwrap();
            let original = document.objects().cloned().collect::<Vec<_>>();
            let groups = document.groups().cloned().collect::<Vec<_>>();
            let registry = CommandRegistry::with_builtins();
            registry
                .execute(&mut document, if all { "UngroupAll" } else { "Ungroup" })
                .unwrap();
            assert_eq!(document.groups().len(), 2);
            assert_eq!(
                document.object(ids[0]).unwrap().group_ids(),
                if all { vec![] } else { vec![inner] }
            );
            assert_eq!(
                document.object(ids[1]).unwrap().group_ids(),
                if partial {
                    vec![inner, outer]
                } else if all {
                    vec![]
                } else {
                    vec![inner]
                }
            );
            assert_eq!(
                document.object(ids[2]).unwrap().group_ids(),
                if partial { vec![outer] } else { vec![] }
            );
            let changed = document.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
            assert_eq!(document.groups().cloned().collect::<Vec<_>>(), groups);
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), changed);
        }
    }
}

fn multispan_surface() -> Geometry {
    Geometry::NurbsSurface(
        NurbsSurface::try_new(
            1,
            1,
            3,
            2,
            [0., 1.]
                .into_iter()
                .flat_map(|y| [0., 1., 2.].map(move |x| Point3::try_new(x, y, 0.).unwrap()))
                .collect(),
            vec![0., 0., 1., 2., 2.],
            vec![0., 0., 1., 1.],
        )
        .unwrap(),
    )
}

fn mesh(duplicate: bool) -> Geometry {
    Geometry::Mesh(
        TriangleMesh::try_new(
            [
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 1., 0.],
                [3., 0., 0.],
                [4., 0., 0.],
                [3., 1., 0.],
            ]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
            if duplicate {
                vec![[0, 1, 2], [0, 1, 2], [3, 4, 5]]
            } else {
                vec![[0, 1, 2], [3, 4, 5]]
            },
            Tolerance::DEFAULT,
        )
        .unwrap(),
    )
}

#[test]
fn decompositions_and_mesh_extractions_do_not_sort_memberships_by_group_id() {
    let curve = Geometry::NurbsCurve(
        NurbsCurve::try_new(
            2,
            [[0., 0., 0.], [1., 2., 0.], [3., 2., 0.], [4., 0., 0.]]
                .map(|p| Point3::try_from(p).unwrap())
                .to_vec(),
            vec![0., 0., 0., 1., 2., 2., 2.],
        )
        .unwrap(),
    );
    let polyline = Geometry::Polyline(
        Polyline3::try_new(
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 2., 0.).unwrap(),
                Point3::try_new(3., 2., 0.).unwrap(),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    for (source, command) in [
        (curve, "Split Parameter=1"),
        (
            multispan_surface(),
            "Split Isocurve=1,0.5,0 Direction=Both Shrink=Yes",
        ),
        (polyline, "Explode"),
        (mesh(false), "SplitDisjointMesh"),
        (mesh(false), "ExtractMeshFaces Faces=0 MakeCopy=Yes"),
        (mesh(false), "ExtractMeshFaces Faces=0 MakeCopy=No"),
        (mesh(true), "ExtractDuplicateMeshFaces"),
    ] {
        // Repeat to ensure randomly generated GroupIds cannot accidentally put
        // a formerly UUID-sorted batching path into the expected order.
        for _ in 0..3 {
            let mut document = Document::default();
            let id = document.add_geometry(source.clone()).unwrap();
            let peer = document.add_geometry(point(100.)).unwrap();
            let groups = [0, 1, 2].map(|i| {
                document
                    .add_group(Some(format!("Group-{i}")), [id, peer])
                    .unwrap()
            });
            let order = [groups[2], groups[0], groups[1]];
            document.set_object_group_memberships(id, order).unwrap();
            document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            let original = document.objects().cloned().collect::<Vec<_>>();
            let registry = CommandRegistry::with_builtins();
            registry
                .execute(&mut document, command)
                .unwrap_or_else(|error| panic!("{command}: {error}"));
            assert!(document.objects().len() > original.len(), "{command}");
            for object in document.objects().filter(|o| o.id() != peer) {
                assert_eq!(object.group_ids(), &order, "{command}");
            }
            let changed = document.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(
                document.objects().cloned().collect::<Vec<_>>(),
                original,
                "{command}"
            );
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(
                document.objects().cloned().collect::<Vec<_>>(),
                changed,
                "{command}"
            );
        }
    }
}

struct Temporary3dm {
    path: std::path::PathBuf,
    _directory: tempfile::TempDir,
}
impl Temporary3dm {
    fn new(label: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        Self {
            path: directory.path().join(format!("{label}.3dm")),
            _directory: directory,
        }
    }
}

#[test]
fn command_3dm_roundtrip_retains_individual_order_empty_groups_and_undo() {
    let input = Temporary3dm::new("input");
    let output = Temporary3dm::new("output");
    let orders = [vec![2, 0, 1], vec![1, 2, 0], vec![0, 1, 2]];
    let objects = orders
        .iter()
        .enumerate()
        .map(|(i, order)| {
            let mut object = ThreeDmObject::new(
                ThreeDmGeometry::Point(Point3::try_new(i as f64, 0., 0.).unwrap()),
                0,
            );
            object.name = Some(i.to_string());
            object.group_indices = order.clone();
            object.visible = i != 1;
            object.locked = i == 2;
            object
        })
        .collect();
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Input".into(),
            color: [3, 4, 5],
            visible: true,
            locked: false,
        }],
        (0..4)
            .map(|i| ThreeDmGroup {
                name: format!("Group-{i}"),
            })
            .collect(),
        objects,
    );
    write_3dm_file(&input.path, &model).unwrap();
    let mut document = Document::default();
    let registry = CommandRegistry::with_builtins();
    for import in 0..2 {
        registry
            .execute(
                &mut document,
                &format!("Import3dm {}", input.path.display()),
            )
            .unwrap();
        let groups = document.groups().map(|g| g.id()).collect::<Vec<_>>();
        for (object, order) in document.objects().skip(import * 3).zip(&orders) {
            assert_eq!(
                object.group_ids(),
                order
                    .iter()
                    .map(|i| groups[import * 4 + i])
                    .collect::<Vec<_>>()
            );
        }
    }
    let imported = document.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.groups().len(), 4);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), imported);
    registry
        .execute(
            &mut document,
            &format!("Export3dm {}", output.path.display()),
        )
        .unwrap();
    let decoded = read_3dm_file(&output.path, Tolerance::DEFAULT).unwrap();
    assert_eq!(decoded.groups.len(), 8);
    for (index, object) in decoded.objects.iter().enumerate() {
        assert_eq!(
            object.group_indices,
            orders[index % 3]
                .iter()
                .map(|i| i + (index / 3) * 4)
                .collect::<Vec<_>>()
        );
        assert_eq!(object.visible, index % 3 != 1);
        assert_eq!(object.locked, index % 3 == 2);
    }
}
