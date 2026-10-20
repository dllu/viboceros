use viboceros_command::{
    CommandRegistry,
    orient_curve_edge::{Edge, Options, Placement, Source},
};
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{Brep, NurbsCurve, Point3, Tolerance};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_from([x, y, z]).unwrap()
}
fn setup() -> (
    Document,
    viboceros_document::ObjectId,
    viboceros_document::ObjectId,
) {
    let mut doc = Document::new(Tolerance::default());
    let source = doc
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![p(0., -3., 1.), p(2., -3., 1.), p(3., -2., 1.)],
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    let target = doc
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                viboceros_command::CommandContext::default().construction_plane,
                [[0., 6.], [0., 4.], [0., 2.]],
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (doc, source, target)
}
#[test]
fn invalid_curve_edge_inputs_preserve_geometry_selection_and_redo() {
    let (mut doc, source, target) = setup();
    let commands = CommandRegistry::with_builtins();
    commands.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for input in [
        format!("OrientCrvToEdge 2,0,0 Edge={target},999"),
        format!("OrientCrvToEdge 2,0,0 Edge={target},0 Face=999"),
        format!("OrientCrvToEdge 2,0,0 Edge={target},0 Copy=Maybe"),
        format!("OrientCrvToEdge 2,0,0 Edge={source},0"),
        format!("OrientCrvToEdge 2,0,0 Edge={target},0 Copy=Yes Copy=No"),
        format!("OrientCrvToEdge 2,0,0 Edge={target},0 SubCrv=0.2,0.2"),
        format!("OrientCrvToEdge 2,0,0 Edge={target},0 SubCrv=NaN,0.8"),
        format!("OrientCrvToEdge 2,0,0 Edge={target},0 SubCrv=-1,0.8"),
        format!("OrientCrvToEdge 2,0,0 Edge={target},0 SubCrv=0.2,2"),
        format!("OrientCrvToEdge 2,0,0 Edge={target},0 SubCrv=0.2,0.8 SubCrv=0.3,0.7"),
    ] {
        assert!(commands.execute(&mut doc, &input).is_err(), "{input}");
        assert_eq!(format!("{doc:?}"), before, "{input}");
    }
}
#[test]
fn curve_edge_placement_is_rigid_and_rejects_nonadjacent_faces() {
    let (doc, source, target) = setup();
    for index in 0..12 {
        assert!(Edge::capture(&doc, target, index, None).is_err());
        let Geometry::Brep(b) = doc.object(target).unwrap().geometry() else {
            unreachable!()
        };
        let face = b
            .faces()
            .iter()
            .position(|f| {
                f.loops()
                    .iter()
                    .any(|l| l.trims().iter().any(|t| t.edge() == Some(index)))
            })
            .unwrap();
        let edge = Edge::capture(&doc, target, index, Some(face)).unwrap();
        let source = Source::capture(&doc, source, Some(p(0., -3., 1.))).unwrap();
        let geometry = source.curve().curve_ref().to_nurbs().unwrap();
        let placement = Placement::new(
            source,
            edge,
            Options::default(),
            viboceros_command::CommandContext::default().construction_plane,
            doc.tolerance(),
        );
        let sample = placement.locate(p(3., 2., 1.)).unwrap();
        let map = placement.transform_at(sample.point()).unwrap();
        assert!(
            map.transform_point(placement.source.base().unwrap().point())
                .unwrap()
                .distance_to(sample.point())
                .unwrap()
                < 1e-10
        );
        let result = geometry.transformed(map).unwrap();
        assert_eq!(result.knots(), geometry.knots());
        assert_eq!(result.degree(), geometry.degree());
        for (a, b) in geometry
            .control_points()
            .iter()
            .zip(result.control_points())
        {
            assert_eq!(a.weight(), b.weight());
        }
        let before = geometry.length(doc.tolerance()).unwrap();
        let after = result.length(doc.tolerance()).unwrap();
        assert!((after - before).abs() < 1e-8);
        for a in [p(0., 0., 0.), p(1., 2., 3.), p(-3., 1., 4.)] {
            for b in [p(1., 0., 0.), p(2., -1., 3.)] {
                assert!(
                    (map.transform_point(a)
                        .unwrap()
                        .distance_to(map.transform_point(b).unwrap())
                        .unwrap()
                        - a.distance_to(b).unwrap())
                    .abs()
                        < 1e-11
                );
            }
        }
    }
}

#[test]
fn spatial_endpoint_tangents_keep_rigid_geometry_and_replay() {
    let (mut doc, source, target) = setup();
    let commands = CommandRegistry::with_builtins();
    doc.replace_object_geometries([(
        source,
        Geometry::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![p(0., -3., 1.), p(2., -3., 1.), p(3., -2., 2.)],
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap(),
        ),
    )])
    .unwrap();
    doc.clear_history().unwrap();
    commands.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let Geometry::Brep(b) = doc.object(target).unwrap().geometry() else {
        unreachable!()
    };
    let face = b
        .faces()
        .iter()
        .position(|f| {
            f.loops()
                .iter()
                .any(|l| l.trims().iter().any(|t| t.edge() == Some(0)))
        })
        .unwrap();
    let before = doc.object(source).unwrap().geometry().clone();
    let target_before = doc.object(target).unwrap().geometry_snapshot().clone();
    commands
        .execute(
            &mut doc,
            &format!("OrientCrvToEdge 2,0,0 Edge={target},0 Face={face} PickPoint=0,-3,1 Copy=No"),
        )
        .unwrap();
    let after = doc.object(source).unwrap().geometry().clone();
    let a = before.curve_ref().unwrap();
    let b = after.curve_ref().unwrap();
    assert!((a.length(doc.tolerance()).unwrap() - b.length(doc.tolerance()).unwrap()).abs() < 1e-9);
    assert!(
        b.evaluate(*b.domain().end())
            .unwrap()
            .distance_to(p(2., 0., 0.))
            .unwrap()
            < 1e-12
    );
    assert!(target_before.shares_storage_with(doc.object(target).unwrap().geometry_snapshot()));
    assert_eq!(doc.undo().unwrap().as_deref(), Some("OrientCrvToEdge"));
    assert_eq!(doc.object(source).unwrap().geometry(), &before);
    doc.redo().unwrap();
    assert_eq!(doc.object(source).unwrap().geometry(), &after);
}
