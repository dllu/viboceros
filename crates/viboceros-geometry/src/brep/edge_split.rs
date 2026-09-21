//! Exact shared-edge subdivision, retaining every incident trim's own parameterization.
use super::*;
mod correspondence;
mod partition;
#[cfg(test)]
mod tests;

const MAX_SPLITS: usize = 100_000;
const MAX_WORK: usize = 4_000_000;

struct Budget(usize);
impl Budget {
    fn charge(&mut self, n: usize) -> Result<(), GeometryError> {
        self.0 = self
            .0
            .checked_sub(n)
            .ok_or(GeometryError::InvalidBrepTopology {
                context: "B-rep edge splitting exceeds its work budget",
            })?;
        Ok(())
    }
}

struct SplitPlan {
    parameters: Vec<Real>,
    edges: Vec<usize>,
    points: Vec<Point3>,
}

impl Brep {
    /// Splits selected edges at distinct interior parameters, updating *every*
    /// incident trim, including both sides of a seam. Surfaces and geometry are
    /// not refitted; each UV curve is trimmed in its own parameterization.
    ///
    /// Existing vertices and the first segment's edge slot are retained. New
    /// vertices/segments are appended in source-edge order, highest cut first.
    /// Mapping uses bounded geometric correspondence and rejects nonmonotone or
    /// unresolved matches. Final topology and trim/edge correspondence are validated.
    /// The input is unchanged on failure; limits are 100,000 cuts and four million
    /// charged control/sample work units.
    pub fn try_split_edges_at_parameters(
        &self,
        splits: &[(usize, Vec<Real>)],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut ordered = BTreeMap::new();
        let mut count = 0usize;
        for (edge, parameters) in splits {
            let Some(source) = self.edges.get(*edge) else {
                return invalid("edge split references a missing edge");
            };
            if parameters.is_empty() || ordered.contains_key(edge) {
                return invalid("edge split needs unique edges and nonempty parameter lists");
            }
            count = count
                .checked_add(parameters.len())
                .filter(|&n| n <= MAX_SPLITS)
                .ok_or(GeometryError::InvalidBrepTopology {
                    context: "too many B-rep edge splits",
                })?;
            let mut parameters = parameters.clone();
            parameters.sort_by(Real::total_cmp);
            let domain = source.curve.domain();
            if parameters
                .iter()
                .any(|&t| !t.is_finite() || t <= *domain.start() || t >= *domain.end())
                || parameters.windows(2).any(|p| p[0] == p[1])
            {
                return Err(GeometryError::InvalidCurveSplitParameter);
            }
            ordered.insert(*edge, parameters);
        }
        if ordered.is_empty() {
            return Ok(self.clone());
        }
        let mut budget = Budget(MAX_WORK);
        let mut vertices = self.vertices.clone();
        let mut edges = self.edges.clone();
        let mut plans = BTreeMap::new();
        for (index, parameters) in ordered {
            let source = &self.edges[index];
            let mut points = Vec::with_capacity(parameters.len());
            for &parameter in &parameters {
                budget.charge(source.curve.degree().saturating_add(1))?;
                points.push(source.curve.evaluate(parameter)?);
            }
            let mut vertex_ids = Vec::with_capacity(parameters.len());
            for &point in points.iter().rev() {
                vertex_ids.push(vertices.len());
                vertices.push(BrepVertex::try_new(point, source.tolerance)?);
            }
            vertex_ids.reverse();
            let mut breaks = vec![*source.curve.domain().start()];
            breaks.extend_from_slice(&parameters);
            breaks.push(*source.curve.domain().end());
            let mut chain = vec![source.vertices[0]];
            chain.extend(vertex_ids);
            chain.push(source.vertices[1]);
            let mut edge_ids = vec![index; breaks.len() - 1];
            for piece in (1..edge_ids.len()).rev() {
                edge_ids[piece] = edges.len();
                edges.push(BrepEdge::try_new(
                    [chain[piece], chain[piece + 1]],
                    partition::subcurve(
                        &source.curve,
                        breaks[piece]..=breaks[piece + 1],
                        &mut budget,
                    )?,
                    source.tolerance,
                )?);
            }
            edges[index] = BrepEdge::try_new(
                [chain[0], chain[1]],
                partition::subcurve(&source.curve, breaks[0]..=breaks[1], &mut budget)?,
                source.tolerance,
            )?;
            plans.insert(
                index,
                SplitPlan {
                    parameters,
                    edges: edge_ids,
                    points,
                },
            );
        }
        let mut faces = self.faces.clone();
        for (face, source_face) in faces.iter_mut().zip(&self.faces) {
            if !source_face
                .loops
                .iter()
                .flat_map(|l| &l.trims)
                .any(|t| t.edge.is_some_and(|e| plans.contains_key(&e)))
            {
                continue;
            }
            budget.charge(
                source_face
                    .loops
                    .iter()
                    .flat_map(|l| &l.trims)
                    .map(|t| t.curve.control_points().len())
                    .sum(),
            )?;
            // Keep interpolation/search away from a large UV origin. Restore
            // coordinates only after subdivision; final validation still uses
            // the original tolerance and rejects unrepresentable endpoints.
            let frame = source_face.local_parameter_frame()?;
            for (boundary, local_boundary) in face.loops.iter_mut().zip(&frame.face.loops) {
                let mut trims = Vec::new();
                for (trim, local_trim) in boundary.trims.iter().zip(&local_boundary.trims) {
                    let Some(index) = trim.edge else {
                        trims.push(trim.clone());
                        continue;
                    };
                    let Some(plan) = plans.get(&index) else {
                        trims.push(trim.clone());
                        continue;
                    };
                    budget.charge(trim.curve.control_points().len())?;
                    let image = trim_image::LiftedTrim::new(local_trim, &frame.face.surface)?;
                    let cuts = correspondence::parameters(
                        &image,
                        trim,
                        &self.edges[index],
                        plan,
                        tolerance,
                        &mut budget,
                    )?;
                    let mut breaks = vec![*image.curve.domain().start()];
                    breaks.extend(cuts);
                    breaks.push(*image.curve.domain().end());
                    for piece in 0..breaks.len() - 1 {
                        let segment = if trim.reversed_3d {
                            plan.edges.len() - 1 - piece
                        } else {
                            piece
                        };
                        let edge = plan.edges[segment];
                        let curve = partition::subcurve(
                            &image.curve,
                            breaks[piece]..=breaks[piece + 1],
                            &mut budget,
                        )?;
                        let curve = NurbsCurve2::try_new_rational(
                            curve.degree(),
                            curve
                                .control_points()
                                .iter()
                                .map(|p| {
                                    WeightedPoint2::try_new(
                                        Point2::try_new(
                                            p.point().x() + frame.origin[0],
                                            p.point().y() + frame.origin[1],
                                        )?,
                                        p.weight(),
                                    )
                                })
                                .collect::<Result<Vec<_>, _>>()?,
                            curve.knots().to_vec(),
                        )?;
                        trims.push(BrepTrim::try_new(
                            oriented_edge_vertices(&edges[edge], trim.reversed_3d),
                            Some(edge),
                            trim.reversed_3d,
                            curve,
                            trim.trim_type,
                            trim.iso,
                            trim.tolerance,
                        )?);
                    }
                }
                boundary.trims = trims;
            }
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Subdivides tangent discontinuities on the requested edges. The angle
    /// criterion is inclusive; knot multiplicity alone never creates a split.
    /// Returns `None` without cloning geometry when there are no qualifying breaks.
    pub fn try_split_kinky_edges(
        &self,
        selected: &[usize],
        angle: Real,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        if !angle.is_finite() || angle <= 0. || angle > std::f64::consts::PI {
            return invalid("edge kink angle must be in (0, pi]");
        }
        let mut splits = Vec::new();
        let mut seen = BTreeSet::new();
        for &index in selected {
            let Some(edge) = self.edges.get(index) else {
                return invalid("edge split references a missing edge");
            };
            if !seen.insert(index) {
                return invalid("edge split selection contains a duplicate");
            }
            let mut parameters = Vec::new();
            for (parameter, multiplicity) in edge.curve.interior_knot_groups() {
                if multiplicity >= edge.curve.degree()
                    && edge.curve.kink_angle_at(parameter)? >= angle
                {
                    parameters.push(parameter);
                }
            }
            if !parameters.is_empty() {
                splits.push((index, parameters));
            }
        }
        if splits.is_empty() {
            Ok(None)
        } else {
            self.try_split_edges_at_parameters(&splits, tolerance)
                .map(Some)
        }
    }
}
