//! Finite intersections of parallel, offset cone walls with unequal slopes.
//!
//! At each common height the walls are two circles. Their three triangle
//! inequalities are affine in height, so their intersection is one interval.
//! A cosine height parameter removes square-root singularities where the
//! circles touch. The remaining smooth branches use bounded-error cubic fits.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const PI: Real = std::f64::consts::PI;
const MAX_SEGMENTS: usize = 4096;
const BINOMIAL: [[Real; 5]; 5] = [
    [1.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 1.0, 0.0, 0.0, 0.0],
    [1.0, 2.0, 1.0, 0.0, 0.0],
    [1.0, 3.0, 3.0, 1.0, 0.0],
    [1.0, 4.0, 6.0, 4.0, 1.0],
];

#[derive(Clone, Copy)]
struct Affine {
    slope: Real,
    intercept: Real,
}

impl Affine {
    fn at(self, axial: Real) -> Real {
        self.slope.mul_add(axial, self.intercept)
    }
}

struct Section {
    frame: Frame3,
    direction: [Real; 2],
    axial_sign: Real,
    offset: Real,
    first_slope: Real,
    second_radius: Affine,
    low: Real,
    high: Real,
    middle: Real,
    radius: Real,
    lower_count: i32,
    upper_count: i32,
    transverse_scale: Real,
    remaining: Vec<Affine>,
}

pub(super) fn intersect(
    (first_frame, first_radius, first_signed_height): (Frame3, Real, Real),
    (second_frame, second_radius, second_signed_height): (Frame3, Real, Real),
    (offset_x, offset_y, offset_z): (Real, Real, Real),
    tolerance: Tolerance,
    spatial_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let first_height = first_signed_height.abs();
    let second_height = second_signed_height.abs();
    let first_slope = first_radius / first_height;
    let second_slope = second_radius / second_height;
    let first_sign = first_signed_height.signum();
    let second_direction = first_frame
        .z_axis()
        .as_vector()
        .dot(second_frame.z_axis().as_vector())?
        * first_sign
        * second_signed_height.signum();
    let second_sign: Real = if second_direction < 0.0 { -1.0 } else { 1.0 };
    let second_apex = first_sign * offset_z;
    let second_base = second_sign.mul_add(second_height, second_apex);
    let mut low = second_apex.min(second_base).max(0.0);
    let mut high = second_apex.max(second_base).min(first_height);
    if high < low {
        return Ok(Vec::new());
    }
    let offset = offset_x.hypot(offset_y);
    let second_radius = Affine {
        slope: second_sign * second_slope,
        intercept: -second_sign * second_slope * second_apex,
    };
    // y² = Π (r1+r2+d)(r1+r2-d)(d+r1-r2)(d-r1+r2) / (4d²).
    let factors = [
        Affine {
            slope: first_slope + second_radius.slope,
            intercept: second_radius.intercept + offset,
        },
        Affine {
            slope: first_slope + second_radius.slope,
            intercept: second_radius.intercept - offset,
        },
        Affine {
            slope: first_slope - second_radius.slope,
            intercept: offset - second_radius.intercept,
        },
        Affine {
            slope: second_radius.slope - first_slope,
            intercept: offset + second_radius.intercept,
        },
    ];
    for factor in factors.iter().skip(1) {
        if factor.slope > 0.0 {
            low = low.max(-factor.intercept / factor.slope);
        } else if factor.slope < 0.0 {
            high = high.min(-factor.intercept / factor.slope);
        } else if factor.intercept < 0.0 {
            return Ok(Vec::new());
        }
    }
    if high < low {
        return Ok(Vec::new());
    }
    let direction = [offset_x / offset, offset_y / offset];
    let roundoff = 128.0
        * Real::EPSILON
        * first_height
            .max(second_height)
            .max(second_apex.abs())
            .max(offset);
    if high - low <= roundoff {
        return isolated_contacts(
            first_frame,
            direction,
            first_sign,
            offset,
            first_slope,
            second_radius,
            0.5 * (low + high),
            spatial_tolerance,
        );
    }
    let middle = 0.5 * (low + high);
    let radius = 0.5 * (high - low);
    let mut lower_count = 0;
    let mut upper_count = 0;
    let mut transverse_scale = 1.0 / (2.0 * offset);
    let mut remaining = Vec::with_capacity(4);
    for factor in factors {
        let scale = factor
            .slope
            .abs()
            .mul_add(low.abs().max(high.abs()), factor.intercept.abs())
            .max(offset);
        let threshold = 128.0 * Real::EPSILON * scale;
        if factor.slope > 0.0 && factor.at(low).abs() <= threshold {
            lower_count += 1;
            transverse_scale *= (2.0 * radius * factor.slope).sqrt();
        } else if factor.slope < 0.0 && factor.at(high).abs() <= threshold {
            upper_count += 1;
            transverse_scale *= (-2.0 * radius * factor.slope).sqrt();
        } else {
            remaining.push(factor);
        }
    }
    let section = Section {
        frame: first_frame,
        direction,
        axial_sign: first_sign,
        offset,
        first_slope,
        second_radius,
        low,
        high,
        middle,
        radius,
        lower_count,
        upper_count,
        transverse_scale,
        remaining,
    };
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * first_radius.max(second_radius.at(high)).max(first_height))
        .max(spatial_tolerance);
    Ok(vec![
        SurfaceSurfaceIntersectionEvent::Curve(fit_curve(&section, 1.0, fit_tolerance)?),
        SurfaceSurfaceIntersectionEvent::Curve(fit_curve(&section, -1.0, fit_tolerance)?),
    ])
}

impl Section {
    fn sample(&self, parameter: Real, side: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (sine, cosine) = parameter.sin_cos();
        let axial = if parameter == 0.0 {
            self.low
        } else if parameter == PI {
            self.high
        } else {
            self.radius.mul_add(-cosine, self.middle)
        };
        let axial_derivative = self.radius * sine;
        let first_radius = self.first_slope * axial;
        let second_radius = self.second_radius.at(axial);
        let along = ((first_radius - second_radius) * (first_radius + second_radius)
            + self.offset * self.offset)
            / (2.0 * self.offset);
        let along_derivative = (first_radius * self.first_slope
            - second_radius * self.second_radius.slope)
            * axial_derivative
            / self.offset;

        let half_sine = (0.5 * parameter).sin();
        let half_cosine = (0.5 * parameter).cos();
        let h = half_sine.powi(self.lower_count) * half_cosine.powi(self.upper_count);
        let mut h_derivative = 0.0;
        if self.lower_count > 0 {
            h_derivative += 0.5
                * Real::from(self.lower_count)
                * half_sine.powi(self.lower_count - 1)
                * half_cosine.powi(self.upper_count + 1);
        }
        if self.upper_count > 0 {
            h_derivative -= 0.5
                * Real::from(self.upper_count)
                * half_sine.powi(self.lower_count + 1)
                * half_cosine.powi(self.upper_count - 1);
        }
        let mut q = 1.0;
        let mut q_derivative = 0.0;
        for (index, factor) in self.remaining.iter().enumerate() {
            let value = factor.at(axial);
            q *= value;
            let mut product = factor.slope * axial_derivative;
            for (other_index, other) in self.remaining.iter().enumerate() {
                if index != other_index {
                    product *= other.at(axial);
                }
            }
            q_derivative += product;
        }
        if !q.is_finite() || q <= 0.0 {
            return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "parallel cone branch radicand is ill-conditioned",
            });
        }
        let root = q.sqrt();
        let across = side * self.transverse_scale * h * root;
        let across_derivative =
            side * self.transverse_scale * (h_derivative * root + h * q_derivative / (2.0 * root));
        let [ux, uy] = self.direction;
        let local = [
            ux.mul_add(along, -uy * across),
            uy.mul_add(along, ux * across),
            self.axial_sign * axial,
        ];
        let derivative = [
            ux.mul_add(along_derivative, -uy * across_derivative),
            uy.mul_add(along_derivative, ux * across_derivative),
            self.axial_sign * axial_derivative,
        ];
        let x = self.frame.x_axis().as_vector().to_array();
        let y = self.frame.y_axis().as_vector().to_array();
        let z = self.frame.z_axis().as_vector().to_array();
        let tangent = Vector3::try_from(std::array::from_fn(|i| {
            derivative[0].mul_add(x[i], derivative[1].mul_add(y[i], derivative[2] * z[i]))
        }))?;
        Ok((self.frame.point_at(local)?, tangent))
    }

    fn fourth_derivative_bound(&self, start: Real, end: Real) -> Result<Real, GeometryError> {
        let axial_start = self.radius.mul_add(-start.cos(), self.middle);
        let axial_end = self.radius.mul_add(-end.cos(), self.middle);
        let sine_max = if start <= 0.5 * PI && end >= 0.5 * PI {
            1.0
        } else {
            start.sin().max(end.sin())
        };
        let cosine_max = start.cos().abs().max(end.cos().abs());
        let axial_derivatives = [
            0.0,
            self.radius * sine_max,
            self.radius * cosine_max,
            self.radius * sine_max,
            self.radius * cosine_max,
        ];
        let mut q_bounds = [1.0, 0.0, 0.0, 0.0, 0.0];
        let mut q_lower = 1.0;
        for factor in &self.remaining {
            let low = factor.at(axial_start);
            let high = factor.at(axial_end);
            let minimum = low.min(high);
            if !minimum.is_finite() || minimum <= 0.0 {
                return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                    context: "parallel cone branch radicand has no positive lower bound",
                });
            }
            q_lower *= minimum;
            let factor_bounds = [
                low.max(high),
                factor.slope.abs() * axial_derivatives[1],
                factor.slope.abs() * axial_derivatives[2],
                factor.slope.abs() * axial_derivatives[3],
                factor.slope.abs() * axial_derivatives[4],
            ];
            let previous = q_bounds;
            for order in 0..=4 {
                q_bounds[order] = (0..=order)
                    .map(|part| {
                        BINOMIAL[order][part] * previous[part] * factor_bounds[order - part]
                    })
                    .sum();
            }
        }
        let root = q_lower.sqrt();
        let lower3 = q_lower * root;
        let lower5 = q_lower * lower3;
        let lower7 = q_lower * lower5;
        let [_, q1, q2, q3, q4] = q_bounds;
        let g = [
            q_bounds[0].sqrt(),
            q1 / (2.0 * root),
            q2 / (2.0 * root) + q1 * q1 / (4.0 * lower3),
            q3 / (2.0 * root) + 3.0 * q1 * q2 / (4.0 * lower3) + 3.0 * q1.powi(3) / (8.0 * lower5),
            q4 / (2.0 * root)
                + (4.0 * q1 * q3 + 3.0 * q2 * q2) / (4.0 * lower3)
                + 9.0 * q1 * q1 * q2 / (4.0 * lower5)
                + 15.0 * q1.powi(4) / (16.0 * lower7),
        ];
        let harmonics = 0.5 * Real::from(self.lower_count + self.upper_count);
        let transverse_fourth = self.transverse_scale.abs()
            * (0..=4)
                .map(|part| BINOMIAL[4][part] * harmonics.powi(part as i32) * g[4 - part])
                .sum::<Real>();
        let quadratic = (self.first_slope * self.first_slope
            - self.second_radius.slope * self.second_radius.slope)
            / (2.0 * self.offset);
        let linear = -self.second_radius.slope * self.second_radius.intercept / self.offset;
        let first_harmonic = self.radius * (2.0 * quadratic * self.middle + linear);
        let second_harmonic = 0.5 * quadratic * self.radius * self.radius;
        let along_fourth = first_harmonic.abs() + 16.0 * second_harmonic.abs();
        Ok(along_fourth.hypot(transverse_fourth).hypot(self.radius))
    }
}

#[allow(clippy::too_many_arguments)]
fn isolated_contacts(
    frame: Frame3,
    direction: [Real; 2],
    axial_sign: Real,
    offset: Real,
    first_slope: Real,
    second_radius: Affine,
    axial: Real,
    tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let first = first_slope * axial;
    let second = second_radius.at(axial);
    if first <= tolerance || second <= tolerance {
        return Ok(Vec::new());
    }
    let along = ((first - second) * (first + second) + offset * offset) / (2.0 * offset);
    let across_squared = (first - along) * (first + along);
    if across_squared < -tolerance * first.max(second) {
        return Ok(Vec::new());
    }
    let across = across_squared.max(0.0).sqrt();
    let sides: &[Real] = if across <= tolerance {
        &[1.0]
    } else {
        &[1.0, -1.0]
    };
    sides
        .iter()
        .map(|side| {
            Ok(SurfaceSurfaceIntersectionEvent::Point(frame.point_at([
                direction[0].mul_add(along, -side * direction[1] * across),
                direction[1].mul_add(along, side * direction[0] * across),
                axial_sign * axial,
            ])?))
        })
        .collect()
}

fn fit_curve(
    section: &Section,
    side: Real,
    fit_tolerance: Real,
) -> Result<NurbsCurve, GeometryError> {
    let mut pending = vec![(0.0, PI)];
    let mut breaks = vec![0.0];
    while let Some((start, end)) = pending.pop() {
        let derivative_bound = section.fourth_derivative_bound(start, end)?;
        let span = end - start;
        if derivative_bound.is_finite() && derivative_bound * span.powi(4) / 384.0 <= fit_tolerance
        {
            breaks.push(end);
        } else {
            if breaks.len() + pending.len() >= MAX_SEGMENTS || span <= 64.0 * Real::EPSILON {
                return Err(GeometryError::TooManyCurveFitControlPoints {
                    maximum: 3 * MAX_SEGMENTS + 1,
                });
            }
            let middle = 0.5 * (start + end);
            pending.push((middle, end));
            pending.push((start, middle));
        }
    }
    let segments = breaks.len() - 1;
    let mut controls = Vec::with_capacity(3 * segments + 1);
    let mut knots = Vec::with_capacity(3 * segments + 5);
    knots.extend([0.0; 4]);
    let (first_point, mut previous_tangent) = section.sample(0.0, side)?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_parameter = 0.0;
    for &parameter in breaks.iter().skip(1) {
        let (point, tangent) = section.sample(parameter, side)?;
        let handle = (parameter - previous_parameter) / 3.0;
        controls.push(previous_point.translated(previous_tangent.scaled(handle)?)?);
        controls.push(point.translated(tangent.scaled(-handle)?)?);
        controls.push(point);
        knots.extend([parameter; 3]);
        previous_point = point;
        previous_tangent = tangent;
        previous_parameter = parameter;
    }
    knots.push(PI);
    NurbsCurve::try_new(3, controls, knots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsSurface, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame(origin: Point3, normal_z: Real) -> Frame3 {
        Frame3::try_from_normal(
            origin,
            Vector3::try_new(0.0, 0.0, normal_z).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    fn cone(frame: Frame3, radius: Real, height: Real) -> NurbsSurface {
        NurbsSurface::try_cone(frame, radius, height).unwrap()
    }

    fn assert_on_walls(
        location: Point3,
        second_apex: Point3,
        second_sign: Real,
        second_height: Real,
    ) {
        assert!((location.x().hypot(location.y()) - 0.75 * location.z()).abs() < 5e-8);
        let relative = second_apex.vector_to(location).unwrap().to_array();
        assert!(((relative[0].hypot(relative[1])) - 0.5 * second_sign * relative[2]).abs() < 5e-8);
        assert!((-5e-8..=4.0 + 5e-8).contains(&location.z()));
        assert!((-5e-8..=second_height + 5e-8).contains(&(second_sign * relative[2])));
    }

    #[test]
    fn unequal_parallel_cones_make_two_bounded_branches_with_internal_turns() {
        let first = cone(frame(point(0.0, 0.0, 0.0), 1.0), 3.0, 4.0);
        let apex = point(1.0, 0.0, 4.0);
        let second = cone(frame(apex, -1.0), 2.0, 4.0);
        for (left, right) in [(&first, &second), (&second, &first)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            let [
                SurfaceSurfaceIntersectionEvent::Curve(upper),
                SurfaceSurfaceIntersectionEvent::Curve(lower),
            ] = events.as_slice()
            else {
                panic!("expected two smooth branches, got {events:#?}")
            };
            for curve in [upper, lower] {
                assert_eq!(curve.degree(), 3);
                assert!(!curve.is_closed().unwrap());
                for index in 0..=64 {
                    let domain = curve.domain();
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                    assert_on_walls(curve.evaluate(parameter).unwrap(), apex, -1.0, 4.0);
                }
            }
            for ends in [
                (*upper.domain().start(), *lower.domain().start()),
                (*upper.domain().end(), *lower.domain().end()),
            ] {
                assert!(
                    upper
                        .evaluate(ends.0)
                        .unwrap()
                        .distance_to(lower.evaluate(ends.1).unwrap())
                        .unwrap()
                        < 5e-9
                );
            }
        }
    }

    #[test]
    fn unequal_parallel_cones_clip_at_rims_and_keep_external_tangency() {
        let first = cone(frame(point(0.0, 0.0, 0.0), 1.0), 3.0, 4.0);
        let apex = point(1.0, 0.0, 0.0);
        let second = cone(frame(apex, 1.0), 1.5, 3.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("rim should clip both branches")
            };
            for index in 0..=64 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                assert_on_walls(curve.evaluate(parameter).unwrap(), apex, 1.0, 3.0);
            }
        }
        let tangent = cone(frame(point(5.0, 0.0, 0.0), 1.0), 2.0, 4.0);
        let contact =
            surface_surface_intersection_events(&first, &tangent, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = contact.as_slice() else {
            panic!("base rims should meet in one point, got {contact:#?}")
        };
        assert!(contact.distance_to(point(3.0, 0.0, 4.0)).unwrap() < 5e-9);
        let disjoint = cone(frame(point(10.0, 0.0, 0.0), 1.0), 2.0, 4.0);
        assert!(
            surface_surface_intersection_events(&first, &disjoint, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn unequal_parallel_cone_sections_stay_on_both_walls_across_finite_layouts() {
        let first = cone(frame(point(0.0, 0.0, 0.0), 1.0), 3.0, 4.0);
        for sign in [-1.0, 1.0] {
            for apex_height in [-1.0, 0.0, 1.0, 4.0] {
                for offset in [0.5, 1.0, 2.0, 4.0] {
                    for slope in [0.25, 0.5, 1.25] {
                        let apex = point(offset, 0.0, apex_height);
                        let second = cone(frame(apex, sign), 4.0 * slope, 4.0);
                        let events = surface_surface_intersection_events(
                            &first,
                            &second,
                            Tolerance::DEFAULT,
                        )
                        .unwrap_or_else(|error| {
                            panic!("sign={sign} apex={apex_height} offset={offset} slope={slope}: {error}")
                        });
                        for event in events {
                            let check = |location: Point3| {
                                let second_axial = sign * (location.z() - apex_height);
                                assert!(
                                    (location.x().hypot(location.y()) - 0.75 * location.z()).abs()
                                        < 2e-6
                                );
                                assert!(
                                    ((location.x() - offset).hypot(location.y())
                                        - slope * second_axial)
                                        .abs()
                                        < 2e-6
                                );
                                assert!((-2e-6..=4.0 + 2e-6).contains(&location.z()));
                                assert!((-2e-6..=4.0 + 2e-6).contains(&second_axial));
                            };
                            match event {
                                SurfaceSurfaceIntersectionEvent::Point(location) => check(location),
                                SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                                    for index in 0..=16 {
                                        let domain = curve.domain();
                                        let parameter = *domain.start()
                                            + (*domain.end() - *domain.start())
                                                * (index as Real / 16.0);
                                        check(curve.evaluate(parameter).unwrap());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn unequal_parallel_cones_support_negative_and_distant_rotated_frames() {
        let base = frame(point(0.0, 0.0, 0.0), 1.0);
        let first = cone(base, 3.0, -4.0);
        let second = cone(base.with_origin(point(1.0, 0.0, 0.0)), 2.0, -4.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("negative cone heights should retain both branches")
            };
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert!((location.x().hypot(location.y()) + 0.75 * location.z()).abs() < 5e-8);
                assert!(
                    ((location.x() - 1.0).hypot(location.y()) + 0.5 * location.z()).abs() < 5e-8
                );
            }
        }

        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let other = rotated.with_origin(rotated.point_at([1.0, 0.0, 0.0]).unwrap());
        let first = cone(rotated, 3.0, 4.0);
        let second = cone(other, 2.0, 4.0);
        for (left, right) in [(&first, &second), (&second, &first)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("rotated offset cones should retain both branches")
                };
                for index in 0..=32 {
                    let domain = curve.domain();
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                    let location = curve.evaluate(parameter).unwrap();
                    let first_local = rotated.coordinates_of(location).unwrap();
                    let second_local = other.coordinates_of(location).unwrap();
                    assert!(
                        (first_local[0].hypot(first_local[1]) - 0.75 * first_local[2]).abs() < 4e-7
                    );
                    assert!(
                        (second_local[0].hypot(second_local[1]) - 0.5 * second_local[2]).abs()
                            < 4e-7
                    );
                }
            }
        }
    }
}
