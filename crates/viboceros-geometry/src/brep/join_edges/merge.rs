//! Valence-two edge coalescing, with linked trim rings and certified curves.
use super::*;
use crate::ParameterSide;
mod curves;
#[cfg(test)]
mod tests;

struct Edge {
    geometry: BrepEdge,
    uses: Vec<usize>,
    displacement: Real,
    minimum_tolerance: Real,
}

#[derive(Clone)]
struct Trim {
    geometry: BrepTrim,
    previous: usize,
    next: usize,
    loop_id: (usize, usize),
}

struct State {
    edges: Vec<Option<Edge>>,
    trims: Vec<Option<Trim>>,
    heads: Vec<Vec<usize>>,
    incidence: Vec<Vec<usize>>,
    singular: Vec<bool>,
}

struct TrimPair {
    keep: usize,
    remove: usize,
    first: usize,
    last: usize,
}

impl Brep {
    /// Coalesces smooth edges meeting at valence-two vertices, updating every
    /// incident trim, including both uses of a seam. Branches and vertices
    /// incident to singular trims are retained. Existing surfaces, face senses
    /// and vertex positions are unchanged; the result is validated atomically.
    ///
    /// The angle is in radians, in `[0, pi]`. Limiting tangents are compared
    /// with atan2, retaining small angles instead of rounding their cosine to
    /// one. Curve concatenation may clamp, elevate or shift parameter domains;
    /// every original spatial piece has a whole-curve displacement certificate
    /// bounded by the modeling absolute tolerance, accumulated across merges.
    /// UV pieces require a zero-displacement certificate: their surface image
    /// is never approximated. Uncertifiable or unrepresentable merges are left
    /// separate. Rational certificates currently support degrees through 16.
    ///
    /// Surviving source table entries precede new edges. Component uncertainty
    /// is recomputed against exact surface isocurves when certified. Otherwise
    /// it propagates outward, including the modeling floor for nonzero changes
    /// and the removed vertex's uncertainty. Original edge uncertainty is never
    /// erased. Work shares the bounded Join certificate machinery.
    pub fn try_merge_all_edges(
        &self,
        angle_tolerance: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        merge(self, angle_tolerance, tolerance, &mut Budget(MAX_WORK))
    }
}

pub(super) fn merge(
    source: &Brep,
    angle: Real,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Brep, GeometryError> {
    if !angle.is_finite() || !(0.0..=std::f64::consts::PI).contains(&angle) {
        return Err(invalid("edge merge angle must be in [0, pi] radians"));
    }
    if !source.is_manifold() {
        return Err(invalid("edge merging requires manifold input"));
    }
    let mut state = State::new(source, budget)?;
    let mut changed = false;
    for original in 0..source.edges.len() {
        let mut current = original;
        while let Some(edge) = &state.edges[current] {
            let vertices = edge.geometry.vertices;
            let mut merged = None;
            for end in [1, 0] {
                let vertex = vertices[end];
                let incident = &state.incidence[vertex];
                if state.singular[vertex] || incident.len() != 2 || incident[0] == incident[1] {
                    continue;
                }
                let other = incident[usize::from(incident[0] == current)];
                if let Some(next) =
                    state.combine(source, current, other, end, angle, tolerance, budget)?
                {
                    merged = Some(next);
                    break;
                }
            }
            let Some(next) = merged else { break };
            current = next;
            changed = true;
        }
    }
    if !changed {
        source.validate(tolerance)?;
        return Ok(source.clone());
    }
    state.finish(source, tolerance, budget)
}

impl State {
    fn new(source: &Brep, budget: &mut Budget) -> Result<Self, GeometryError> {
        budget.charge(source.vertices.len().saturating_add(source.edges.len()))?;
        for edge in &source.edges {
            budget.charge(edge.curve.control_points().len())?;
        }
        for usage in source.trim_uses() {
            budget.charge(usage.trim.curve.control_points().len())?;
        }
        let mut state = Self {
            edges: source
                .edges
                .iter()
                .map(|e| {
                    Some(Edge {
                        geometry: e.clone(),
                        uses: Vec::new(),
                        displacement: 0.,
                        minimum_tolerance: e.tolerance,
                    })
                })
                .collect(),
            trims: Vec::new(),
            heads: Vec::new(),
            incidence: vec![Vec::new(); source.vertices.len()],
            singular: vec![false; source.vertices.len()],
        };
        for (i, edge) in source.edges.iter().enumerate() {
            for &v in &edge.vertices {
                state.incidence[v].push(i);
            }
        }
        for (face_id, face) in source.faces.iter().enumerate() {
            let mut heads = Vec::new();
            for (loop_id, boundary) in face.loops.iter().enumerate() {
                budget.charge(boundary.trims.len())?;
                let head = state.trims.len();
                let count = boundary.trims.len();
                heads.push(head);
                for (i, trim) in boundary.trims.iter().enumerate() {
                    if let Some(edge) = trim.edge {
                        state.edges[edge].as_mut().unwrap().uses.push(head + i);
                    } else {
                        for &v in &trim.vertices {
                            state.singular[v] = true;
                        }
                    }
                    state.trims.push(Some(Trim {
                        geometry: trim.clone(),
                        previous: head + (i + count - 1) % count,
                        next: head + (i + 1) % count,
                        loop_id: (face_id, loop_id),
                    }));
                }
            }
            state.heads.push(heads);
        }
        Ok(state)
    }

    fn trim(&self, index: usize) -> &Trim {
        self.trims[index].as_ref().unwrap()
    }

    fn pairs(&self, a: usize, b: usize, vertex: usize) -> Option<Vec<TrimPair>> {
        let (ea, eb) = (self.edges[a].as_ref()?, self.edges[b].as_ref()?);
        if ea.uses.len() != eb.uses.len() {
            return None;
        }
        let mut pairs: Vec<TrimPair> = Vec::new();
        for &keep in &ea.uses {
            let trim = self.trim(keep);
            let forward = trim.geometry.vertices[1] == vertex;
            let remove = if forward { trim.next } else { trim.previous };
            let other = self.trim(remove);
            if other.geometry.edge != Some(b)
                || other.geometry.trim_type != trim.geometry.trim_type
                || pairs.iter().any(|p| p.remove == remove)
            {
                return None;
            }
            let (first, last) = if forward {
                (keep, remove)
            } else {
                (remove, keep)
            };
            if self.trim(first).geometry.vertices[1] != vertex
                || self.trim(last).geometry.vertices[0] != vertex
            {
                return None;
            }
            pairs.push(TrimPair {
                keep,
                remove,
                first,
                last,
            });
        }
        Some(pairs)
    }

    #[allow(clippy::too_many_arguments)]
    fn combine(
        &mut self,
        source: &Brep,
        a: usize,
        b: usize,
        end: usize,
        angle: Real,
        tolerance: Tolerance,
        budget: &mut Budget,
    ) -> Result<Option<usize>, GeometryError> {
        budget.charge(8)?;
        let ea = self.edges[a].as_ref().unwrap();
        let eb = self.edges[b].as_ref().unwrap();
        if ea.geometry.curve.degree() > 16 || eb.geometry.curve.degree() > 16 {
            return Ok(None);
        }
        let vertex = ea.geometry.vertices[end];
        if ea.geometry.vertices[0] == ea.geometry.vertices[1]
            || eb.geometry.vertices[0] == eb.geometry.vertices[1]
        {
            return Ok(None);
        }
        let bend = usize::from(eb.geometry.vertices[1] == vertex);
        let Some(pairs) = self.pairs(a, b, vertex) else {
            return Ok(None);
        };
        let tangent = |e: &BrepEdge, end: usize| {
            let t = if end == 0 {
                *e.curve.domain().start()
            } else {
                *e.curve.domain().end()
            };
            e.curve.tangent_at_on_side(
                t,
                if end == 0 {
                    ParameterSide::Right
                } else {
                    ParameterSide::Left
                },
            )
        };
        budget.charge(
            ea.geometry
                .curve
                .degree()
                .saturating_add(eb.geometry.curve.degree())
                .saturating_mul(4),
        )?;
        let (Ok(ta), Ok(mut tb)) = (tangent(&ea.geometry, end), tangent(&eb.geometry, bend)) else {
            return Ok(None);
        };
        let reverse_b = end == bend;
        if reverse_b {
            tb = tb.opposite();
        }
        if ta.as_vector().angle_to(tb.as_vector())? > angle {
            return Ok(None);
        }
        let bc = if reverse_b {
            eb.geometry.curve.reversed()?
        } else {
            eb.geometry.curve.clone()
        };
        let (first, last, previous) = if end == 1 {
            (&ea.geometry.curve, &bc, [ea.displacement, eb.displacement])
        } else {
            (&bc, &ea.geometry.curve, [eb.displacement, ea.displacement])
        };
        let Some(curve) = curves::append(first, last, tolerance.absolute(), previous, budget)?
        else {
            return Ok(None);
        };
        let vertices = if end == 1 {
            [ea.geometry.vertices[0], eb.geometry.vertices[1 - bend]]
        } else {
            [eb.geometry.vertices[1 - bend], ea.geometry.vertices[1]]
        };
        let old_tolerances = if end == 1 {
            [ea.geometry.tolerance, eb.geometry.tolerance]
        } else {
            [eb.geometry.tolerance, ea.geometry.tolerance]
        };
        let propagated = |i: usize| {
            certificate::add_bound(
                if curve.bounds[i] == 0. {
                    old_tolerances[i]
                } else {
                    old_tolerances[i].max(tolerance.absolute())
                },
                curve.bounds[i],
            )
        };
        let mut edge_tolerance = propagated(0)?
            .max(propagated(1)?)
            .max(source.vertices[vertex].tolerance);
        let minimum_tolerance = ea.minimum_tolerance.max(eb.minimum_tolerance);
        let new_index = self.edges.len();
        let mut trims = Vec::new();
        for pair in &pairs {
            let first = self.trim(pair.first);
            let last = self.trim(pair.last);
            let Some(uv) = curves::append_uv(&first.geometry.curve, &last.geometry.curve, budget)?
            else {
                return Ok(None);
            };
            let old = &self.trim(pair.keep).geometry;
            trims.push(Trim {
                geometry: BrepTrim::try_new(
                    [first.geometry.vertices[0], last.geometry.vertices[1]],
                    Some(new_index),
                    old.reversed_3d,
                    uv,
                    old.trim_type,
                    if first.geometry.iso == last.geometry.iso {
                        first.geometry.iso
                    } else {
                        SurfaceIso::NotIso
                    },
                    std::array::from_fn(|i| {
                        first.geometry.tolerance[i].max(last.geometry.tolerance[i])
                    }),
                )?,
                previous: first.previous,
                next: last.next,
                loop_id: first.loop_id,
            });
        }
        let mut measured = Some(0_f64);
        for trim in &trims {
            let image = automatic::rebuild::image::BoundaryImage::new(
                &source.faces[trim.loop_id.0].surface,
                &trim.geometry,
                budget,
            )?;
            let bound = if let Some(image) = image {
                image.bound(&curve.curve, trim.geometry.reversed_3d, true, budget)?
            } else {
                None
            };
            measured = measured.zip(bound).map(|(a, b)| a.max(b));
        }
        if let Some(measured) = measured {
            edge_tolerance = minimum_tolerance.max(measured);
        }
        let redirect = |index| {
            pairs
                .iter()
                .find(|p| p.remove == index)
                .map_or(index, |p| p.keep)
        };
        for trim in &mut trims {
            trim.previous = redirect(trim.previous);
            trim.next = redirect(trim.next);
        }
        // All geometric and topology checks precede mutation of the local plan.
        self.edges[a] = None;
        self.edges[b] = None;
        self.edges.push(Some(Edge {
            geometry: BrepEdge::try_new(vertices, curve.curve, edge_tolerance)?,
            uses: pairs.iter().map(|p| p.keep).collect(),
            displacement: curve.displacement,
            minimum_tolerance,
        }));
        self.incidence[vertex].clear();
        for &v in &vertices {
            self.incidence[v].retain(|&e| e != a && e != b);
            self.incidence[v].push(new_index);
        }
        for (pair, trim) in pairs.iter().zip(trims) {
            self.trims[pair.remove] = None;
            self.trims[pair.keep] = Some(trim);
        }
        for pair in &pairs {
            let (previous, next) = (self.trim(pair.keep).previous, self.trim(pair.keep).next);
            self.trims[previous].as_mut().unwrap().next = pair.keep;
            self.trims[next].as_mut().unwrap().previous = pair.keep;
        }
        for pair in &pairs {
            let (face, boundary) = self.trim(pair.keep).loop_id;
            let head = &mut self.heads[face][boundary];
            *head = redirect(*head);
        }
        Ok(Some(new_index))
    }

    fn finish(
        self,
        source: &Brep,
        tolerance: Tolerance,
        budget: &mut Budget,
    ) -> Result<Brep, GeometryError> {
        budget.charge(self.edges.len().saturating_add(self.trims.len()))?;
        let mut used = vec![false; source.vertices.len()];
        for trim in self.trims.iter().flatten() {
            for &v in &trim.geometry.vertices {
                used[v] = true;
            }
        }
        let mut vertices = Vec::new();
        let mut vertex_map = vec![usize::MAX; used.len()];
        for (i, vertex) in source.vertices.iter().enumerate() {
            if used[i] {
                vertex_map[i] = vertices.len();
                vertices.push(*vertex);
            }
        }
        let mut edges = Vec::new();
        let mut edge_map = vec![usize::MAX; self.edges.len()];
        for (i, edge) in self.edges.iter().enumerate() {
            if let Some(edge) = edge {
                let mut edge = edge.geometry.clone();
                edge.vertices = edge.vertices.map(|v| vertex_map[v]);
                edge_map[i] = edges.len();
                edges.push(edge);
            }
        }
        let mut faces = source.faces.clone();
        for (face, heads) in faces.iter_mut().zip(&self.heads) {
            for (boundary, &head) in face.loops.iter_mut().zip(heads) {
                boundary.trims.clear();
                let mut index = head;
                loop {
                    budget.charge(1)?;
                    let node = self.trim(index);
                    let mut trim = node.geometry.clone();
                    trim.vertices = trim.vertices.map(|v| vertex_map[v]);
                    trim.edge = trim.edge.map(|e| edge_map[e]);
                    boundary.trims.push(trim);
                    index = node.next;
                    if index == head {
                        break;
                    }
                }
            }
        }
        Brep::try_new(vertices, edges, faces, tolerance)
    }
}
