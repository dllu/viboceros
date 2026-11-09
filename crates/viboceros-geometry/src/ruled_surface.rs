//! Ordered projective ruled tensors, independent of Loft region/weight policy.
use crate::{GeometryError, NurbsCurve, NurbsSurface, Real, section_basis};

const MAX_CONTROLS: usize = 4096;

impl NurbsSurface {
    /// Match boundary degrees and normalized knot vectors, then interleave
    /// their homogeneous controls. U crosses between the supplied boundaries;
    /// V follows their original traversal. Both domains are [0,1]. No sorting,
    /// seam relocation or endpoint-weight normalization is performed.
    ///
    /// Closed and constant boundaries are accepted. Each original weight gauge
    /// is retained: the transverse clock is projective when weights differ.
    /// Poles remain possible for mixed/opposite weight signs and are reported
    /// by evaluators. This does not trim crossings or construct B-rep topology.
    pub fn try_ruled_from_curves(a: &NurbsCurve, b: &NurbsCurve) -> Result<Self, GeometryError> {
        Self::try_ruled_from_curves_with_operations(
            a,
            b,
            &mut |c| c.clamped_to_active_domain(),
            &mut |c, p| c.try_change_degree(p, false),
            &mut |c, t, m| c.try_insert_knot(t, m),
        )
    }

    /// Delegate structural edits while retaining the ordered basis, gauges and
    /// tensor layout. At most 4096 controls per boundary are accepted.
    pub fn try_ruled_from_curves_with_operations<E: From<GeometryError>>(
        a: &NurbsCurve,
        b: &NurbsCurve,
        clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
        elevate: &mut impl FnMut(&NurbsCurve, usize) -> Result<NurbsCurve, E>,
        refine: &mut impl FnMut(&NurbsCurve, Real, usize) -> Result<NurbsCurve, E>,
    ) -> Result<Self, E> {
        let sections = section_basis::prepare_with_operations(
            &[a.clone(), b.clone()],
            MAX_CONTROLS,
            GeometryError::RuledSurfaceResourceLimit {
                maximum: MAX_CONTROLS,
            },
            section_basis::WeightScale::Preserve,
            clamp,
            elevate,
            refine,
        )?;
        let first = &sections[0];
        let second = &sections[1];
        Ok(Self::try_new_rational(
            1,
            first.degree(),
            2,
            first.control_points().len(),
            first
                .control_points()
                .iter()
                .zip(second.control_points())
                .flat_map(|(a, b)| [*a, *b])
                .collect(),
            vec![0., 0., 1., 1.],
            first.knots().to_vec(),
        )?)
    }
}

#[cfg(test)]
mod tests;
