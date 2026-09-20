//! Query-owned, last-span evaluation state. No station values or global caches.
use super::*;
use exact::ExactJetNet;

#[cfg(test)]
mod tests;

enum Prepared<'a> {
    Float(EvaluationControls),
    Exact(ExactJetNet<'a>),
}

pub(in crate::nurbs_surface) struct SurfaceQuery<'a> {
    pub(in crate::nurbs_surface) surface: &'a NurbsSurface,
    active: Option<([usize; 2], Prepared<'a>)>,
}

impl<'a> SurfaceQuery<'a> {
    pub(in crate::nurbs_surface) fn new(surface: &'a NurbsSurface) -> Self {
        Self {
            surface,
            active: None,
        }
    }

    pub(in crate::nurbs_surface) fn evaluate(
        &mut self,
        u: Real,
        v: Real,
    ) -> Result<Point3, GeometryError> {
        Ok(self.jet([u, v], 0)?.point)
    }

    pub(in crate::nurbs_surface) fn evaluate_with_derivatives(
        &mut self,
        u: Real,
        v: Real,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        let jet = self.jet([u, v], 1)?;
        Ok((jet.point, jet.derivative_u, jet.derivative_v))
    }

    pub(in crate::nurbs_surface) fn evaluate_with_second_derivatives(
        &mut self,
        u: Real,
        v: Real,
    ) -> Result<SurfaceJet2, GeometryError> {
        self.jet([u, v], 2)
    }

    fn jet(&mut self, [u, v]: [Real; 2], order: u8) -> Result<SurfaceJet2, GeometryError> {
        let s = self.surface;
        // Strict, right-sided queries share validation precedence with the
        // public evaluator. Invalid stations cannot reuse a previous span.
        let spans = [
            checked_span(s.degree_u, s.control_point_count_u, &s.knots_u, u)?,
            checked_span(s.degree_v, s.control_point_count_v, &s.knots_v, v)?,
        ];
        if self
            .active
            .as_ref()
            .is_none_or(|(previous, _)| *previous != spans)
        {
            let prepared = match s.evaluation_controls(spans) {
                Ok(controls) if !controls.needs_exact => Prepared::Float(controls),
                _ => Prepared::Exact(ExactJetNet::new(s, spans)),
            };
            self.active = Some((spans, prepared));
        }
        match &mut self.active.as_mut().expect("active span initialized").1 {
            Prepared::Exact(net) => net.evaluate([u, v], order),
            Prepared::Float(controls) => s
                .jet_at_spans::<false>([u, v], spans, controls.origin, &controls.homogeneous, order)
                .or_else(|_| s.exact_jet_at_spans([u, v], spans, order)),
        }
    }
}
