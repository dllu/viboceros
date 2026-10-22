//! curves creation command regressions.
use super::*;

#[test]
fn creates_a_clamped_control_point_curve() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "ControlPointCurve 3 0,0 2,3 5,3 8,0")
        .unwrap();
    let object = document.objects().next().unwrap();
    let Geometry::NurbsCurve(curve) = object.geometry() else {
        panic!("expected a NURBS curve")
    };
    assert_eq!(curve.degree(), 3);
    assert_eq!(curve.control_points().len(), 4);
    assert_eq!(
        curve.evaluate(0.0).unwrap(),
        Point3::try_new(0.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        curve.evaluate(1.0).unwrap(),
        Point3::try_new(8.0, 0.0, 0.0).unwrap()
    );
}

#[test]
fn curve_preserves_controls_and_matches_rhino_degree_rules() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let controls = [
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 2.0, 0.5).unwrap(),
        Point3::try_new(4.0, -1.0, 2.0).unwrap(),
        Point3::try_new(4.5, 3.0, -0.5).unwrap(),
        Point3::try_new(10.0, 0.0, 1.0).unwrap(),
    ];
    let message = registry
        .execute(
            &mut document,
            "Curve 0,0,0 1,2,.5 4,-1,2 4.5,3,-.5 10,0,1 Degree=3",
        )
        .unwrap();
    assert!(message.contains("degree 3 curve"));
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("expected a control-point NURBS curve")
    };
    assert_eq!(curve.degree(), 3);
    assert_eq!(
        curve
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        controls
    );
    let domain_end = 17.976_753_701_093_052;
    for (actual, expected) in curve.knots().iter().zip([
        0.0,
        0.0,
        0.0,
        0.0,
        domain_end / 2.0,
        domain_end,
        domain_end,
        domain_end,
        domain_end,
    ]) {
        assert!((actual - expected).abs() <= 2.0e-14);
    }

    let message = registry
        .execute(&mut document, "Curve Degree=5 0,0 2,3 10,0")
        .unwrap();
    assert!(message.contains("degree 2 curve"));
    let Geometry::NurbsCurve(lowered) = document.objects().nth(1).unwrap().geometry() else {
        panic!("expected a lowered-degree NURBS curve")
    };
    assert_eq!(lowered.degree(), 2);

    let message = registry
        .execute(
            &mut document,
            "Curve Degree=15 0,0 1,0 2,0 3,0 4,0 5,0 6,0 7,0 8,0 9,0 10,0 11,0",
        )
        .unwrap();
    assert!(message.contains("degree 11 curve"));
    let Geometry::NurbsCurve(clamped) = document.objects().nth(2).unwrap().geometry() else {
        panic!("expected a maximum-degree NURBS curve")
    };
    assert_eq!(clamped.degree(), 11);
}

#[test]
fn curve_supports_smooth_periodic_and_sharp_closed_seams() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let message = registry
            .execute(
                &mut document,
                "Curve Degree=4 Close=Smooth 0,0,0 1,2,.5 4,-1,2 4.5,3,-.5 10,0,1 8,-4,2 2,-3,-1 -2,1,.25",
            )
            .unwrap();
    assert!(message.contains("periodic degree 4 curve"));
    let Geometry::NurbsCurve(periodic) = document.objects().next().unwrap().geometry() else {
        panic!("expected a periodic control-point curve")
    };
    assert_eq!(periodic.degree(), 4);
    assert_eq!(periodic.control_points().len(), 12);
    assert!(periodic.is_periodic());
    assert!(periodic.is_closed().unwrap());

    let message = registry
        .execute(
            &mut document,
            "Curve Close=Sharp Degree=5 0,0,0 2,3,1 10,0,0",
        )
        .unwrap();
    assert!(message.contains("sharp closed degree 3 curve"));
    let Geometry::NurbsCurve(sharp) = document.objects().nth(1).unwrap().geometry() else {
        panic!("expected a sharp closed control-point curve")
    };
    assert_eq!(sharp.degree(), 3);
    assert_eq!(sharp.control_points().len(), 4);
    assert!(!sharp.is_periodic());
    assert!(sharp.is_closed().unwrap());

    let message = registry
        .execute(&mut document, "Curve Degree=1 Close=Smooth 0,0 2,3 10,0")
        .unwrap();
    assert!(message.contains("closed degree 1 curve"));
    let Geometry::NurbsCurve(linear) = document.objects().nth(2).unwrap().geometry() else {
        panic!("expected a closed degree-one control-point curve")
    };
    assert!(linear.is_closed().unwrap());
    assert!(!linear.is_periodic());
}

#[test]
fn invalid_curve_arguments_are_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for input in [
        "Curve 0,0",
        "Curve 0,0 1,1 Degree=3 Degree=4",
        "Curve 0,0 1,1 Knots=Chord",
        "Curve 0,0 1,1 2,0 Close=Bad",
        "Curve 0,0 1,1 Close=Smooth",
        "Curve 0,0 1,1 2,0 Close=Sharp Close=Smooth",
    ] {
        assert!(registry.execute(&mut document, input).is_err(), "{input}");
        assert_eq!(document.objects().len(), 0);
    }
}

#[test]
fn creates_interpolated_curves_with_rhino_style_options() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let message = registry
        .execute(&mut document, "InterpCrv 0,0 1,2 4,-1 6,0")
        .unwrap();
    assert!(message.contains("degree 3 interpolated curve"));
    let Geometry::NurbsCurve(default_curve) = document.objects().next().unwrap().geometry() else {
        panic!("expected an interpolated NURBS curve")
    };
    let expected_points = [
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 2.0, 0.0).unwrap(),
        Point3::try_new(4.0, -1.0, 0.0).unwrap(),
        Point3::try_new(6.0, 0.0, 0.0).unwrap(),
    ];
    let mut parameter = 0.0;
    for (index, expected) in expected_points.into_iter().enumerate() {
        assert!(
            default_curve
                .evaluate(parameter)
                .unwrap()
                .is_near(expected, Tolerance::DEFAULT)
        );
        if let Some(next) = expected_points.get(index + 1) {
            parameter += expected.distance_to(*next).unwrap();
        }
    }

    registry
        .execute(
            &mut document,
            "InterpCurve Knots=SqrtChrd Close=Smooth 0,0 3,0 3,2 0,2",
        )
        .unwrap();
    let Geometry::NurbsCurve(periodic) = document.objects().nth(1).unwrap().geometry() else {
        panic!("expected a periodic NURBS curve")
    };
    assert!(periodic.is_periodic());
    assert!(periodic.is_closed().unwrap());

    registry
        .execute(
            &mut document,
            "InterpCrv Degree=3 StartTangent=-1,0,0 EndTangent=0,1,0 10,0 12,2",
        )
        .unwrap();
    let Geometry::NurbsCurve(tangent_curve) = document.objects().nth(2).unwrap().geometry() else {
        panic!("expected a tangent-constrained NURBS curve")
    };
    assert_eq!(tangent_curve.degree(), 3);
    assert_eq!(tangent_curve.control_points().len(), 4);
    assert!(tangent_curve.control_points()[1].point().x() < 10.0);

    let message = registry
        .execute(&mut document, "InterpCrv 20,0 24,0")
        .unwrap();
    assert!(message.contains("degree 3 interpolated curve"));

    let message = registry
        .execute(&mut document, "InterpCrv Degree=1 30,0 34,0")
        .unwrap();
    assert!(message.contains("degree 1 interpolated curve"));
}

#[test]
fn invalid_interp_crv_options_are_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for input in [
        "InterpCrv 0,0 1,1 Degree=2",
        "InterpCrv 0,0 1,1 Knots=Bad",
        "InterpCrv 0,0 1,1 Degree=3 Degree=3",
        "InterpCrv 0,0 1,1 Close=Smooth StartTangent=1,0,0",
    ] {
        assert!(registry.execute(&mut document, input).is_err(), "{input}");
        assert_eq!(document.objects().len(), 0);
    }
}
