use super::*;
use crate::CommandRegistry;

#[test]
fn fillet_edge_prompt_owns_radius_and_edge_filter() {
    let registry = CommandRegistry::with_builtins();
    let prompt = registry
        .component_selection_prompt("FilletEdge Radius=0.5")
        .unwrap()
        .unwrap();
    assert_eq!(prompt.kind, ComponentSelectionKind::BrepEdge);
    assert_eq!(prompt.numbers[0].value, 0.5);
    assert!(
        registry
            .component_selection_prompt("FilletEdge Radius=-1")
            .is_err()
    );
}

mod native {
    use super::*;
    fn selected_box() -> (CommandRegistry, Document, ObjectId) {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, "Box 0,0,0 10,10,0 10").unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        let id = doc.objects().next().unwrap().id();
        (registry, doc, id)
    }
    #[test]
    fn all_edge_command_preserves_identity_attributes_and_replays_history() {
        let (registry, mut doc, id) = selected_box();
        registry.execute(&mut doc, "SetObjectName Rounded").unwrap();
        registry.execute(&mut doc, "Group FilletGroup").unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut doc, "FilletEdge Radius=1 All=Yes")
            .unwrap();
        let object = doc.object(id).unwrap();
        assert_eq!(object.attributes(), before[0].attributes());
        assert_eq!(object.group_ids(), before[0].group_ids());
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("editable rounded B-rep expected")
        };
        assert!(brep.is_solid());
        assert_eq!(brep.faces().len(), 26);
        assert_eq!(doc.undo_label(), Some("FilletEdge"));
        let after = doc.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    }

    #[test]
    fn explicit_ball_corners_round_all_cube_vertices() {
        let (registry, mut doc, id) = selected_box();
        registry
            .execute(&mut doc, "FilletEdge Corner=Ball Radius=1 All=Yes")
            .unwrap();
        let Geometry::Brep(brep) = doc.object(id).unwrap().geometry() else {
            panic!("editable rounded B-rep expected");
        };
        assert!(brep.is_solid());
        assert_eq!(brep.faces().len(), 26);
    }
    #[test]
    fn one_original_edge_is_selected_once_by_typed_index() {
        let (registry, mut doc, id) = selected_box();
        registry
            .execute(&mut doc, &format!("FilletEdge 1 {id} 0,0"))
            .unwrap();
        let Geometry::Brep(brep) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert!(brep.is_solid());
        assert_eq!(brep.faces().len(), 7);
        assert!(
            (brep.signed_volume(doc.tolerance()).unwrap()
                - (1000. - 10. * (1. - std::f64::consts::PI / 4.)))
                .abs()
                < 1e-7
        );
    }

    #[test]
    fn trim_and_join_no_adds_only_blend_patches_as_one_undo_step() {
        for (command, options, setback) in [
            ("FilletEdge", "", 1.),
            (
                "ChamferEdge",
                "RailType=DistBetweenRails",
                2_f64.sqrt().recip(),
            ),
            ("BlendEdge", "RailType=RollingBall", 1.),
        ] {
            let (registry, mut doc, id) = selected_box();
            registry
                .execute(&mut doc, "SetObjectName PatchSource")
                .unwrap();
            registry.execute(&mut doc, "Group PatchParent").unwrap();
            let before = doc.object(id).unwrap().clone();
            let Geometry::Brep(source) = before.geometry() else {
                panic!("expected a box")
            };
            let find = |y, z| {
                source
                    .edges()
                    .iter()
                    .position(|edge| {
                        edge.vertices().iter().all(|&vertex| {
                            let p = source.vertices()[vertex].point();
                            (p.y(), p.z()) == (y, z)
                        })
                    })
                    .unwrap()
            };
            let a = find(0., 0.);
            let b = find(10., 10.);
            let count = registry
                .execute(
                    &mut doc,
                    &format!("{command} 1 {id} {a},{b} _TrimAndJoin=_No {options}"),
                )
                .unwrap_or_else(|error| panic!("{command}: {error}"));
            assert!(count.contains("Created 2"), "{count}");
            assert_eq!(doc.object(id).unwrap(), &before);
            let patches = doc.objects().filter(|o| o.id() != id).collect::<Vec<_>>();
            assert_eq!(patches.len(), 2);
            for object in &patches {
                assert_eq!(object.attributes(), before.attributes());
                assert!(object.group_ids().is_empty());
                let Geometry::Brep(brep) = object.geometry() else {
                    panic!("expected a patch")
                };
                assert_eq!(brep.faces().len(), 1);
                assert!(!brep.is_solid());
                let bounds = viboceros_geometry::BoundingBox3::from_points(
                    brep.vertices().iter().map(|vertex| vertex.point()),
                )
                .unwrap();
                assert!(
                    (bounds.max().x() - bounds.min().x() - 10.).abs() < 1e-6,
                    "{command}: {bounds:?}"
                );
                assert!(
                    (bounds.max().y() - bounds.min().y() - setback).abs() < 1e-6,
                    "{command}: {bounds:?}"
                );
                assert!(
                    (bounds.max().z() - bounds.min().z() - setback).abs() < 1e-6,
                    "{command}: {bounds:?}"
                );
            }
            assert_eq!(doc.selected_objects().count(), 0);
            let after = doc.objects().cloned().collect::<Vec<_>>();
            assert_eq!(doc.undo_label(), Some(command));
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), vec![before]);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
        }
    }

    #[test]
    fn detached_box_corner_creates_three_edge_patches_and_a_corner_patch() {
        let (registry, doc, id) = selected_box();
        let Geometry::Brep(source) = doc.object(id).unwrap().geometry() else {
            panic!("expected a box")
        };
        let incident = source
            .edges()
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                edge.vertices()
                    .iter()
                    .any(|&vertex| source.vertices()[vertex].point().to_array() == [0.; 3])
            })
            .map(|(index, _)| (id, index))
            .collect::<Vec<_>>();
        for kind in [
            EdgeBlendKind::Fillet(FilletCorner::Automatic),
            EdgeBlendKind::Chamfer(crate::chamfer_edge::ChamferRailType::RollingBall),
        ] {
            let plan = EdgeBlendPlan::capture(&doc, incident.iter().copied(), 1., kind).unwrap();
            let staged = plan
                .with_trim_and_join(false)
                .solve()
                .unwrap_or_else(|error| panic!("{kind:?}: {error}"));
            let mut edges = staged
                .outputs()
                .iter()
                .map(|(_, output)| {
                    let Geometry::Brep(patch) = output else {
                        panic!("expected a detached face")
                    };
                    assert_eq!(patch.faces().len(), 1, "{kind:?}");
                    patch.edges().len()
                })
                .collect::<Vec<_>>();
            edges.sort_unstable();
            assert_eq!(edges, vec![3, 4, 4, 4], "{kind:?}");
        }
        assert_eq!(
            registry
                .component_selection_prompt("FilletEdge TrimAndJoin=No")
                .unwrap()
                .unwrap()
                .kind,
            ComponentSelectionKind::BrepEdge
        );
    }
    #[test]
    fn failed_later_source_leaves_every_object_and_history_unchanged() {
        let (registry, mut doc, first) = selected_box();
        registry.execute(&mut doc, "Box 20,0,0 30,10,0 10").unwrap();
        let second = doc.objects().last().unwrap().id();
        let before = format!("{doc:?}");
        assert!(
            registry
                .execute(&mut doc, &format!("FilletEdge 1 {first} 0 {second} 999"))
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
    #[test]
    fn stale_geometry_invalidates_a_prepared_fillet() {
        let (registry, mut doc, id) = selected_box();
        let prepared = FilletEdgeSelection::prepare(&doc, [(id, 0)], 1.).unwrap();
        registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
        let before = format!("{doc:?}");
        assert!(prepared.commit(&mut doc).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
}
