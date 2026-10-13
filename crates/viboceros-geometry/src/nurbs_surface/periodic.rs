//! Bounded tensor packing around the shared curve seam policy.
use super::*;

impl NurbsSurface {
    pub(super) fn make_smooth_periodic_in_axis<E: From<GeometryError>>(
        &self,
        in_u: bool,
        solve: &mut impl FnMut(&crate::NurbsInterpolationSystem<'_>, usize) -> Result<Vec<Real>, E>,
    ) -> Result<Self, E> {
        // The native interpolation adapter accepts 2,048 homogeneous channels.
        // Chunking larger surfaces preserves that bound without rejecting them.
        const BATCH_CURVES: usize = 512;
        let (degree, count, other_count, source_knots) = if in_u {
            (
                self.degree_u,
                self.control_point_count_u,
                self.control_point_count_v,
                &self.knots_u,
            )
        } else {
            (
                self.degree_v,
                self.control_point_count_v,
                self.control_point_count_u,
                &self.knots_v,
            )
        };
        let mut controls = self.control_points.clone();
        let mut output_knots = None;
        for start in (0..other_count).step_by(BATCH_CURVES) {
            let end = (start + BATCH_CURVES).min(other_count);
            let curves = (start..end)
                .map(|other| {
                    let row = (0..count)
                        .map(|index| {
                            let (u, v) = if in_u { (index, other) } else { (other, index) };
                            self.control_points[self.control_index(u, v)]
                        })
                        .collect();
                    crate::NurbsCurve::try_new_rational(degree, row, source_knots.clone())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mapped = crate::nurbs::periodic::smooth_curves(&curves, solve)?;
            for (other, curve) in (start..end).zip(mapped) {
                if curve.control_points().len() != count
                    || curve.degree() != degree
                    || output_knots
                        .as_ref()
                        .is_some_and(|knots| knots != curve.knots())
                {
                    return Err(GeometryError::InvalidControlNet {
                        context: "periodic control curves produced inconsistent mapped layouts",
                    }
                    .into());
                }
                output_knots.get_or_insert_with(|| curve.knots().to_vec());
                for (index, point) in curve.control_points().iter().enumerate() {
                    let (u, v) = if in_u { (index, other) } else { (other, index) };
                    controls[self.control_index(u, v)] = *point;
                }
            }
        }
        let knots = output_knots.ok_or(GeometryError::InvalidControlNet {
            context: "a surface has no periodic control curves",
        })?;
        Ok(Self::try_new_rational(
            self.degree_u,
            self.degree_v,
            self.control_point_count_u,
            self.control_point_count_v,
            controls,
            if in_u {
                knots.clone()
            } else {
                self.knots_u.clone()
            },
            if in_u { self.knots_v.clone() } else { knots },
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closed_rows(count: usize) -> NurbsSurface {
        let coordinates = [0., 2., 4., 1., 0.];
        let controls = (0..count)
            .flat_map(|v| coordinates.map(|x| Point3::try_new(x, x * x, v as Real).unwrap()))
            .collect();
        let knots_v = [
            vec![0.],
            (0..count).map(|i| i as Real).collect(),
            vec![(count - 1) as Real],
        ]
        .concat();
        NurbsSurface::try_new(
            2,
            1,
            5,
            count,
            controls,
            vec![0., 0., 0., 2., 5., 8., 8., 8.],
            knots_v,
        )
        .unwrap()
    }

    #[test]
    fn periodic_tensor_batches_obey_channel_limit_and_leave_source_immutable() {
        let source = closed_rows(513);
        let before = source.clone();
        let mut columns = Vec::new();
        let output = source
            .try_make_periodic_with_solver(SurfaceKnotDirection::U, true, &mut |input, shift| {
                columns.push(input.columns);
                assert_eq!(input.targets.len(), 3 * input.columns);
                crate::nurbs::portable_periodic_interpolation(input, shift)
            })
            .unwrap();
        assert_eq!(columns, [2048, 4]);
        assert_eq!(source, before);
        assert!(output.is_periodic_u());
        assert_eq!(output.control_point_count_v(), 513);
        assert_eq!(output.knots_v(), source.knots_v());
        for v in 0..513 {
            let original = NurbsCurve::try_new_rational(
                2,
                (0..5)
                    .map(|u| source.control_point(u, v).unwrap())
                    .collect(),
                source.knots_u.clone(),
            )
            .unwrap();
            let expected = original.try_make_periodic(true).unwrap();
            for u in 0..5 {
                let actual = output.control_point(u, v).unwrap();
                assert!(
                    actual
                        .point()
                        .distance_to(expected.control_points()[u].point())
                        .unwrap()
                        < 1e-10
                );
                assert!((actual.weight() - expected.control_points()[u].weight()).abs() < 1e-12);
            }
        }
        let mut calls = 0;
        let failure = GeometryError::InterpolationBackendFailed {
            message: "second batch rejected".into(),
        };
        assert_eq!(
            source.try_make_periodic_with_solver(
                SurfaceKnotDirection::U,
                true,
                &mut |input, shift| {
                    calls += 1;
                    if calls == 2 {
                        return Err(failure.clone());
                    }
                    crate::nurbs::portable_periodic_interpolation(input, shift)
                }
            ),
            Err(failure.clone())
        );
        assert_eq!(calls, 2);
        assert_eq!(source, before);
        assert_eq!(
            source.try_make_periodic_with_solver(SurfaceKnotDirection::U, true, &mut |_, _| Ok::<
                _,
                GeometryError,
            >(
                vec![]
            )),
            Err(GeometryError::PeriodicInterpolationSolveFailed)
        );
    }

    #[test]
    fn both_periodic_directions_share_one_solve_each_and_noops_skip_the_solver() {
        let coordinates = [0., 2., 4., 1., 0.];
        let controls = (0..5)
            .flat_map(|v| {
                coordinates
                    .map(|u| Point3::try_new(u, coordinates[v], 0.1 * u * coordinates[v]).unwrap())
            })
            .collect();
        let source = NurbsSurface::try_new(
            2,
            2,
            5,
            5,
            controls,
            vec![0., 0., 0., 2., 5., 8., 8., 8.],
            vec![0., 0., 0., 2., 5., 8., 8., 8.],
        )
        .unwrap();
        let mut columns = Vec::new();
        let result = source
            .try_make_periodic_with_solver(SurfaceKnotDirection::Both, true, &mut |input, shift| {
                columns.push(input.columns);
                crate::nurbs::portable_periodic_interpolation(input, shift)
            })
            .unwrap();
        assert_eq!(columns, [20, 20]);
        assert!(result.is_periodic_u() && result.is_periodic_v());
        let mut unexpected = |_: &crate::NurbsInterpolationSystem<'_>, _: usize| {
            panic!("already-periodic and nonsmooth conversion must not solve")
        };
        assert_eq!(
            result
                .try_make_periodic_with_solver::<GeometryError>(
                    SurfaceKnotDirection::Both,
                    true,
                    &mut unexpected
                )
                .unwrap(),
            result
        );
        assert!(
            source
                .try_make_periodic_with_solver::<GeometryError>(
                    SurfaceKnotDirection::Both,
                    false,
                    &mut unexpected
                )
                .unwrap()
                .is_periodic_u()
        );
    }
}
