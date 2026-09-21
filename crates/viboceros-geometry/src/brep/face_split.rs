//! In-place face partitioning at exact, continuous tensor knot lines.
use super::join_edges::certificate;
use super::*;
use crate::SurfaceKnotDirection;

mod curves;
mod rings;
mod singular;
mod surface;
#[cfg(test)]
mod tests;

const MAX_WORK: usize = 16_000_000;
const MAX_PARTS: usize = 100_000;

struct Budget(usize);
impl Budget {
    fn charge(&mut self, n: usize) -> Result<(), GeometryError> {
        self.0 = self
            .0
            .checked_sub(n)
            .ok_or(GeometryError::InvalidBrepTopology {
                context: "face partition exceeds its work budget",
            })?;
        Ok(())
    }
}

impl Brep {
    /// Partitions a face at a continuous interior knot of full degree
    /// multiplicity. The tensor patches are exact control-table slices, not
    /// fitted surfaces. Shared boundary edges are split in every incident face.
    /// All source vertices, surviving edge slots, face senses, and uncertainty
    /// are retained; new edges, vertices and additional faces are appended.
    ///
    /// Root searches and edge correspondence only propose subdivisions. Every
    /// spatial and UV partition must certify the original locus exactly: either
    /// zero-displacement restrictions or ordered, gap-free collinear segments.
    /// Singular UV trims are subdivided with the same certificates, retaining
    /// their original pole vertex without creating spatial boundary edges.
    /// Whole control hulls certify which side of the cut a trim occupies.
    /// Unrepresentable exact splits, ambiguous/touching cut incidence,
    /// mixed-sign trim hulls, and collapsed interior seams are errors, never
    /// silently approximated. General certificates support degree 16.
    ///
    /// Returns `None` when the knot line does not divide the trimmed region.
    /// The source is unchanged on every outcome. `Both` is not a direction;
    /// partition one knot line at a time. Work is bounded, and ordinary B-rep
    /// validation also applies, including its existing containment limitations.
    pub fn try_split_face_at_knot(
        &self,
        face: usize,
        direction: SurfaceKnotDirection,
        parameter: Real,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        self.split_face_at_knot_with_budget(
            face,
            direction,
            parameter,
            tolerance,
            &mut Budget(MAX_WORK),
        )
    }

    /// Splits qualifying fully-multiple surface knots without rebuilding
    /// unrelated faces or discarding existing boundary topology. The angular
    /// candidates use [`NurbsSurface::sampled_kink_parameters`], not a continuous
    /// maximum-angle certificate. Every accepted partition still meets
    /// [`Self::try_split_face_at_knot`]'s exact-locus guarantees and limits.
    /// All partitions share a face-partition work budget; the edge subdivision
    /// helper additionally enforces its own per-call budget. Returns `None` if
    /// nothing splits.
    pub fn try_split_kinky_faces(
        &self,
        angle: Real,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        if !angle.is_finite() || !(0.0..=std::f64::consts::PI).contains(&angle) {
            return invalid("face kink angle must be in [0, pi]");
        }
        let mut budget = Budget(MAX_WORK);
        let mut result: Option<Self> = None;
        let mut index = 0;
        loop {
            let current = result.as_ref().unwrap_or(self);
            let Some(face) = current.faces.get(index) else {
                break;
            };
            charge_kink_search(&face.surface, &mut budget)?;
            let candidates = face.surface.sampled_kink_parameters(angle)?;
            let mut partition = None;
            for (cuts, direction) in candidates
                .into_iter()
                .zip([SurfaceKnotDirection::U, SurfaceKnotDirection::V])
            {
                for cut in cuts {
                    if let Some(next) = current.split_face_at_knot_with_budget(
                        index,
                        direction,
                        cut,
                        tolerance,
                        &mut budget,
                    )? {
                        partition = Some(next);
                        break;
                    }
                }
                if partition.is_some() {
                    break;
                }
            }
            if let Some(next) = partition {
                result = Some(next);
            } else {
                index += 1;
            }
        }
        Ok(result)
    }

    fn split_face_at_knot_with_budget(
        &self,
        face: usize,
        direction: SurfaceKnotDirection,
        parameter: Real,
        tolerance: Tolerance,
        budget: &mut Budget,
    ) -> Result<Option<Self>, GeometryError> {
        let axis = match direction {
            SurfaceKnotDirection::U => 0,
            SurfaceKnotDirection::V => 1,
            SurfaceKnotDirection::Both => return invalid("face partition needs one direction"),
        };
        let source = self
            .faces
            .get(face)
            .ok_or(GeometryError::InvalidBrepTopology {
                context: "face partition references a missing face",
            })?;
        if !self.is_manifold() {
            return invalid("face partition requires manifold input");
        }
        // Bound full-table staging before any clone or new geometry allocation.
        budget.charge(
            self.vertices
                .len()
                .saturating_add(self.edges.len())
                .saturating_add(self.faces.len()),
        )?;
        for edge in &self.edges {
            budget.charge(
                edge.curve
                    .control_points()
                    .len()
                    .saturating_add(edge.curve.knots().len()),
            )?;
        }
        for face in &self.faces {
            budget.charge(face.surface.control_points().len())?;
            for trim in face.loops.iter().flat_map(|l| &l.trims) {
                budget.charge(
                    trim.curve
                        .control_points()
                        .len()
                        .saturating_add(trim.curve.knots().len()),
                )?;
            }
        }
        let patches = surface::split(&source.surface, axis, parameter, budget)?;
        let splits = curves::crossings(self, face, axis, parameter, tolerance, budget)?;
        let mut result = self.try_split_edges_at_parameters(&splits, tolerance)?;
        curves::certify(self, &mut result, &splits, budget)?;
        singular::split(&mut result.faces[face], axis, parameter, budget)?;
        let Some(partitions) = rings::partition(
            &mut result,
            face,
            axis,
            parameter,
            patches,
            tolerance,
            budget,
        )?
        else {
            return Ok(None);
        };
        let mut partitions = partitions.into_iter();
        result.faces[face] = partitions.next().expect("two nonempty sides");
        result.faces.extend(partitions);
        // A seam's two old uses can now belong to different faces. Never
        // preserve a stale seam/boundary tag after changing face ownership.
        let mut uses = vec![Vec::new(); result.edges.len()];
        for (f, face) in result.faces.iter().enumerate() {
            for trim in face.loops.iter().flat_map(|l| &l.trims) {
                if let Some(e) = trim.edge {
                    uses[e].push(f);
                }
            }
        }
        for face in &mut result.faces {
            for trim in face.loops.iter_mut().flat_map(|l| &mut l.trims) {
                if let Some(e) = trim.edge {
                    trim.trim_type = match uses[e].as_slice() {
                        [_] => BrepTrimType::Boundary,
                        [a, b] if a == b => BrepTrimType::Seam,
                        [_, _] => BrepTrimType::Mated,
                        _ => return invalid("face partition produced nonmanifold incidence"),
                    };
                }
            }
        }
        result.validate(tolerance)?;
        Ok(Some(result))
    }
}

fn charge_kink_search(surface: &NurbsSurface, budget: &mut Budget) -> Result<(), GeometryError> {
    let mut probes = 0usize;
    for (knots, degree, domain, transverse_count) in [
        (
            surface.knots_u(),
            surface.degree_u(),
            surface.domain_u(),
            surface.control_point_count_v(),
        ),
        (
            surface.knots_v(),
            surface.degree_v(),
            surface.domain_v(),
            surface.control_point_count_u(),
        ),
    ] {
        budget.charge(knots.len())?;
        let candidates = knots
            .chunk_by(|a, b| a == b)
            .filter(|group| {
                group.len() >= degree && group[0] > *domain.start() && group[0] < *domain.end()
            })
            .count();
        // The current sampler builds an isocurve for every candidate knot at
        // each transverse span endpoint/midpoint. Charging only the tensor net
        // would miss this multiplicative cost on large piecewise-linear grids.
        probes = probes.saturating_add(
            candidates.saturating_mul(transverse_count.saturating_mul(2).saturating_add(1)),
        );
    }
    budget.charge(
        probes
            .saturating_mul(surface.control_points().len())
            .saturating_mul(
                surface
                    .degree_u()
                    .max(surface.degree_v())
                    .saturating_add(1)
                    .saturating_pow(3),
            ),
    )
}
