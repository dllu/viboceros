//! surfaces seams command regressions.
use super::*;

#[test]
fn changes_surface_seams_in_both_directions_without_losing_document_state() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New SeamSurfaces")
        .unwrap();
    registry
        .execute(&mut document, "Torus 0,0,0 5 1.5")
        .unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut document, "SetObjectName ClosedSurface")
        .unwrap();
    registry
        .execute(&mut document, "Group SeamSurface")
        .unwrap();
    let before = document.object(id).unwrap().clone();
    let Geometry::NurbsSurface(source) = before.geometry() else {
        panic!("Torus must create an exact NURBS surface")
    };
    let expected = source
        .try_change_closed_seam(SurfaceKnotDirection::Both, 5.0, 1.0)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "SrfSeam Parameter=5,1 Direction=Both",)
            .unwrap(),
        "Set the selected NURBS surface seam in U at 5 and V at 1"
    );
    let object = document.object(id).unwrap();
    let Geometry::NurbsSurface(actual) = object.geometry() else {
        panic!("surface seam relocation must retain its native class");
    };
    assert_eq!(actual.degree_u(), expected.degree_u());
    assert_eq!(actual.degree_v(), expected.degree_v());
    assert_eq!(actual.knots_u(), expected.knots_u());
    assert_eq!(actual.knots_v(), expected.knots_v());
    assert_eq!(
        actual.control_points().len(),
        expected.control_points().len()
    );
    for (a, e) in actual
        .control_points()
        .iter()
        .zip(expected.control_points())
    {
        assert!(a.point().distance_to(e.point()).unwrap() < 1e-10);
        assert!((a.weight() - e.weight()).abs() < 1e-12);
    }
    let relocated_geometry = object.geometry().clone();
    assert_eq!(object.attributes(), before.attributes());
    assert!(document.is_selected(id));
    assert!(
        document
            .group_by_name("SeamSurface")
            .unwrap()
            .members()
            .any(|member| member == id)
    );
    assert_eq!(document.undo_label(), Some("SrfSeam"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.object(id).unwrap(), &before);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.object(id).unwrap().geometry(), &relocated_geometry);
}

#[test]
fn surface_seam_uses_a_model_point_and_infers_the_only_closed_direction() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Sphere 0,0,0 5").unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();
    let Geometry::NurbsSurface(source) = document.object(id).unwrap().geometry() else {
        panic!("Sphere must create an exact NURBS surface")
    };
    assert!(source.is_closed_u().unwrap());
    assert!(!source.is_closed_v().unwrap());
    let source = source.clone();
    let u = source.parameter_at_u(0.37).unwrap();
    let v = source.parameter_at_v(0.43).unwrap();
    let pick = source.evaluate(u, v).unwrap();

    registry
        .execute(
            &mut document,
            &format!("SrfSeam {:.17},{:.17},{:.17}", pick.x(), pick.y(), pick.z()),
        )
        .unwrap();
    let Geometry::NurbsSurface(relocated) = document.object(id).unwrap().geometry() else {
        panic!("SrfSeam must retain NURBS surface geometry")
    };
    assert!(
        document
            .tolerance()
            .approx_eq(*relocated.domain_u().start(), u)
    );
    assert_eq!(relocated.domain_v(), source.domain_v());
    assert!(
        relocated
            .evaluate(*relocated.domain_u().start(), v)
            .unwrap()
            .is_near(pick, document.tolerance())
    );
}

#[test]
fn surface_seam_rejects_invalid_open_or_ambiguous_requests_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "SrfSeam",
        "SrfSeam Parameter",
        "SrfSeam Parameter=1,2 Direction=Sideways",
        "SrfSeam 1,2,3 Direction=U Direction=V",
    ] {
        assert!(matches!(
            registry.execute(&mut document, command),
            Err(CommandError::Usage(SURFACE_SEAM_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut document, "SrfSeam 0,0"),
        Err(CommandError::SurfaceSeamRequiresOneSurface { actual: 0 })
    ));

    registry
        .execute(&mut document, "SrfPt 0,0 4,0 4,3 0,3")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "SrfSeam 2,1"),
        Err(CommandError::SurfaceSeamHasNoClosedDirection)
    ));
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.undo_label(), history.as_deref());

    document.clear_selection();
    registry.execute(&mut document, "Torus 10,0,0 5 1").unwrap();
    let torus = document.objects().last().unwrap().id();
    document
        .select_object(torus, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "SrfSeam Parameter=1 Direction=Both"),
        Err(CommandError::SurfaceSeamBothDirectionsRequireUv)
    ));
    registry.execute(&mut document, "Sphere 25,0,0 3").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    assert!(matches!(
        registry.execute(&mut document, "SrfSeam 10,0,0"),
        Err(CommandError::SurfaceSeamRequiresOneSurface { actual: 3 })
    ));
}
