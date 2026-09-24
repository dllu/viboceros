//! Straight chamfers between selected ends of open curves.

use crate::{
    Curve3, CurveArcExtensionStyle, CurveOtherExtensionStyle, CurveRef, CurveSegment3,
    GeometryError, LineSegment, Point3, PolyCurve3, Real, Tolerance,
    curve::ArcLengthSampler,
    curve_connect_pair::connected_ends_with_styles,
    curve_pair_support::{
        curve_from_segments, meeting_lines, oriented, original_direction, selected_end,
    },
};

/// Styles used to extend nonmeeting curve ends before chamfering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurveChamferExtensionStyles {
    pub arc: CurveArcExtensionStyle,
    pub other: CurveOtherExtensionStyle,
}

impl Default for CurveChamferExtensionStyles {
    fn default() -> Self {
        Self {
            arc: CurveArcExtensionStyle::Arc,
            other: CurveOtherExtensionStyle::Line,
        }
    }
}

/// Connects the selected curve ends using the requested extension styles,
/// then measures each chamfer setback along the connected curves.
pub fn try_chamfer_curves_joined_with_styles(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    distances: [Real; 2],
    styles: CurveChamferExtensionStyles,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    let (connected, picks) = connected_ends_with_styles(
        first,
        first_pick,
        second,
        second_pick,
        styles.arc,
        styles.other,
        tolerance,
    )?;
    Ok(chamfer_connected_curves(
        &connected[0],
        picks[0],
        &connected[1],
        picks[1],
        distances,
        tolerance,
    )?
    .0)
}

/// Creates independent retained curves and a chamfer with extension options.
pub fn try_chamfer_curves_parts_with_styles(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    distances: [Real; 2],
    trim: bool,
    styles: CurveChamferExtensionStyles,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    let (connected, picks) = connected_ends_with_styles(
        first.0,
        first.1,
        second.0,
        second.1,
        styles.arc,
        styles.other,
        tolerance,
    )?;
    let reverse_first = !selected_end(&connected[0], picks[0], tolerance)?;
    let reverse_second = selected_end(&connected[1], picks[1], tolerance)?;
    let (joined, first_count, has_bevel) = chamfer_connected_curves(
        &connected[0],
        picks[0],
        &connected[1],
        picks[1],
        distances,
        tolerance,
    )?;
    chamfer_parts_from_joined(
        &joined,
        first_count,
        has_bevel,
        trim,
        reverse_first,
        reverse_second,
        tolerance,
    )
}

fn chamfer_connected_curves(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    distances: [Real; 2],
    tolerance: Tolerance,
) -> Result<(PolyCurve3, usize, bool), GeometryError> {
    validate_distances(distances[0], distances[1])?;
    let first = oriented(first, first_pick, true, tolerance)?;
    let second = oriented(second, second_pick, false, tolerance)?;
    let first_sampler = ArcLengthSampler::try_new(CurveRef::PolyCurve(&first), tolerance)?;
    let second_sampler = ArcLengthSampler::try_new(CurveRef::PolyCurve(&second), tolerance)?;
    if distances[0] >= first_sampler.total_length() - tolerance.absolute()
        || distances[1] >= second_sampler.total_length() - tolerance.absolute()
    {
        return Err(unsupported());
    }
    let first_parameter = if distances[0] == 0.0 {
        *first.domain().end()
    } else {
        first_sampler.parameter_at_distance(first_sampler.total_length() - distances[0])?
    };
    let second_parameter = if distances[1] == 0.0 {
        *second.domain().start()
    } else {
        second_sampler.parameter_at_distance(distances[1])?
    };
    let first_retained = first.try_trimmed(*first.domain().start()..=first_parameter)?;
    let second_retained = second.try_trimmed(second_parameter..=*second.domain().end())?;
    let first_cut = first_retained.evaluate(*first_retained.domain().end())?;
    let second_cut = second_retained.evaluate(*second_retained.domain().start())?;
    let first_count = first_retained.segments().len();
    let has_bevel = first_cut.distance_to(second_cut)? > tolerance.absolute();
    let mut segments = first_retained.segments().to_vec();
    if has_bevel {
        segments.push(CurveSegment3::Line(LineSegment::try_new(
            first_cut,
            second_cut,
            Tolerance::NUMERICAL_VALIDATION,
        )?));
    }
    segments.extend_from_slice(second_retained.segments());
    Ok((PolyCurve3::try_new(segments)?, first_count, has_bevel))
}

fn chamfer_parts_from_joined(
    joined: &PolyCurve3,
    first_count: usize,
    has_bevel: bool,
    trim: bool,
    reverse_first: bool,
    reverse_second: bool,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    if !trim && !has_bevel {
        return Err(unsupported());
    }
    let segments = joined.segments();
    if !trim {
        return Ok(vec![segments[first_count].clone().into_curve()]);
    }
    let second_start = first_count + usize::from(has_bevel);
    let mut parts = vec![
        original_direction(
            curve_from_segments(&segments[..first_count])?,
            reverse_first,
            tolerance,
        )?,
        original_direction(
            curve_from_segments(&segments[second_start..])?,
            reverse_second,
            tolerance,
        )?,
    ];
    if has_bevel {
        parts.push(segments[first_count].clone().into_curve());
    }
    Ok(parts)
}

/// Trims or extends terminal straight segments to a bevel whose setbacks are
/// measured along the two supporting lines from their intersection.
pub fn try_chamfer_curves_joined(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    first_distance: Real,
    second_distance: Real,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    validate_distances(first_distance, second_distance)?;
    let first = oriented(first, first_pick, true, tolerance)?;
    let second = oriented(second, second_pick, false, tolerance)?;
    let last = first.segments().len() - 1;
    let first_terminal = &first.segments()[last];
    let second_terminal = &second.segments()[0];
    let (first_retained, second_retained, first_cut, second_cut) =
        if let (CurveSegment3::Line(first_line), CurveSegment3::Line(second_line)) =
            (first_terminal, second_terminal)
        {
            let (first_line, second_line) = meeting_lines(*first_line, *second_line, tolerance)?;
            let corner = first_line.end();
            let first_direction = first_line.direction(tolerance)?.as_vector();
            let second_direction = second_line.direction(tolerance)?.as_vector();
            let first_cut = corner.translated(first_direction.scaled(-first_distance)?)?;
            let second_cut = corner.translated(second_direction.scaled(second_distance)?)?;
            if first_distance >= first_line.length()? - tolerance.absolute()
                || second_distance >= second_line.length()? - tolerance.absolute()
            {
                return Err(unsupported());
            }
            (
                CurveSegment3::Line(LineSegment::try_new(
                    first_line.start(),
                    first_cut,
                    Tolerance::NUMERICAL_VALIDATION,
                )?),
                CurveSegment3::Line(LineSegment::try_new(
                    second_cut,
                    second_line.end(),
                    Tolerance::NUMERICAL_VALIDATION,
                )?),
                first_cut,
                second_cut,
            )
        } else {
            let end = first_terminal.evaluate(*first_terminal.domain().end())?;
            let start = second_terminal.evaluate(*second_terminal.domain().start())?;
            if end.distance_to(start)? > tolerance.absolute() {
                return Err(unsupported());
            }
            let first_sampler = ArcLengthSampler::try_new(first_terminal.as_ref(), tolerance)?;
            let second_sampler = ArcLengthSampler::try_new(second_terminal.as_ref(), tolerance)?;
            if first_distance >= first_sampler.total_length() - tolerance.absolute()
                || second_distance >= second_sampler.total_length() - tolerance.absolute()
            {
                return Err(unsupported());
            }
            let first_parameter = first_sampler
                .parameter_at_distance(first_sampler.total_length() - first_distance)?;
            let second_parameter = second_sampler.parameter_at_distance(second_distance)?;
            let first_retained =
                first_terminal.try_trimmed(*first_terminal.domain().start()..=first_parameter)?;
            let second_retained =
                second_terminal.try_trimmed(second_parameter..=*second_terminal.domain().end())?;
            let first_cut = first_retained.evaluate(*first_retained.domain().end())?;
            let second_cut = second_retained.evaluate(*second_retained.domain().start())?;
            (first_retained, second_retained, first_cut, second_cut)
        };
    let mut segments = first.segments()[..last].to_vec();
    segments.push(first_retained);
    if first_cut.distance_to(second_cut)? > tolerance.absolute() {
        segments.push(CurveSegment3::Line(LineSegment::try_new(
            first_cut,
            second_cut,
            Tolerance::NUMERICAL_VALIDATION,
        )?));
    }
    segments.push(second_retained);
    segments.extend_from_slice(&second.segments()[1..]);
    PolyCurve3::try_new(segments)
}

/// Returns retained source curves in their original directions, followed by
/// the bevel. With trimming off, only the bevel is returned.
pub fn try_chamfer_curves_parts(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    distances: [Real; 2],
    trim: bool,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    if !trim && distances == [0.0, 0.0] {
        return Err(unsupported());
    }
    let reverse_first = !selected_end(first, first_pick, tolerance)?;
    let reverse_second = selected_end(second, second_pick, tolerance)?;
    let first_count = first.to_polycurve()?.segments().len();
    let joined = try_chamfer_curves_joined(
        first,
        first_pick,
        second,
        second_pick,
        distances[0],
        distances[1],
        tolerance,
    )?;
    let second_count = second.to_polycurve()?.segments().len();
    let has_bevel = joined.segments().len() > first_count + second_count;
    chamfer_parts_from_joined(
        &joined,
        first_count,
        has_bevel,
        trim,
        reverse_first,
        reverse_second,
        tolerance,
    )
}

fn validate_distances(first: Real, second: Real) -> Result<(), GeometryError> {
    if !first.is_finite() || !second.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "curve chamfer distance",
        });
    }
    if first < 0.0 || second < 0.0 {
        return Err(GeometryError::Degenerate {
            context: "curve chamfer distance",
        });
    }
    Ok(())
}

fn unsupported() -> GeometryError {
    GeometryError::InvalidPolyCurve {
        context: "selected curve ends cannot form a supported chamfer",
    }
}

#[cfg(test)]
mod tests {
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
    fn meeting_arc_and_line_trim_at_arc_length_setbacks() {
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
            std::f64::consts::PI / 6.,
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
        let source_length = crate::CurveRef::NurbsCurve(&nurbs)
            .length(tolerance)
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
            (crate::CurveRef::NurbsCurve(retained)
                .length(tolerance)
                .unwrap()
                - (source_length - 0.3))
                .abs()
                < 1e-8
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
        assert!((retained.length().unwrap() - (std::f64::consts::PI - 0.2)).abs() < 1e-9);
        assert!(
            bevel
                .start()
                .distance_to(p(-0.2_f64.cos(), 0.2_f64.sin()))
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
        assert!((retained.length().unwrap() - (std::f64::consts::FRAC_PI_2 - 0.2)).abs() < 1e-9);
        assert!(
            bevel
                .start()
                .distance_to(p(0.2_f64.sin(), 0.2_f64.cos()))
                .unwrap()
                < 1e-9
        );
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
    fn smooth_nurbs_extension_is_set_back_by_curve_length() {
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
        let connected_length = connected[0].as_ref().length(Tolerance::DEFAULT).unwrap();
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
        let retained_length = crate::CurveRef::NurbsCurve(retained)
            .length(Tolerance::DEFAULT)
            .unwrap();
        assert!((connected_length - retained_length - 0.2).abs() < 1e-8);
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
        let retained_length = crate::CurveRef::NurbsCurve(retained)
            .length(Tolerance::DEFAULT)
            .unwrap();
        assert!((original_length - retained_length - 0.2).abs() < 1e-8);
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
}
