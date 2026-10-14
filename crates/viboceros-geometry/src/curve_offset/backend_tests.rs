use super::*;
use crate::NurbsCurve;

fn source() -> NurbsCurve {
    NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 2., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}
fn operations() -> CurveOffsetOperations {
    CurveOffsetOperations {
        smooth: |_, _, _, _, _| Ok(None),
        trim: |c, range| c.try_trimmed(range),
        split: NurbsCurve::try_split,
        split_at: NurbsCurve::try_split_at_parameters,
        bezier: NurbsCurve::try_bezier_spans,
    }
}
fn run(source: NurbsCurve, operations: CurveOffsetOperations) -> Result<Curve3, GeometryError> {
    let tolerance = Tolerance::DEFAULT;
    let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    Curve3::NurbsCurve(source).try_offset_with_operations(
        0.125,
        normal,
        tolerance,
        CurveOffsetCornerStyle::Sharp,
        Some(operations),
    )
}

#[test]
fn offset_backend_output_is_independently_checked_and_errors_propagate() {
    let mut ops = operations();
    ops.smooth = |c, _, _, _, _| Ok(Some(c.clone()));
    assert_eq!(run(source(), ops), Err(GeometryError::NurbsOffsetFitLimit));
    ops.smooth = |c, _, _, _, _| Ok(Some(c.try_reparameterized(-4. ..=7.)?));
    assert_eq!(run(source(), ops), Err(GeometryError::NurbsOffsetFitLimit));
    ops.smooth = |_, _, _, _, _| {
        Err(GeometryError::InterpolationBackendFailed {
            message: "offset producer failure".into(),
        })
    };
    assert_eq!(
        run(source(), ops),
        Err(GeometryError::InterpolationBackendFailed {
            message: "offset producer failure".into()
        })
    );
    let expected = run(source(), operations()).unwrap();
    let tolerance = Tolerance::DEFAULT;
    let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    assert_eq!(
        expected,
        Curve3::NurbsCurve(source())
            .try_offset(0.125, normal, tolerance)
            .unwrap()
    );
}

#[test]
fn offset_admission_rejects_stationary_sources_before_invoking_backend() {
    let mut ops = operations();
    ops.smooth = |_, _, _, _, _| panic!("inadmissible source reached producer");
    let p = Point3::try_new(0., 0., 0.).unwrap();
    let source = NurbsCurve::try_new(
        2,
        vec![p, p, Point3::try_new(2., 0., 0.).unwrap()],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert!(matches!(
        run(source, ops),
        Err(GeometryError::Degenerate {
            context: "NURBS offset stationary point"
        })
    ));
}
