//! Tensor-grid point evaluation with the scalar evaluator's operation order.
use super::*;
use crate::nurbs::de_boor_extended_in_place;
use crate::nurbs::exact::{Direction, Homogeneous};

#[cfg(test)]
mod tests;

struct Column {
    span_u: usize,
    contraction: Contraction,
}

enum Contraction {
    Float(FloatColumn),
    Exact(Vec<Homogeneous>),
}

struct FloatColumn {
    origin: Point3,
    homogeneous: Vec<[Real; 4]>,
}

enum Controls {
    Float(EvaluationControls),
    Exact(Vec<Homogeneous>),
}

impl NurbsSurface {
    /// Visits points in V-major/U-minor order, including individual failures.
    /// Only one V span's U contractions are retained; ordering and duplicates
    /// in either input are allowed. This does not cache or modify the surface.
    pub(in crate::nurbs_surface) fn for_each_grid_point(
        &self,
        parameters_u: &[Real],
        parameters_v: &[Real],
        mut visit: impl FnMut(usize, usize, Result<Point3, GeometryError>),
    ) {
        let us = parameters_u
            .iter()
            .map(|&u| (u, ParameterSide::Right))
            .collect::<Vec<_>>();
        let vs = parameters_v
            .iter()
            .map(|&v| (v, ParameterSide::Right))
            .collect::<Vec<_>>();
        self.try_for_each_grid_point_on_sides(&us, &vs, |i, j, p| {
            visit(i, j, p);
            Ok::<(), std::convert::Infallible>(())
        })
        .expect("infallible grid visitor");
    }

    /// Reuse tensor contractions while retaining each station's requested knot
    /// side, scalar evaluation order and recovery. Visitor errors stop at that
    /// exact cell; failures in evaluation remain values passed to the visitor.
    pub(crate) fn try_for_each_grid_point_on_sides<E>(
        &self,
        parameters_u: &[(Real, ParameterSide)],
        parameters_v: &[(Real, ParameterSide)],
        mut visit: impl FnMut(usize, usize, Result<Point3, GeometryError>) -> Result<(), E>,
    ) -> Result<(), E> {
        if parameters_u.is_empty() || parameters_v.is_empty() {
            return Ok(());
        }
        let mut span = None;
        let mut columns: Vec<Option<Column>> = Vec::new();
        let mut work = vec![[0.; 4]; self.degree_v + 1];
        for (j, &(v, side_v)) in parameters_v.iter().enumerate() {
            let next_span = sided_span(
                self.degree_v,
                self.control_point_count_v,
                &self.knots_v,
                v,
                side_v,
            )
            .ok();
            if next_span != span {
                columns =
                    next_span.map_or_else(Vec::new, |span| self.grid_columns(parameters_u, span));
                span = next_span;
            }
            for (i, &(u, side_u)) in parameters_u.iter().enumerate() {
                let cached =
                    columns
                        .get(i)
                        .and_then(Option::as_ref)
                        .zip(span)
                        .map(|(column, span_v)| match &column.contraction {
                            Contraction::Exact(net) => {
                                let direction = Direction {
                                    knots: &self.knots_v,
                                    degree: self.degree_v,
                                    span: span_v,
                                    parameter: v,
                                };
                                direction.evaluate(net.clone()).and_then(|h| {
                                    self.exact_point_from_homogeneous(
                                        [u, v],
                                        [column.span_u, span_v],
                                        &h,
                                    )
                                })
                            }
                            Contraction::Float(contraction) => {
                                work.copy_from_slice(&contraction.homogeneous);
                                de_boor_extended_in_place(
                                    &self.knots_v,
                                    self.degree_v,
                                    span_v,
                                    v,
                                    &mut work,
                                )
                                .and_then(project_homogeneous)
                                .and_then(|local| {
                                    self.restore_point(
                                        [u, v],
                                        [column.span_u, span_v],
                                        contraction.origin,
                                        local,
                                    )
                                })
                                .or_else(|_| self.evaluate_on_sides(u, v, side_u, side_v))
                            }
                        });
                // Invalid cells preserve the scalar path's validation precedence.
                visit(
                    i,
                    j,
                    cached.unwrap_or_else(|| self.evaluate_on_sides(u, v, side_u, side_v)),
                )?;
            }
        }
        Ok(())
    }

    fn grid_columns(
        &self,
        parameters_u: &[(Real, ParameterSide)],
        span_v: usize,
    ) -> Vec<Option<Column>> {
        let mut active: Option<(usize, Controls)> = None;
        let mut work = vec![[0.; 4]; self.degree_u + 1];
        parameters_u
            .iter()
            .map(|&(u, side)| {
                let span_u = sided_span(
                    self.degree_u,
                    self.control_point_count_u,
                    &self.knots_u,
                    u,
                    side,
                )
                .ok()?;
                if active.as_ref().is_none_or(|(span, _)| *span != span_u) {
                    let controls = match self.evaluation_controls([span_u, span_v]) {
                        Ok(controls) if !controls.needs_exact => Controls::Float(controls),
                        _ => Controls::Exact(self.exact_controls([span_u, span_v])),
                    };
                    active = Some((span_u, controls));
                }
                let (_, controls) = active.as_ref()?;
                let contraction = match controls {
                    Controls::Exact(net) => {
                        let direction = Direction {
                            knots: &self.knots_u,
                            degree: self.degree_u,
                            span: span_u,
                            parameter: u,
                        };
                        let column = net
                            .chunks_exact(self.degree_u + 1)
                            .map(|row| direction.evaluate(row.to_vec()))
                            .collect::<Result<Vec<_>, _>>()
                            .ok()?;
                        Contraction::Exact(column)
                    }
                    Controls::Float(controls) => {
                        let mut homogeneous = Vec::with_capacity(self.degree_v + 1);
                        for row in controls.homogeneous.chunks_exact(self.degree_u + 1) {
                            work.copy_from_slice(row);
                            homogeneous.push(
                                de_boor_extended_in_place(
                                    &self.knots_u,
                                    self.degree_u,
                                    span_u,
                                    u,
                                    &mut work,
                                )
                                .ok()?,
                            );
                        }
                        Contraction::Float(FloatColumn {
                            origin: controls.origin,
                            homogeneous,
                        })
                    }
                };
                Some(Column {
                    span_u,
                    contraction,
                })
            })
            .collect()
    }
}

fn sided_span(
    degree: usize,
    count: usize,
    knots: &[Real],
    t: Real,
    side: ParameterSide,
) -> Result<usize, GeometryError> {
    let span = checked_span(degree, count, knots, t)?;
    Ok(
        if side == ParameterSide::Left && t > knots[degree] && t < knots[count] {
            knots.partition_point(|k| *k < t) - 1
        } else {
            span
        },
    )
}
