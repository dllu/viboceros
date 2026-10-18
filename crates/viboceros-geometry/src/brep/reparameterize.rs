//! Covariant face charts: surfaces and p-curves change together.
use super::*;
use crate::parameter::scaled_ratio;
use std::ops::RangeInclusive;

fn coordinate(
    value: Real,
    source: &RangeInclusive<Real>,
    target: &RangeInclusive<Real>,
) -> Result<Real, GeometryError> {
    // P-curve controls may lie outside the face domain even when their locus
    // stays inside. Do not clamp them or change the curve's own knot vector.
    let (old, new) = if (value - source.start()).abs() <= (value - source.end()).abs() {
        (*source.start(), *target.start())
    } else {
        (*source.end(), *target.end())
    };
    let result = new
        + scaled_ratio(
            value - old,
            target.end() - target.start(),
            source.end() - source.start(),
        )?;
    require_finite([result], "face chart coordinate")?;
    Ok(result)
}

impl Brep {
    /// Affinely reparameterize selected face surfaces and their UV trims.
    /// Entries contain a face index and its new increasing U/V domains.
    /// Spatial edges, vertices, incidences, senses and curve parameters stay
    /// unchanged. Duplicate indices, invalid domains and changes that fail the
    /// ordinary continuous B-rep boundary checks are errors. No tolerance is
    /// enlarged to accommodate coordinate rounding in an ill-conditioned chart.
    pub fn try_reparameterized_face_uv(
        &self,
        domains: impl IntoIterator<Item = (usize, [RangeInclusive<Real>; 2])>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut faces = self.faces.clone();
        let mut seen = BTreeSet::new();
        let mut changed = false;
        for (index, target) in domains {
            if !seen.insert(index) {
                return invalid("duplicate face chart index");
            }
            let face = faces
                .get_mut(index)
                .ok_or(GeometryError::InvalidBrepTopology {
                    context: "face chart index",
                })?;
            let source = [face.surface.domain_u(), face.surface.domain_v()];
            if target == source {
                continue;
            }
            let surface = face
                .surface
                .try_reparameterized(target[0].clone(), target[1].clone())?;
            for trim in face.loops.iter_mut().flat_map(|l| &mut l.trims) {
                let controls = trim
                    .curve
                    .control_points()
                    .iter()
                    .map(|p| {
                        WeightedPoint2::try_new(
                            Point2::try_new(
                                coordinate(p.point().x(), &source[0], &target[0])?,
                                coordinate(p.point().y(), &source[1], &target[1])?,
                            )?,
                            p.weight(),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                trim.curve = NurbsCurve2::try_new_rational(
                    trim.curve.degree(),
                    controls,
                    trim.curve.knots().to_vec(),
                )?;
                for axis in 0..2 {
                    trim.tolerance[axis] = scaled_ratio(
                        trim.tolerance[axis],
                        target[axis].end() - target[axis].start(),
                        source[axis].end() - source[axis].start(),
                    )?;
                }
            }
            face.surface = surface;
            changed = true;
        }
        if changed {
            Self::try_new(self.vertices.clone(), self.edges.clone(), faces, tolerance)
        } else {
            Ok(self.clone())
        }
    }
}

#[cfg(test)]
mod tests;
