//! Output geometry shares the validated Inset construction and exact edge cuts.
use super::*;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanarInsetOutputMode {
    Joined,
    Separate,
    Curves,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlanarInsetOutput {
    Breps(Vec<Brep>),
    Curves(Vec<NurbsCurve>),
}

impl Brep {
    /// Produces joined surfaces, edge-separated surfaces, or cap loops and
    /// crease connectors. Curve output does not include the source geometry.
    /// Separate output connects components through old edges and cuts new
    /// edges across components. Borders can stay attached to unselected faces;
    /// new edges inside a component remain joined.
    /// Component order follows the shared kernel's source-face order.
    pub fn try_inset_planar_faces_output(
        &self,
        selected: &[usize],
        options: PlanarInsetOptions,
        mode: PlanarInsetOutputMode,
        tolerance: Tolerance,
        operations: PlanarInsetOperations,
    ) -> Result<PlanarInsetOutput, GeometryError> {
        let result =
            self.try_inset_planar_faces_with_options(selected, options, tolerance, operations)?;
        if mode == PlanarInsetOutputMode::Joined {
            return Ok(PlanarInsetOutput::Breps(vec![result]));
        }
        let first_new = self.edges.len();
        if mode == PlanarInsetOutputMode::Separate {
            // Components are connected through old edges. New edges internal
            // to one component remain joined, including crease connectors on
            // a border still attached to unselected faces.
            let components = result.edge_connected_face_components_where(|e| e < first_new);
            let mut membership = vec![0; result.faces.len()];
            for (c, faces) in components.iter().enumerate() {
                for &f in faces {
                    membership[f] = c;
                }
            }
            let mut first = vec![None; result.edges.len()];
            let mut cut = vec![false; result.edges.len()];
            for (f, face) in result.faces.iter().enumerate() {
                for trim in face.loops.iter().flat_map(|l| &l.trims) {
                    if let Some(e) = trim.edge {
                        if let Some(c) = first[e] {
                            cut[e] |= c != membership[f];
                        } else {
                            first[e] = Some(membership[f]);
                        }
                    }
                }
            }
            let edges = (first_new..result.edges.len())
                .filter(|&e| cut[e])
                .collect::<Vec<_>>();
            let mut work = Work {
                bytes: result.inset_storage_estimate(&BTreeSet::new(), false)?,
            };
            for &edge in &edges {
                work.charge_curve(&result.edges[edge].curve)?;
            }
            let parts = result.try_unjoin_edges(&edges, tolerance)?;
            if parts.is_empty() {
                return Err(invalid("inset produced no separated surfaces"));
            }
            return Ok(PlanarInsetOutput::Breps(parts));
        }
        let mut work = Work { bytes: 0 };
        let mut curves = Vec::new();
        let mut used = vec![false; result.edges.len()];
        for index in selected.iter().copied().collect::<BTreeSet<_>>() {
            for boundary in &result.faces[index].loops {
                let curve = curves::joined(&result, &boundary.trims)?;
                work.charge_curve(&curve)?;
                curves.push(curve);
                for trim in &boundary.trims {
                    if let Some(e) = trim.edge {
                        used[e] = true;
                    }
                }
            }
        }
        for (i, edge) in result.edges.iter().enumerate().skip(first_new) {
            if !used[i] {
                work.charge_curve(&edge.curve)?;
                curves.push(edge.curve.clone());
            }
        }
        Ok(PlanarInsetOutput::Curves(curves))
    }
}
