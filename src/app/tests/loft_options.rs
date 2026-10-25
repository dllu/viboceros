use super::*;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn loft_splitting_options_use_the_ordinary_command_entry_and_preserve_undo() {
    let mut app = test_app();
    let ids = [(1., 0.), (2., 2.), (1., 4.)]
        .into_iter()
        .map(|(radius, z)| {
            let curve = viboceros_geometry::NurbsCurve::try_new(
                1,
                [
                    [0., 0.],
                    [radius, 0.],
                    [radius, radius],
                    [0., radius],
                    [0., 0.],
                ]
                .into_iter()
                .map(|p| Point3::try_new(p[0], p[1], z).unwrap())
                .collect(),
                vec![0., 0., 0.25, 0.5, 0.75, 1., 1.],
            )
            .unwrap();
            app.document
                .add_geometry(Geometry::NurbsCurve(curve))
                .unwrap()
        })
        .collect::<Vec<_>>();
    app.document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "CreaseSplitting Disable");
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "Loft SplitAtTangents=No Natural");
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Loft B-rep")
    };
    assert_eq!(brep.faces().len(), 1);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Loft SplitAtTangents=Yes Natural");
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Loft B-rep")
    };
    assert_eq!(brep.faces().len(), 4);
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "CreaseSplitting Enable");
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.commands.crease_splitting());
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn point_ended_loft_uses_command_entry_and_restores_its_poles_on_redo() {
    let mut app = test_app();
    let tolerance = app.document.tolerance();
    let circle = viboceros_geometry::Circle3::try_new(
        Point3::try_new(0., 0., 1.).unwrap(),
        1.,
        viboceros_geometry::UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let id = app
        .document
        .add_geometry(Geometry::NurbsCurve(circle))
        .unwrap();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Loft StartPoint=0,0,-1 EndPoint=0,0,3");
    let output = app.document.objects().find(|o| o.id() != id).unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        panic!("Loft point-ended B-rep")
    };
    assert!(brep.is_solid());
    assert_eq!((brep.vertices().len(), brep.edges().len()), (2, 1));
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
}
