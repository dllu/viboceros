//! refinement uniform command regressions.
use super::*;

#[test]
fn make_uniform_replaces_curve_knots_without_losing_identity_or_controls() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let input_layer = document
        .add_layer("Uniform source", ColorRgb::new(35, 70, 105))
        .unwrap();
    let controls = vec![
        WeightedPoint3::try_new(Point3::try_new(0.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
        WeightedPoint3::try_new(Point3::try_new(1.0, 2.0, 0.0).unwrap(), 0.5).unwrap(),
        WeightedPoint3::try_new(Point3::try_new(3.0, 1.0, 0.0).unwrap(), 2.0).unwrap(),
        WeightedPoint3::try_new(Point3::try_new(4.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
    ];
    let source =
        NurbsCurve::try_new_rational(2, controls.clone(), vec![0.0, 0.0, 0.0, 0.2, 1.0, 1.0, 1.0])
            .unwrap();
    let source_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(source.clone()),
            ObjectAttributes::on_layer(input_layer).with_name("weighted source"),
        )
        .unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(20.0, 30.0, 40.0).unwrap()))
        .unwrap();
    document
        .select_objects([source_id, point_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "MakeUniform").unwrap(),
        "Made 1 object(s) uniform without changing control points (1 curve(s), 0 NURBS surface(s))"
    );
    assert_eq!(document.objects().len(), 2);
    let object = document.object(source_id).unwrap();
    assert_eq!(object.attributes().layer_id(), input_layer);
    assert_eq!(object.attributes().name(), Some("weighted source"));
    assert!(document.is_selected(source_id));
    assert!(document.is_selected(point_id));
    let Geometry::NurbsCurve(curve) = object.geometry() else {
        panic!("MakeUniform must replace eligible curves with NURBS geometry")
    };
    assert_eq!(curve.control_points(), controls);
    assert_eq!(curve.knots(), &[0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]);
    assert_eq!(document.undo_label(), Some("MakeUniform"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source)
    );
}

#[test]
fn make_uniform_validates_arguments_and_curve_selection_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects([point_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "MakeUniform"),
        Err(CommandError::NoMakeUniformObjects)
    ));
    assert!(matches!(
        registry.execute(&mut document, "MakeUniformUV"),
        Err(CommandError::NoMakeUniformUvSurfaces)
    ));

    let line_id = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(5.0, 0.0, 0.0).unwrap(),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects([line_id], SelectionMode::Replace)
        .unwrap();
    let before = document.object(line_id).unwrap().geometry().clone();
    let history = document.undo_label().map(str::to_owned);
    for command in ["MakeUniform extra", "MakeUniform Direction=U"] {
        assert!(matches!(
            registry.execute(&mut document, command),
            Err(CommandError::Usage(MAKE_UNIFORM_USAGE))
        ));
    }
    assert_eq!(document.object(line_id).unwrap().geometry(), &before);
    assert_eq!(document.undo_label(), history.as_deref());

    registry.execute(&mut document, "_MakeUniform").unwrap();
    let Geometry::NurbsCurve(curve) = document.object(line_id).unwrap().geometry() else {
        panic!("MakeUniform must convert analytic curves to NURBS")
    };
    assert_eq!(curve.knots(), &[0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn make_uniform_uv_defaults_to_u_and_changes_the_surface_in_place() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document
        .add_layer("Uniform surface", ColorRgb::new(45, 90, 135))
        .unwrap();
    let source = rational_multi_span_surface();
    let source_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(source.clone()),
            ObjectAttributes::on_layer(layer).with_name("weighted surface"),
        )
        .unwrap();
    document
        .select_objects([source_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "MakeUniformUV").unwrap(),
        "Made 1 NURBS surface(s) uniform in the U direction without changing control points"
    );
    let object = document.object(source_id).unwrap();
    assert_eq!(object.attributes().layer_id(), layer);
    assert_eq!(object.attributes().name(), Some("weighted surface"));
    assert!(document.is_selected(source_id));
    let Geometry::NurbsSurface(surface) = object.geometry() else {
        panic!("MakeUniformUV must retain NURBS surface geometry")
    };
    assert_eq!(surface.control_points(), source.control_points());
    assert_eq!(surface.knots_u(), &[0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0]);
    assert_eq!(surface.knots_v(), source.knots_v());
    assert_eq!(document.undo_label(), Some("MakeUniformUV"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source_id).unwrap().geometry(),
        &Geometry::NurbsSurface(source)
    );
}

#[test]
fn make_uniform_uv_accepts_v_direction() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = rational_multi_span_surface();
    let surface_id = document
        .add_geometry(Geometry::NurbsSurface(source.clone()))
        .unwrap();
    document
        .select_objects([surface_id], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "MakeUniformUV Direction _V")
        .unwrap();
    let Geometry::NurbsSurface(surface) = document.object(surface_id).unwrap().geometry() else {
        panic!("MakeUniformUV must retain NURBS surface geometry")
    };
    assert_eq!(surface.knots_u(), source.knots_u());
    assert_eq!(surface.knots_v(), &[0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0]);
}

#[test]
fn make_uniform_uniformizes_both_surface_directions() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = rational_multi_span_surface();
    let surface_id = document
        .add_geometry(Geometry::NurbsSurface(source.clone()))
        .unwrap();
    document
        .select_objects([surface_id], SelectionMode::Replace)
        .unwrap();

    registry.execute(&mut document, "MakeUniform").unwrap();
    let Geometry::NurbsSurface(surface) = document.object(surface_id).unwrap().geometry() else {
        panic!("MakeUniform must retain NURBS surface geometry")
    };
    assert_eq!(surface.control_points(), source.control_points());
    assert_eq!(surface.knots_u(), &[0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0]);
    assert_eq!(surface.knots_v(), &[0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0]);
}

#[test]
fn make_uniform_uv_rejects_invalid_surface_directions_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let surface_id = document
        .add_geometry(Geometry::NurbsSurface(rational_multi_span_surface()))
        .unwrap();
    document
        .select_objects([surface_id], SelectionMode::Replace)
        .unwrap();
    let before = document.object(surface_id).unwrap().geometry().clone();
    let history = document.undo_label().map(str::to_owned);

    for command in [
        "MakeUniformUV Direction=Other",
        "MakeUniformUV Direction=Both",
        "MakeUniformUV Direction=U Direction=V",
        "MakeUniformUV Other=U",
    ] {
        assert!(
            matches!(
                registry.execute(&mut document, command),
                Err(CommandError::Usage(MAKE_UNIFORM_UV_USAGE))
            ),
            "{command}"
        );
        assert_eq!(
            document.object(surface_id).unwrap().geometry(),
            &before,
            "{command}"
        );
        assert_eq!(document.undo_label(), history.as_deref(), "{command}");
    }
}
