//! Tensor-grid point evaluation with the scalar evaluator's operation order.
use super::*;
use crate::nurbs::de_boor_extended_in_place;

#[cfg(test)]
mod tests;

struct Column {
    span_u: usize,
    origin: Point3,
    homogeneous: Vec<[Real; 4]>,
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
        if parameters_u.is_empty() || parameters_v.is_empty() {
            return;
        }
        let mut span = None;
        let mut columns: Vec<Option<Column>> = Vec::new();
        let mut work = vec![[0.; 4]; self.degree_v + 1];
        for (j, &v) in parameters_v.iter().enumerate() {
            let next_span =
                checked_span(self.degree_v, self.control_point_count_v, &self.knots_v, v).ok();
            if next_span != span {
                columns =
                    next_span.map_or_else(Vec::new, |span| self.grid_columns(parameters_u, span));
                span = next_span;
            }
            for (i, &u) in parameters_u.iter().enumerate() {
                let cached = columns.get(i).and_then(Option::as_ref).and_then(|column| {
                    let span_v = span?;
                    work.copy_from_slice(&column.homogeneous);
                    let h = de_boor_extended_in_place(
                        &self.knots_v,
                        self.degree_v,
                        span_v,
                        v,
                        &mut work,
                    )
                    .ok()?;
                    let local = project_homogeneous(h).ok()?;
                    self.restore_point([u, v], [column.span_u, span_v], column.origin, local)
                        .ok()
                });
                // Preserve the scalar path's validation precedence and errors,
                // including its world-origin retry for signed-weight overflow.
                visit(i, j, cached.map_or_else(|| self.evaluate(u, v), Ok));
            }
        }
    }

    fn grid_columns(&self, parameters_u: &[Real], span_v: usize) -> Vec<Option<Column>> {
        let mut active: Option<(usize, Point3, Vec<[Real; 4]>)> = None;
        let mut work = vec![[0.; 4]; self.degree_u + 1];
        parameters_u
            .iter()
            .map(|&u| {
                let span_u =
                    checked_span(self.degree_u, self.control_point_count_u, &self.knots_u, u)
                        .ok()?;
                if active.as_ref().is_none_or(|(span, _, _)| *span != span_u) {
                    let (origin, controls) =
                        self.evaluation_controls([span_u, span_v], true).ok()?;
                    active = Some((span_u, origin, controls));
                }
                let (_, origin, controls) = active.as_ref()?;
                let mut homogeneous = Vec::with_capacity(self.degree_v + 1);
                for row in controls.chunks_exact(self.degree_u + 1) {
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
                Some(Column {
                    span_u,
                    origin: *origin,
                    homogeneous,
                })
            })
            .collect()
    }
}
