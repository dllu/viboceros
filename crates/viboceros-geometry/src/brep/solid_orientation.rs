//! Conservative spatial orientation witnesses; never a signed-volume heuristic.
use super::*;
mod planar;
mod rectangle;

#[cfg(test)]
mod tests;

/// Spatial boundary sense, distinct from closed and consistently oriented topology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrepSolidOrientation {
    /// Oriented edge incidence does not define a topological solid.
    NotSolid,
    /// An exact outside-boundary witness points away from its component.
    Outward,
    /// An exact outside-boundary witness points into its component.
    Inward,
    /// No sufficient spatial witness, conflicting tied shells, or exhausted work budget.
    Unknown,
}

const EXACT_WORK_LIMIT: usize = 262_144;

impl Brep {
    /// Classifies spatial sense using exact outside-boundary witnesses.
    ///
    /// Unlike `is_solid`, this query considers embedding; unlike signed volume,
    /// it does not sum or cancel oppositely oriented disconnected shells.
    /// Exactly supported planar polygons use actual trim bounds to select the
    /// minimum-X components, then exact first crossings of +X rays to classify
    /// each component. Affine patches with piecewise-linear trims (including
    /// holes) and convex planar bilinear rectangles are supported. Ambiguous
    /// edge, coplanar, and tied ray hits are not classification evidence.
    ///
    /// Otherwise same-sign surface weights give a control-hull bound. A witness
    /// must attain that bound exactly, lie on an exactly verified rectangular
    /// trim, and have a nonzero natural normal parallel to X. Face sense is then
    /// applied independently. Every shell tied at the bound must yield the same
    /// sense; conflicting or unresolved ties return `Unknown`.
    ///
    /// This is a conservative, incomplete classifier, not validation of a
    /// non-self-intersecting solid. Unsupported trims, unattained hull bounds,
    /// mixed weights, unsupported corner extrema, and work exhaustion can return
    /// `Unknown`. Knot endpoints and mid-span stations propose contacts; exact
    /// rational predicates, not sampling accuracy, authorize accepted results.
    /// No tolerance or volume integration is used, and geometry is unchanged.
    pub fn solid_orientation(&self) -> Result<BrepSolidOrientation, GeometryError> {
        self.solid_orientation_with_budget(EXACT_WORK_LIMIT)
    }

    fn solid_orientation_with_budget(
        &self,
        mut remaining: usize,
    ) -> Result<BrepSolidOrientation, GeometryError> {
        use BrepSolidOrientation::*;
        if !self.is_solid() {
            return Ok(NotSolid);
        }
        if let Some(sense) = planar::classify(self, &mut remaining) {
            return Ok(sense);
        }
        let mut bounds = Vec::with_capacity(self.faces.len());
        for face in &self.faces {
            let controls = face.surface.control_points();
            let sign = controls[0].weight().is_sign_positive();
            if controls
                .iter()
                .any(|c| c.weight().is_sign_positive() != sign)
            {
                return Ok(Unknown);
            }
            bounds.push(
                controls
                    .iter()
                    .map(|c| c.point().x())
                    .fold(Real::INFINITY, Real::min),
            );
        }
        let minimum = bounds.iter().copied().fold(Real::INFINITY, Real::min);
        let mut orientation = None;
        for component in self.edge_connected_face_components() {
            if !component.iter().any(|&f| bounds[f] == minimum) {
                continue;
            }
            let mut witness = None;
            for f in component {
                if bounds[f] != minimum {
                    continue;
                }
                let face = &self.faces[f];
                let controls = face.surface.control_points();
                if (1..3).any(|axis| {
                    controls
                        .iter()
                        .all(|c| c.point().to_array()[axis] == controls[0].point().to_array()[axis])
                }) {
                    // A surface confined to a Y- or Z-normal plane cannot
                    // have a regular tangent plane normal to X. This exact
                    // rejection avoids evaluating other faces of an axis box.
                    continue;
                }
                let Some(rectangle) = rectangle::bounds(face) else {
                    continue;
                };
                let u = stations(face.surface.spans_u(), rectangle[0]);
                let v = stations(face.surface.spans_v(), rectangle[1]);
                let cost = (face.surface.degree_u() + 1)
                    .saturating_mul(face.surface.degree_v() + 1)
                    .saturating_mul(
                        face.surface
                            .degree_u()
                            .saturating_add(face.surface.degree_v())
                            .saturating_add(2),
                    );
                'stations: for &v in &v {
                    for &u in &u {
                        let Some(next) = remaining.checked_sub(cost) else {
                            return Ok(Unknown);
                        };
                        remaining = next;
                        if let Some(inward) =
                            face.surface.minimum_x_support_sense_at(u, v, minimum)?
                        {
                            witness = Some(if inward ^ face.reversed {
                                Inward
                            } else {
                                Outward
                            });
                            break 'stations;
                        }
                    }
                }
                if witness.is_some() {
                    break;
                }
            }
            let Some(witness) = witness else {
                return Ok(Unknown);
            };
            if orientation.is_some_and(|previous| previous != witness) {
                return Ok(Unknown);
            }
            orientation = Some(witness);
        }
        Ok(orientation.unwrap_or(Unknown))
    }
}

fn stations(spans: impl Iterator<Item = (Real, Real)>, [lo, hi]: [Real; 2]) -> Vec<Real> {
    let mut values = vec![lo, hi];
    for (a, b) in spans {
        let (a, b) = (a.max(lo), b.min(hi));
        if a <= b {
            values.extend([a, a * 0.5 + b * 0.5, b]);
        }
    }
    values.sort_by(Real::total_cmp);
    values.dedup();
    // The center avoids natural poles and corners on common primitives.
    let middle = lo * 0.5 + hi * 0.5;
    values.retain(|&t| t != middle);
    values.insert(0, middle);
    values
}
