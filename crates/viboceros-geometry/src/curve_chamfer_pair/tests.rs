use super::*;
use crate::{CircularArc3, NurbsCurve, try_connect_curves_parts_with_styles};

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

fn line(a: Point3, b: Point3) -> Curve3 {
    Curve3::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
}

#[test]
fn unequal_setbacks_and_extended_lines() {
    let joined = try_chamfer_curves_joined(
        &line(p(0., 0.), p(3., 0.)),
        p(2.9, 0.),
        &line(p(4., 1.), p(4., 4.)),
        p(4., 1.1),
        1.,
        0.5,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(joined.segments().len(), 3);
    let points = joined
        .segments()
        .iter()
        .map(|segment| {
            let CurveSegment3::Line(line) = segment else {
                panic!("straight chamfer")
            };
            [line.start(), line.end()]
        })
        .collect::<Vec<_>>();
    assert_eq!(
        points,
        vec![
            [p(0., 0.), p(3., 0.)],
            [p(3., 0.), p(4., 0.5)],
            [p(4., 0.5), p(4., 4.)],
        ]
    );
}

#[test]
fn separate_pieces_preserve_source_directions() {
    let parts = try_chamfer_curves_parts(
        &line(p(4., 0.), p(0., 0.)),
        p(3.8, 0.),
        &line(p(4., 4.), p(4., 0.)),
        p(4., 0.2),
        [0.5, 1.],
        true,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let [
        Curve3::Line(first),
        Curve3::Line(second),
        Curve3::Line(bevel),
    ] = parts.as_slice()
    else {
        panic!("three straight pieces");
    };
    assert_eq!((first.start(), first.end()), (p(3.5, 0.), p(0., 0.)));
    assert_eq!((second.start(), second.end()), (p(4., 4.), p(4., 1.)));
    assert_eq!((bevel.start(), bevel.end()), (p(3.5, 0.), p(4., 1.)));
}

#[test]
fn oblique_setbacks_follow_each_curve_length() {
    let joined = try_chamfer_curves_joined(
        &line(p(0., 0.), p(4., 0.)),
        p(3.8, 0.),
        &line(p(4., 0.), p(6., 2.)),
        p(4.2, 0.2),
        1.,
        1.,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let CurveSegment3::Line(bevel) = &joined.segments()[1] else {
        panic!("straight bevel");
    };
    assert!(bevel.start().distance_to(p(3., 0.)).unwrap() < 1e-12);
    let diagonal = 2.0_f64.sqrt() / 2.0;
    assert!(bevel.end().distance_to(p(4. + diagonal, diagonal)).unwrap() < 1e-12);
}

#[test]
fn skew_lines_have_no_chamfer() {
    let raised = |x: Real, y: Real| Point3::try_new(x, y, 1.).unwrap();
    assert!(
        try_chamfer_curves_joined(
            &line(p(0., 0.), p(4., 0.)),
            p(3.8, 0.),
            &line(raised(4., 0.), raised(4., 4.)),
            raised(4., 0.2),
            0.5,
            0.5,
            Tolerance::DEFAULT,
        )
        .is_err()
    );
}

#[test]
fn meeting_arc_and_line_trim_at_chord_setbacks() {
    let arc = CircularArc3::try_from_three_points(
        p(1., 0.),
        p(2.0_f64.sqrt() / 2., 2.0_f64.sqrt() / 2.),
        p(0., 1.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let joined = try_chamfer_curves_joined(
        &Curve3::Arc(arc),
        p(0.1, 0.99),
        &line(p(0., 1.), p(-2., 1.)),
        p(-0.1, 1.),
        2. * (std::f64::consts::PI / 12.).sin(),
        0.5,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let [
        CurveSegment3::Arc(retained),
        CurveSegment3::Line(bevel),
        CurveSegment3::Line(_),
    ] = joined.segments()
    else {
        panic!("arc, bevel, line");
    };
    assert!((retained.length().unwrap() - std::f64::consts::PI / 3.).abs() < 1e-10);
    assert!(
        bevel
            .start()
            .distance_to(p(0.5, 3.0_f64.sqrt() / 2.))
            .unwrap()
            < 1e-10
    );
    assert!(bevel.end().distance_to(p(-0.5, 1.)).unwrap() < 1e-10);
}

#[test]
fn meeting_nurbs_and_line_retain_native_nurbs_leaf() {
    let tolerance = Tolerance::DEFAULT;
    let nurbs = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(2., 0.), p(2., 2.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let joined = try_chamfer_curves_joined(
        &Curve3::NurbsCurve(nurbs),
        p(2., 1.9),
        &line(p(2., 2.), p(6., 2.)),
        p(2.1, 2.),
        0.3,
        0.4,
        tolerance,
    )
    .unwrap();
    let [
        CurveSegment3::NurbsCurve(retained),
        CurveSegment3::Line(bevel),
        CurveSegment3::Line(_),
    ] = joined.segments()
    else {
        panic!("NURBS, bevel, line");
    };
    assert!(
        (retained
            .evaluate(*retained.domain().end())
            .unwrap()
            .distance_to(p(2., 2.))
            .unwrap()
            - 0.3)
            .abs()
            < 1e-10
    );
    assert!(
        retained
            .evaluate(*retained.domain().end())
            .unwrap()
            .distance_to(bevel.start())
            .unwrap()
            < 1e-9
    );
    assert!(bevel.end().distance_to(p(2.4, 2.)).unwrap() < 1e-9);
}

#[test]
fn nonmeeting_arc_extends_on_its_circle_before_chamfer() {
    let arc = CircularArc3::try_from_three_points(
        p(1., 0.),
        p(2.0_f64.sqrt() / 2., 2.0_f64.sqrt() / 2.),
        p(0., 1.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let joined = try_chamfer_curves_joined_with_styles(
        &Curve3::Arc(arc),
        p(0., 1.),
        &line(p(-1., 2.), p(-1., 3.)),
        p(-1., 2.),
        [0.2, 0.3],
        CurveChamferExtensionStyles::default(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let [
        CurveSegment3::Arc(retained),
        CurveSegment3::Line(bevel),
        CurveSegment3::Line(line),
    ] = joined.segments()
    else {
        panic!("arc, bevel, retained line")
    };
    assert!((retained.radius() - 1.0).abs() < 1e-12);
    let angle = 2. * 0.1_f64.asin();
    assert!((retained.length().unwrap() - (std::f64::consts::PI - angle)).abs() < 1e-9);
    assert!(
        bevel
            .start()
            .distance_to(p(-angle.cos(), angle.sin()))
            .unwrap()
            < 1e-9
    );
    assert!(bevel.end().distance_to(p(-1., 0.3)).unwrap() < 1e-9);
    assert!(line.start().distance_to(bevel.end()).unwrap() < 1e-9);
}

#[test]
fn tangent_arc_extension_keeps_its_leaf_and_separate_bevel() {
    let arc = Curve3::Arc(
        CircularArc3::try_from_three_points(
            p(1., 0.),
            p(2.0_f64.sqrt() / 2., 2.0_f64.sqrt() / 2.),
            p(0., 1.),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    let line = line(p(-1., 2.), p(-1., 3.));
    let styles = CurveChamferExtensionStyles {
        arc: CurveArcExtensionStyle::Line,
        ..CurveChamferExtensionStyles::default()
    };
    let joined = try_chamfer_curves_joined_with_styles(
        &arc,
        p(0., 1.),
        &line,
        p(-1., 2.),
        [0.2, 0.3],
        styles,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let [
        CurveSegment3::Arc(retained_arc),
        CurveSegment3::Line(extension),
        CurveSegment3::Line(bevel),
        CurveSegment3::Line(_),
    ] = joined.segments()
    else {
        panic!("arc, tangent extension, bevel, retained line")
    };
    let Curve3::Arc(source_arc) = &arc else {
        unreachable!()
    };
    assert_eq!(retained_arc, source_arc);
    assert!(extension.start().distance_to(p(0., 1.)).unwrap() < 1e-9);
    assert!(extension.end().distance_to(p(-0.8, 1.)).unwrap() < 1e-9);
    assert!(bevel.end().distance_to(p(-1., 1.3)).unwrap() < 1e-9);

    let separate = try_chamfer_curves_parts_with_styles(
        (&arc, p(0., 1.)),
        (&line, p(-1., 2.)),
        [0.2, 0.3],
        true,
        styles,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(separate.len(), 3);
    let bevel_only = try_chamfer_curves_parts_with_styles(
        (&arc, p(0., 1.)),
        (&line, p(-1., 2.)),
        [0.2, 0.3],
        false,
        styles,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(bevel_only, vec![Curve3::Line(*bevel)]);
}

#[test]
fn setback_can_cross_tangent_extension_into_original_arc() {
    let arc = Curve3::Arc(
        CircularArc3::try_from_three_points(
            p(1., 0.),
            p(2.0_f64.sqrt() / 2., 2.0_f64.sqrt() / 2.),
            p(0., 1.),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    let line = line(p(-1., 2.), p(-1., 3.));
    let styles = CurveChamferExtensionStyles {
        arc: CurveArcExtensionStyle::Line,
        ..CurveChamferExtensionStyles::default()
    };
    let joined = try_chamfer_curves_joined_with_styles(
        &arc,
        p(0., 1.),
        &line,
        p(-1., 2.),
        [1.2, 0.3],
        styles,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let [
        CurveSegment3::Arc(retained),
        CurveSegment3::Line(bevel),
        CurveSegment3::Line(_),
    ] = joined.segments()
    else {
        panic!("trimmed original arc, bevel, retained line")
    };
    assert!((bevel.start().distance_to(p(-1., 1.)).unwrap() - 1.2).abs() < 1e-9);
    assert!(retained.length().unwrap() < std::f64::consts::FRAC_PI_2);
    assert!(bevel.end().distance_to(p(-1., 1.3)).unwrap() < 1e-9);
    let parts = try_chamfer_curves_parts_with_styles(
        (&arc, p(0., 1.)),
        (&line, p(-1., 2.)),
        [1.2, 0.3],
        true,
        styles,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], Curve3::Arc(*retained));
    assert_eq!(parts[2], Curve3::Line(*bevel));
}

#[test]
fn smooth_nurbs_extension_is_set_back_by_chord() {
    let nurbs = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(1., 0.), p(2., 1.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let source = Curve3::NurbsCurve(nurbs);
    let target = line(p(3., 3.), p(3., 4.));
    let connected = try_connect_curves_parts_with_styles(
        &source,
        p(2., 1.),
        &target,
        p(3., 3.),
        CurveArcExtensionStyle::Arc,
        CurveOtherExtensionStyle::Smooth,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let corner = connected[0].as_ref().end_point().unwrap();
    let joined = try_chamfer_curves_joined_with_styles(
        &source,
        p(2., 1.),
        &target,
        p(3., 3.),
        [0.2, 0.3],
        CurveChamferExtensionStyles {
            other: CurveOtherExtensionStyle::Smooth,
            ..CurveChamferExtensionStyles::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    let [
        CurveSegment3::NurbsCurve(retained),
        CurveSegment3::Line(bevel),
        CurveSegment3::Line(line),
    ] = joined.segments()
    else {
        panic!("NURBS, bevel, retained line")
    };
    assert!((bevel.start().distance_to(corner).unwrap() - 0.2).abs() < 1e-9);
    assert_eq!(retained.degree(), 2);
    assert!(bevel.end().distance_to(p(3., 2.55)).unwrap() < 1e-9);
    assert!(line.start().distance_to(bevel.end()).unwrap() < 1e-9);
}

#[test]
fn setback_can_cross_tangent_extension_into_original_nurbs() {
    let nurbs = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(1., 0.), p(2., 1.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let original_length = crate::CurveRef::NurbsCurve(&nurbs)
        .length(Tolerance::DEFAULT)
        .unwrap();
    let source = Curve3::NurbsCurve(nurbs);
    let target = line(p(3., 3.), p(3., 4.));
    let joined = try_chamfer_curves_joined_with_styles(
        &source,
        p(2., 1.),
        &target,
        p(3., 3.),
        [2.0_f64.sqrt() + 0.2, 0.3],
        CurveChamferExtensionStyles::default(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let [
        CurveSegment3::NurbsCurve(retained),
        CurveSegment3::Line(bevel),
        CurveSegment3::Line(_),
    ] = joined.segments()
    else {
        panic!("trimmed original NURBS, bevel, retained line")
    };
    assert!((bevel.start().distance_to(p(3., 2.)).unwrap() - (2.0_f64.sqrt() + 0.2)).abs() < 1e-9);
    assert!(
        retained
            .evaluate(*retained.domain().end())
            .unwrap()
            .distance_to(p(2., 1.))
            .unwrap()
            > 0.1
    );
    assert!(bevel.end().distance_to(p(3., 2.3)).unwrap() < 1e-9);
    assert!(
        try_chamfer_curves_joined_with_styles(
            &source,
            p(2., 1.),
            &target,
            p(3., 3.),
            [2.0_f64.sqrt() + original_length, 0.3],
            CurveChamferExtensionStyles::default(),
            Tolerance::DEFAULT,
        )
        .is_err()
    );
}
