//! Remove internal coplanar face edges and reconstruct polygon loops and holes.
use super::*;

impl Brep {
    /// Merges edge-adjacent exact affine planar polygon faces within caller groups.
    ///
    /// One group label per face is required. Only equally oriented coplanar
    /// affine bilinear faces with straight polygon trim/edge images participate;
    /// other faces remain unchanged. Interior edges disappear, exterior 3D edge
    /// curves are retained, and their exact affine projections provide UV trims.
    /// A supporting plane is extended if the combined region needs it. Outer
    /// and hole boundaries are reconstructed and validated without loosening
    /// tolerance. Ambiguous boundary junctions return errors. No merge is `None`.
    ///
    /// This does not merge exterior edge subdivisions; `try_merge_all_edges`
    /// can remove eligible collinear subdivisions afterward.
    pub fn try_merge_coplanar_polygon_faces_in_groups(
        &self,
        groups: &[usize],
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        if groups.len() != self.faces.len() {
            return Err(GeometryError::InvalidBrepTopology {
                context: "one coplanar merge group per face is required",
            });
        }
        let mut budget = Budget(EXACT_WORK_LIMIT);
        for face in &self.faces {
            budget.spend(1)?;
            for trim in face.loops.iter().flat_map(|l| &l.trims) {
                budget.spend(trim.curve.control_points().len())?;
            }
        }
        let planes = self
            .faces
            .iter()
            .map(|face| affine_plane(self, face))
            .collect::<Vec<_>>();
        let mut parents = (0..self.faces.len()).collect::<Vec<_>>();
        let mut first: Vec<Option<usize>> = vec![None; self.edges.len()];
        for (face, value) in self.faces.iter().enumerate() {
            for trim in value.loops.iter().flat_map(|l| &l.trims) {
                let Some(edge) = trim.edge else {
                    continue;
                };
                if let Some(other) = first[edge] {
                    budget.spend(1)?;
                    if groups[face] == groups[other]
                        && let (Some(a), Some(b)) = (&planes[face], &planes[other])
                        && zero(&cross(&a.1, &b.1))
                        && dot(&a.1, &b.1).is_positive()
                        && dot(&a.1, &sub(&b.0, &a.0)).is_zero()
                    {
                        let a = root(&mut parents, face);
                        let b = root(&mut parents, other);
                        if a != b {
                            parents[a.max(b)] = a.min(b);
                        }
                    }
                } else {
                    first[edge] = Some(face);
                }
            }
        }
        let mut components = BTreeMap::<usize, Vec<usize>>::new();
        for face in 0..self.faces.len() {
            components
                .entry(root(&mut parents, face))
                .or_default()
                .push(face);
        }
        if components.len() == self.faces.len() {
            return Ok(None);
        }
        let mut faces = Vec::with_capacity(components.len());
        for component in components.values() {
            faces.push(if component.len() == 1 {
                self.faces[component[0]].clone()
            } else {
                merged_face(self, component, &mut budget)?
            });
        }
        compact(self, faces, tolerance).map(Some)
    }
}

fn root(parents: &mut [usize], mut i: usize) -> usize {
    while parents[i] != i {
        parents[i] = parents[parents[i]];
        i = parents[i];
    }
    i
}

fn affine_plane(brep: &Brep, face: &BrepFace) -> Option<(ExactPoint, ExactPoint)> {
    let surface = &face.surface;
    let controls = surface.control_points();
    if surface.degree_u() != 1
        || surface.degree_v() != 1
        || controls.len() != 4
        || controls.iter().any(|p| p.weight() != controls[0].weight())
    {
        return None;
    }
    let p = controls
        .iter()
        .map(|c| point(c.point()))
        .collect::<Vec<_>>();
    if (0..3).any(|i| &p[3][i] + &p[0][i] != &p[1][i] + &p[2][i]) {
        return None;
    }
    if face.loops.iter().flat_map(|l| &l.trims).any(|trim| {
        !trim.curve.is_straight_segment()
            || trim.edge.is_none_or(|edge| {
                let curve = &brep.edges[edge].curve;
                curve.degree() != 1
                    || curve.control_points().len() != 2
                    || curve.control_points()[0].weight().is_sign_positive()
                        != curve.control_points()[1].weight().is_sign_positive()
            })
    }) {
        return None;
    }
    let mut normal = cross(&sub(&p[1], &p[0]), &sub(&p[2], &p[0]));
    if zero(&normal) {
        return None;
    }
    if face.reversed {
        normal = normal.map(|v| -v);
    }
    Some((p[0].clone(), normal))
}

fn merged_face(
    brep: &Brep,
    component: &[usize],
    budget: &mut Budget,
) -> Result<BrepFace, GeometryError> {
    let source = &brep.faces[component[0]];
    let global_uses = brep.edge_use_counts();
    let mut counts = BTreeMap::new();
    for &face in component {
        for trim in brep.faces[face].loops.iter().flat_map(|l| &l.trims) {
            budget.spend(1)?;
            *counts
                .entry(trim.edge.ok_or(GeometryError::UnrepresentableBrepBoolean)?)
                .or_insert(0usize) += 1;
        }
    }
    let mut boundary = BTreeMap::new();
    for &face in component {
        let value = &brep.faces[face];
        for trim in value.loops.iter().flat_map(|l| &l.trims) {
            let edge = trim.edge.ok_or(GeometryError::UnrepresentableBrepBoolean)?;
            if counts[&edge] == 2 {
                continue;
            }
            if counts[&edge] != 1 {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            }
            let mut vertices = trim.vertices;
            if value.reversed != source.reversed {
                vertices.reverse();
            }
            if boundary.insert(vertices[0], (edge, vertices)).is_some() {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            }
        }
    }
    let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    for (_, vertices) in boundary.values() {
        let p = uv(&source.surface, &point(brep.vertices[vertices[0]].point))?;
        for (i, value) in [p.x(), p.y()].into_iter().enumerate() {
            bounds[i][0] = bounds[i][0].min(value);
            bounds[i][1] = bounds[i][1].max(value);
        }
    }
    let surface = if source.surface.domain_u().contains(&bounds[0][0])
        && source.surface.domain_u().contains(&bounds[0][1])
        && source.surface.domain_v().contains(&bounds[1][0])
        && source.surface.domain_v().contains(&bounds[1][1])
    {
        source.surface.clone()
    } else {
        extended_surface(&source.surface, bounds)?
    };
    let mut loops = Vec::new();
    while let Some((&start, _)) = boundary.first_key_value() {
        let mut current = start;
        let mut trims = Vec::new();
        loop {
            budget.spend(1)?;
            let (edge, vertices) = boundary
                .remove(&current)
                .ok_or(GeometryError::UnrepresentableBrepBoolean)?;
            let original = &brep.edges[edge];
            let reversed = vertices != original.vertices;
            let mut curve = NurbsCurve2::try_new_rational(
                original.curve.degree(),
                original
                    .curve
                    .control_points()
                    .iter()
                    .map(|c| {
                        WeightedPoint2::try_new(uv(&source.surface, &point(c.point()))?, c.weight())
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?,
                original.curve.knots().to_vec(),
            )?;
            if reversed {
                curve = curve.reversed()?;
            }
            trims.push(BrepTrim::try_new(
                vertices,
                Some(edge),
                reversed,
                curve,
                if global_uses[edge] == 1 {
                    BrepTrimType::Boundary
                } else {
                    BrepTrimType::Mated
                },
                SurfaceIso::NotIso,
                [0., 0.],
            )?);
            current = vertices[1];
            if current == start {
                break;
            }
        }
        loops.push(trims);
    }
    BrepFace::try_from_polygon_boundaries(surface, source.reversed, loops)
}

pub(super) fn extended_surface(
    source: &NurbsSurface,
    bounds: [[Real; 2]; 2],
) -> Result<NurbsSurface, GeometryError> {
    let c = source.control_points();
    let origin = point(c[0].point());
    let u = sub(&point(c[1].point()), &origin);
    let v = sub(&point(c[2].point()), &origin);
    let domains = [source.domain_u(), source.domain_v()];
    let mut controls = Vec::new();
    for b in bounds[1] {
        for a in bounds[0] {
            let fractions = [a, b].map(rational);
            let fractions: [Rational; 2] = std::array::from_fn(|i| {
                (&fractions[i] - rational(*domains[i].start()))
                    / (rational(*domains[i].end()) - rational(*domains[i].start()))
            });
            let p: ExactPoint =
                std::array::from_fn(|i| &origin[i] + &fractions[0] * &u[i] + &fractions[1] * &v[i]);
            check_point(&p)?;
            controls.push(Point3::try_from([
                scalar(&p[0])?,
                scalar(&p[1])?,
                scalar(&p[2])?,
            ])?);
        }
    }
    NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        controls,
        vec![bounds[0][0], bounds[0][0], bounds[0][1], bounds[0][1]],
        vec![bounds[1][0], bounds[1][0], bounds[1][1], bounds[1][1]],
    )
}

fn compact(
    source: &Brep,
    mut faces: Vec<BrepFace>,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let mut vertex_map = BTreeMap::new();
    let mut edge_map = BTreeMap::new();
    for face in &faces {
        for trim in face.loops.iter().flat_map(|l| &l.trims) {
            for vertex in trim.vertices {
                vertex_map.insert(vertex, 0usize);
            }
            if let Some(edge) = trim.edge {
                edge_map.insert(edge, 0usize);
                for vertex in source.edges[edge].vertices {
                    vertex_map.insert(vertex, 0usize);
                }
            }
        }
    }
    let mut vertices = Vec::new();
    for (&old, new) in &mut vertex_map {
        *new = vertices.len();
        vertices.push(source.vertices[old]);
    }
    let mut edges = Vec::new();
    for (&old, new) in &mut edge_map {
        *new = edges.len();
        let original = &source.edges[old];
        edges.push(BrepEdge::try_new(
            original.vertices.map(|v| vertex_map[&v]),
            original.curve.clone(),
            original.tolerance,
        )?);
    }
    for face in &mut faces {
        for trim in face.loops.iter_mut().flat_map(|l| &mut l.trims) {
            trim.vertices = trim.vertices.map(|v| vertex_map[&v]);
            trim.edge = trim.edge.map(|e| edge_map[&e]);
        }
    }
    Brep::try_new(vertices, edges, faces, tolerance)
}
