use super::*;
use crate::{CircularArc3, CurveRef, curve_offset::nurbs_offset_plane};

const SAMPLES_PER_SPAN: usize = 48;
const MAX_SAMPLES: usize = 2_048;

struct NurbsNurbsSolution {
    first: NurbsCurve,
    fillet: CircularArc3,
    second: NurbsCurve,
    removed_length: Real,
}

#[derive(Clone, Copy)]
struct OffsetSample {
    parameter: Real,
    x: Real,
    y: Real,
}

struct Search<'a> {
    first: &'a NurbsCurve,
    second: &'a NurbsCurve,
    radius: Real,
    tolerance: Tolerance,
    normal: Vector3,
    origin: Point3,
    x_axis: Vector3,
    y_axis: Vector3,
}

pub(super) fn resolve_nurbs_nurbs_kinks(
    source: &PolyCurve3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<Option<PolyCurve3>, GeometryError> {
    let closed = source.is_closed()?;
    let mut segments = source.segments().to_vec();
    let mut changed = false;
    let mut index = 0;
    while index + 1 < segments.len() {
        if let Some(solution) =
            fillet_pair(&segments[index], &segments[index + 1], radius, tolerance)?
        {
            segments.splice(
                index..=index + 1,
                [
                    CurveSegment3::NurbsCurve(solution.first),
                    CurveSegment3::Arc(solution.fillet),
                    CurveSegment3::NurbsCurve(solution.second),
                ],
            );
            changed = true;
            index += 2;
        } else {
            index += 1;
        }
    }
    if closed
        && let Some(solution) =
            fillet_pair(segments.last().unwrap(), &segments[0], radius, tolerance)?
    {
        segments[0] = CurveSegment3::NurbsCurve(solution.second);
        *segments.last_mut().unwrap() = CurveSegment3::NurbsCurve(solution.first);
        segments.insert(0, CurveSegment3::Arc(solution.fillet));
        changed = true;
    }
    if changed {
        Ok(Some(PolyCurve3::try_new(segments)?))
    } else {
        Ok(None)
    }
}

fn fillet_pair(
    before: &CurveSegment3,
    after: &CurveSegment3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<Option<NurbsNurbsSolution>, GeometryError> {
    let (CurveSegment3::NurbsCurve(first), CurveSegment3::NurbsCurve(second)) = (before, after)
    else {
        return Ok(None);
    };
    if straight_leaf_vertices(before)?.is_some()
        || straight_leaf_vertices(after)?.is_some()
        || !arc_line::sharp_joint(before, after, tolerance)?
    {
        return Ok(None);
    }
    Ok(Some(solve_nurbs_nurbs(first, second, radius, tolerance)?))
}

fn solve_nurbs_nurbs(
    first: &NurbsCurve,
    second: &NurbsCurve,
    radius: Real,
    tolerance: Tolerance,
) -> Result<NurbsNurbsSolution, GeometryError> {
    let first_tangent = CurveRef::NurbsCurve(first)
        .evaluate_with_tangent_on_side(*first.domain().end(), ParameterSide::Left)?
        .tangent()
        .as_vector();
    let second_tangent = CurveRef::NurbsCurve(second)
        .evaluate_with_tangent_on_side(*second.domain().start(), ParameterSide::Right)?
        .tangent()
        .as_vector();
    let fallback = first_tangent.cross(second_tangent)?.normalized_nonzero()?;
    let first_normal = nurbs_offset_plane(first, fallback, tolerance)?;
    let second_normal = nurbs_offset_plane(second, fallback, tolerance)?;
    if first_normal
        .as_vector()
        .cross(second_normal.as_vector())?
        .length()?
        > tolerance.angular()
    {
        return Err(unsupported_curved_corner());
    }
    let normal = first_normal.as_vector();
    let x_axis = first_tangent.normalized_nonzero()?.as_vector();
    let y_axis = normal.cross(x_axis)?.normalized_nonzero()?.as_vector();
    let search = Search {
        first,
        second,
        radius,
        tolerance,
        normal,
        origin: first.evaluate(*first.domain().end())?,
        x_axis,
        y_axis,
    };
    let mut best: Option<NurbsNurbsSolution> = None;
    for first_side in [-1.0, 1.0] {
        let first_samples = search.samples(first, first_side)?;
        for second_side in [-1.0, 1.0] {
            let second_samples = search.samples(second, second_side)?;
            search.intersections(
                &first_samples,
                &second_samples,
                first_side,
                second_side,
                &mut best,
            )?;
        }
    }
    best.ok_or_else(unsupported_curved_corner)
}

impl Search<'_> {
    fn offset_point(
        &self,
        curve: &NurbsCurve,
        parameter: Real,
        side: Real,
    ) -> Result<(Point3, Vector3, Point3), GeometryError> {
        let sample = CurveRef::NurbsCurve(curve).evaluate_with_tangent_on_side(
            parameter,
            if parameter == *curve.domain().start() {
                ParameterSide::Right
            } else {
                ParameterSide::Left
            },
        )?;
        let tangent = sample.tangent().as_vector();
        let left = self
            .normal
            .cross(tangent)?
            .normalized_nonzero()?
            .as_vector();
        let center = sample
            .point()
            .translated(left.scaled(side * self.radius)?)?;
        Ok((sample.point(), tangent, center))
    }

    fn projected(
        &self,
        curve: &NurbsCurve,
        parameter: Real,
        side: Real,
    ) -> Result<OffsetSample, GeometryError> {
        let (_, _, center) = self.offset_point(curve, parameter, side)?;
        let displacement = self.origin.vector_to(center)?;
        Ok(OffsetSample {
            parameter,
            x: displacement.dot(self.x_axis)?,
            y: displacement.dot(self.y_axis)?,
        })
    }

    fn samples(&self, curve: &NurbsCurve, side: Real) -> Result<Vec<OffsetSample>, GeometryError> {
        let spans = curve.spans().collect::<Vec<_>>();
        if spans.len().saturating_mul(SAMPLES_PER_SPAN) > MAX_SAMPLES {
            return Err(GeometryError::InvalidPolyCurve {
                context: "too many NURBS fillet search intervals",
            });
        }
        let mut samples = Vec::with_capacity(spans.len() * (SAMPLES_PER_SPAN + 1));
        for (start, end) in spans {
            samples.push(self.projected(curve, start, side)?);
            for index in 1..=SAMPLES_PER_SPAN {
                let fraction = index as Real / SAMPLES_PER_SPAN as Real;
                let parameter = (end - start).mul_add(fraction, start);
                samples.push(self.projected(curve, parameter, side)?);
            }
        }
        Ok(samples)
    }

    fn intersections(
        &self,
        first: &[OffsetSample],
        second: &[OffsetSample],
        first_side: Real,
        second_side: Real,
        best: &mut Option<NurbsNurbsSolution>,
    ) -> Result<(), GeometryError> {
        let mut sorted = (0..second.len() - 1).collect::<Vec<_>>();
        sorted.sort_by(|&a, &b| {
            second[a]
                .x
                .min(second[a + 1].x)
                .total_cmp(&second[b].x.min(second[b + 1].x))
        });
        for first_pair in first.windows(2) {
            let min_x = first_pair[0].x.min(first_pair[1].x);
            let max_x = first_pair[0].x.max(first_pair[1].x);
            let min_y = first_pair[0].y.min(first_pair[1].y);
            let max_y = first_pair[0].y.max(first_pair[1].y);
            let limit =
                sorted.partition_point(|&index| second[index].x.min(second[index + 1].x) <= max_x);
            for &index in &sorted[..limit] {
                let second_pair = &second[index..=index + 1];
                if second_pair[0].x.max(second_pair[1].x) < min_x
                    || second_pair[0].y.min(second_pair[1].y) > max_y
                    || second_pair[0].y.max(second_pair[1].y) < min_y
                {
                    continue;
                }
                let Some((first_fraction, second_fraction)) =
                    segment_intersection(first_pair, second_pair)
                else {
                    continue;
                };
                let first_parameter = (first_pair[1].parameter - first_pair[0].parameter)
                    .mul_add(first_fraction, first_pair[0].parameter);
                let second_parameter = (second_pair[1].parameter - second_pair[0].parameter)
                    .mul_add(second_fraction, second_pair[0].parameter);
                if let Some((first_parameter, second_parameter)) =
                    self.refine(first_parameter, second_parameter, first_side, second_side)?
                    && let Some(solution) = self.candidate_at(
                        first_parameter,
                        second_parameter,
                        first_side,
                        second_side,
                    )?
                    && best
                        .as_ref()
                        .is_none_or(|previous| solution.removed_length < previous.removed_length)
                {
                    *best = Some(solution);
                }
            }
        }
        Ok(())
    }

    fn derivative(
        &self,
        curve: &NurbsCurve,
        parameter: Real,
        side: Real,
    ) -> Result<(Real, Real), GeometryError> {
        let (_, velocity, acceleration) = curve.evaluate_with_second_derivative_on_side(
            parameter,
            if parameter == *curve.domain().start() {
                ParameterSide::Right
            } else {
                ParameterSide::Left
            },
        )?;
        let speed = velocity.length()?;
        if speed == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "NURBS fillet stationary point",
            });
        }
        let tangent = velocity.scaled(1.0 / speed)?;
        let tangent_acceleration = tangent.scaled(tangent.dot(acceleration)?)?;
        let normal_acceleration = Vector3::try_new(
            acceleration.x() - tangent_acceleration.x(),
            acceleration.y() - tangent_acceleration.y(),
            acceleration.z() - tangent_acceleration.z(),
        )?;
        let tangent_derivative = normal_acceleration.scaled(1.0 / speed)?;
        let offset_derivative = self
            .normal
            .cross(tangent_derivative)?
            .scaled(side * self.radius)?;
        let derivative = Vector3::try_new(
            velocity.x() + offset_derivative.x(),
            velocity.y() + offset_derivative.y(),
            velocity.z() + offset_derivative.z(),
        )?;
        Ok((derivative.dot(self.x_axis)?, derivative.dot(self.y_axis)?))
    }

    fn refine(
        &self,
        mut first_parameter: Real,
        mut second_parameter: Real,
        first_side: Real,
        second_side: Real,
    ) -> Result<Option<(Real, Real)>, GeometryError> {
        let first_domain = self.first.domain();
        let second_domain = self.second.domain();
        for _ in 0..24 {
            let a = self.projected(self.first, first_parameter, first_side)?;
            let b = self.projected(self.second, second_parameter, second_side)?;
            let dx = a.x - b.x;
            let dy = a.y - b.y;
            let error = dx.hypot(dy);
            let scale = a.x.hypot(a.y).max(b.x.hypot(b.y)).max(1.0);
            let target = (self.tolerance.absolute() * 0.001).max(512.0 * Real::EPSILON * scale);
            if error <= target {
                return Ok(Some((first_parameter, second_parameter)));
            }
            let (ax, ay) = self.derivative(self.first, first_parameter, first_side)?;
            let (bx, by) = self.derivative(self.second, second_parameter, second_side)?;
            let determinant = ax.mul_add(by, -ay * bx);
            if determinant.abs() <= 1e-12 * ax.hypot(ay) * bx.hypot(by) {
                return Ok(None);
            }
            let first_step = -(dx * by - dy * bx) / determinant;
            let second_step = (ax * dy - ay * dx) / determinant;
            let mut fraction = 1.0;
            let mut accepted = false;
            for _ in 0..16 {
                let next_first = first_step.mul_add(fraction, first_parameter);
                let next_second = second_step.mul_add(fraction, second_parameter);
                if next_first > *first_domain.start()
                    && next_first < *first_domain.end()
                    && next_second > *second_domain.start()
                    && next_second < *second_domain.end()
                {
                    let c = self.projected(self.first, next_first, first_side)?;
                    let d = self.projected(self.second, next_second, second_side)?;
                    if (c.x - d.x).hypot(c.y - d.y) < error {
                        first_parameter = next_first;
                        second_parameter = next_second;
                        accepted = true;
                        break;
                    }
                }
                fraction *= 0.5;
            }
            if !accepted {
                return Ok(None);
            }
        }
        Ok(None)
    }

    fn candidate_at(
        &self,
        first_parameter: Real,
        second_parameter: Real,
        first_side: Real,
        second_side: Real,
    ) -> Result<Option<NurbsNurbsSolution>, GeometryError> {
        if !(first_parameter > *self.first.domain().start()
            && first_parameter < *self.first.domain().end()
            && second_parameter > *self.second.domain().start()
            && second_parameter < *self.second.domain().end())
        {
            return Ok(None);
        }
        let (first_point, first_tangent, first_center) =
            self.offset_point(self.first, first_parameter, first_side)?;
        let (second_point, second_tangent, second_center) =
            self.offset_point(self.second, second_parameter, second_side)?;
        if first_center.distance_to(second_center)? > self.tolerance.absolute() * 0.001
            || first_point.distance_to(self.first.evaluate(*self.first.domain().end())?)?
                <= self.tolerance.absolute()
            || second_point.distance_to(self.second.evaluate(*self.second.domain().start())?)?
                <= self.tolerance.absolute()
        {
            return Ok(None);
        }
        let Some(fillet) = arc_line::tangent_fillet_arc(
            first_center,
            first_point,
            second_point,
            first_tangent,
            second_tangent,
            self.radius,
            self.tolerance,
        )?
        else {
            return Ok(None);
        };
        let removed_length = self
            .first
            .try_trimmed(first_parameter..=*self.first.domain().end())?
            .length(self.tolerance)?
            + self
                .second
                .try_trimmed(*self.second.domain().start()..=second_parameter)?
                .length(self.tolerance)?;
        Ok(Some(NurbsNurbsSolution {
            first: self
                .first
                .try_trimmed(*self.first.domain().start()..=first_parameter)?,
            fillet,
            second: self
                .second
                .try_trimmed(second_parameter..=*self.second.domain().end())?,
            removed_length,
        }))
    }
}

fn segment_intersection(first: &[OffsetSample], second: &[OffsetSample]) -> Option<(Real, Real)> {
    let ax = first[1].x - first[0].x;
    let ay = first[1].y - first[0].y;
    let bx = second[1].x - second[0].x;
    let by = second[1].y - second[0].y;
    let determinant = ax.mul_add(by, -ay * bx);
    if determinant.abs() <= 1e-12 * ax.hypot(ay) * bx.hypot(by) {
        return None;
    }
    let dx = second[0].x - first[0].x;
    let dy = second[0].y - first[0].y;
    let first_fraction = (dx * by - dy * bx) / determinant;
    let second_fraction = (dx * ay - dy * ax) / determinant;
    if (0.0..=1.0).contains(&first_fraction) && (0.0..=1.0).contains(&second_fraction) {
        Some((first_fraction, second_fraction))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineSegment;

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }

    #[test]
    fn two_quadratic_nurbs_keep_native_leaves_and_tangent_fillet() {
        let first = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(2., 0.), p(2., 2.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let second = NurbsCurve::try_new(
            2,
            vec![p(2., 2.), p(4., 2.), p(4., 4.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        for segments in [
            vec![
                CurveSegment3::NurbsCurve(first.clone()),
                CurveSegment3::NurbsCurve(second.clone()),
            ],
            vec![
                CurveSegment3::NurbsCurve(second.reversed().unwrap()),
                CurveSegment3::NurbsCurve(first.reversed().unwrap()),
            ],
        ] {
            let source = PolyCurve3::try_new(segments).unwrap();
            let result = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
            assert_eq!(result.segments().len(), 3);
            assert!(matches!(result.segments()[0], CurveSegment3::NurbsCurve(_)));
            let CurveSegment3::Arc(fillet) = result.segments()[1] else {
                panic!("middle leaf is the fillet")
            };
            assert!((fillet.radius() - 0.5).abs() < 1e-8);
            assert!(matches!(result.segments()[2], CurveSegment3::NurbsCurve(_)));
            for pair in result.segments().windows(2) {
                assert!(!arc_line::sharp_joint(&pair[0], &pair[1], Tolerance::DEFAULT).unwrap());
            }
        }

        let closed = PolyCurve3::try_new(vec![
            CurveSegment3::NurbsCurve(second),
            CurveSegment3::Line(
                LineSegment::try_new(p(4., 4.), p(0., 4.), Tolerance::DEFAULT).unwrap(),
            ),
            CurveSegment3::Line(
                LineSegment::try_new(p(0., 4.), p(0., 0.), Tolerance::DEFAULT).unwrap(),
            ),
            CurveSegment3::NurbsCurve(first),
        ])
        .unwrap();
        let rounded = closed.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert!(rounded.is_closed().unwrap());
        assert!(matches!(rounded.segments()[0], CurveSegment3::Arc(_)));
        assert_eq!(rounded.segments().len(), 8);
    }
}
