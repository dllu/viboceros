use super::*;

#[derive(Debug, PartialEq)]
enum Failure {
    Geometry(GeometryError),
    Producer,
}
impl From<GeometryError> for Failure {
    fn from(error: GeometryError) -> Self {
        Self::Geometry(error)
    }
}

#[test]
fn producer_failures_propagate_through_connect_fillet_and_chamfer_without_mutating_sources() {
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let first = Curve3::NurbsCurve(
        NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(1., 0.), p(2., 1.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap(),
    );
    let second = Curve3::NurbsCurve(
        NurbsCurve::try_new(
            2,
            vec![p(3., 3.), p(3., 4.), p(3.2, 5.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap(),
    );
    let before = [first.clone(), second.clone()];
    for fail_trim in [false, true] {
        let mut extend = |curve: &NurbsCurve, range| {
            if !fail_trim {
                Err(Failure::Producer)
            } else {
                Ok(curve.try_extended_to(range)?)
            }
        };
        let mut trim = |_: &NurbsCurve, _| Err(Failure::Producer);
        let mut clamp =
            |_: &NurbsCurve, _| panic!("Connect must not clamp an unchanged source endpoint");
        let mut operations = CurveExtensionOperations {
            extend: &mut extend,
            trim: &mut trim,
            clamp_endpoint: &mut clamp,
        };
        let a = (&first, p(2., 1.));
        let b = (&second, p(3., 3.));
        assert_eq!(
            try_connect_curves_parts_with_operations(
                a,
                b,
                CurveArcExtensionStyle::Arc,
                CurveOtherExtensionStyle::Smooth,
                Tolerance::DEFAULT,
                &mut operations
            ),
            Err(Failure::Producer)
        );
        assert_eq!(
            crate::try_fillet_curves_joined_with_operations(
                a,
                b,
                0.1,
                crate::CurveFilletExtensionStyles {
                    arc: CurveArcExtensionStyle::Line,
                    other: CurveOtherExtensionStyle::Smooth
                },
                Tolerance::DEFAULT,
                &mut operations
            ),
            Err(Failure::Producer)
        );
        assert_eq!(
            crate::try_chamfer_curves_joined_with_operations(
                a,
                b,
                [0.1, 0.2],
                crate::CurveChamferExtensionStyles {
                    arc: CurveArcExtensionStyle::Arc,
                    other: CurveOtherExtensionStyle::Smooth
                },
                Tolerance::DEFAULT,
                &mut operations
            ),
            Err(Failure::Producer)
        );
        assert_eq!([first.clone(), second.clone()], before);
        assert!(matches!(
            try_connect_curves_parts_with_operations(
                (&first, p(1., 0.5)),
                b,
                CurveArcExtensionStyle::Arc,
                CurveOtherExtensionStyle::Smooth,
                Tolerance::DEFAULT,
                &mut operations
            ),
            Err(Failure::Geometry(_))
        ));
    }
}

#[test]
fn meeting_nurbs_keep_connect_sources_exact_and_propagate_chamfer_extraction_failure() {
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let first = Curve3::NurbsCurve(
        NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(1., 0.), p(2., 1.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap(),
    );
    let second =
        Curve3::Line(LineSegment::try_new(p(2., 1.), p(2., 4.), Tolerance::DEFAULT).unwrap());
    let a = (&first, p(2., 1.));
    let b = (&second, p(2., 1.));
    let mut extend = |_: &NurbsCurve, _| panic!("meeting curves must not extrapolate");
    let mut trim = |_: &NurbsCurve, _| Err(Failure::Producer);
    let mut clamp = |_: &NurbsCurve, _| panic!("meeting curves must not clamp");
    let mut operations = CurveExtensionOperations {
        extend: &mut extend,
        trim: &mut trim,
        clamp_endpoint: &mut clamp,
    };
    assert_eq!(
        try_connect_curves_parts_with_operations(
            a,
            b,
            CurveArcExtensionStyle::Arc,
            CurveOtherExtensionStyle::Smooth,
            Tolerance::DEFAULT,
            &mut operations,
        )
        .unwrap(),
        vec![first.clone(), second.clone()]
    );
    assert_eq!(
        crate::try_chamfer_curves_joined_with_operations(
            a,
            b,
            [0.1, 0.2],
            crate::CurveChamferExtensionStyles {
                arc: CurveArcExtensionStyle::Arc,
                other: CurveOtherExtensionStyle::Smooth,
            },
            Tolerance::DEFAULT,
            &mut operations,
        ),
        Err(Failure::Producer)
    );
}
