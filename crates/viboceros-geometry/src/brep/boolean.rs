//! Exact half-space clipping of certified convex polygonal input shells.
//!
//! Classification and construction use the rational values of stored binary64
//! coordinates. Rounding happens once, when ordinary validated B-reps are built.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
use num_traits::{Signed, Zero};
use solid_orientation::planar::{ExactPoint, cross, dot, extract_face, point, sub, zero};

#[cfg(test)]
mod tests;

const MAX_INPUT_FACES: usize = 256;
const MAX_OUTPUT_FACES: usize = 4096;
const EXACT_WORK_LIMIT: usize = 2_000_000;
const MAX_RATIONAL_BITS: u64 = 8192;
mod difference;
pub use difference::{BrepDifferenceComponent, subtract_convex_breps};
mod polyhedral;
pub use polyhedral::{
    BrepPolyhedralBooleanComponent, BrepPolyhedralBooleanPlan, BrepPolyhedralBoundaryComponent,
    BrepPolyhedralRegion, BrepPolyhedralShell, boolean_polyhedral_breps,
    intersect_polyhedral_brep_sets, intersect_polyhedral_breps,
    polyhedral_brep_boundary_interactions, polyhedral_brep_subtraction_interactions,
    subtract_polyhedral_breps, union_polyhedral_breps,
};
mod intersection;
mod merge;
pub use intersection::{
    BrepConvexIntersection, BrepSetIntersection, intersect_convex_brep_sets, intersect_convex_breps,
};
mod union;
pub use union::{
    BrepUnionComponent, convex_brep_boundary_interactions, convex_brep_subtraction_interactions,
    union_convex_breps,
};

/// Set operation on two supported closed polygonal B-reps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrepBooleanOperation {
    Union,
    Intersection,
    /// The first operand minus the second operand.
    Difference,
}

struct Budget(usize);

impl Budget {
    fn spend(&mut self, count: usize) -> Result<(), GeometryError> {
        self.0 = self
            .0
            .checked_sub(count)
            .ok_or(GeometryError::BrepBooleanWorkLimit)?;
        Ok(())
    }
}

#[derive(Clone)]
struct Polygon<'a> {
    ring: Vec<ExactPoint>, // outward winding
    normal: ExactPoint,
    source: &'a BrepFace,
    reversed: bool,
}

impl Polygon<'_> {
    fn plane_side(&self, p: &ExactPoint) -> Rational {
        dot(&self.normal, &sub(p, &self.ring[0]))
    }

    fn reverse(mut self) -> Self {
        self.ring.reverse();
        self.normal = self.normal.map(|v| -v);
        self.reversed = !self.reversed;
        self
    }

    fn with_ring(&self, ring: Vec<ExactPoint>) -> Self {
        Self {
            ring,
            ..self.clone()
        }
    }
}

fn source_faces(
    breps: &[&Brep],
    output: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<Vec<[usize; 2]>, GeometryError> {
    let mut sources = BTreeMap::new();
    for (input, brep) in breps.iter().enumerate() {
        budget.spend(brep.faces.len())?;
        for (face, value) in brep.faces.iter().enumerate() {
            sources
                .entry(std::ptr::from_ref(value))
                .or_insert([input, face]);
        }
    }
    budget.spend(output.len())?;
    Ok(output
        .iter()
        .map(|p| sources[&std::ptr::from_ref(p.source)])
        .collect())
}

impl Brep {
    /// Computes a set operation on two certified convex polyhedral shells.
    ///
    /// Inputs must each be one closed manifold shell, with convex polygonal
    /// faces on exact affine bilinear surfaces and two-control straight edges.
    /// All stored UV/vertex correspondences and half-space convexity are checked
    /// exactly; tolerance does not turn unsupported inputs into certificates.
    /// Inward inputs are normalized. Inputs are never changed.
    ///
    /// `None` denotes the empty region. A result may contain disconnected outer
    /// shells or an inward cavity. Supporting surfaces are retained, while face
    /// polygons and shared edge subdivisions are rebuilt and validated at the
    /// supplied tolerance. Coplanar face fragments are retained separately.
    ///
    /// This is an initial kernel capability, not the general Rhino Boolean
    /// command: curved, open, nonconvex, multi-shell and numerically unresolved
    /// inputs are rejected. Work and rational size are bounded. Results whose
    /// rounded topology is collapsed or nonmanifold are rejected as well.
    pub fn try_boolean_convex(
        &self,
        other: &Self,
        operation: BrepBooleanOperation,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let mut budget = Budget(EXACT_WORK_LIMIT);
        let left = extract(self, &mut budget)?;
        let right = extract(other, &mut budget)?;
        let mut output = Vec::new();
        for (side, polygons, cutters) in [(0, &left, &right), (1, &right, &left)] {
            for polygon in polygons {
                let shared = coplanar_sense(polygon, cutters, &mut budget)?;
                match operation {
                    BrepBooleanOperation::Union => {
                        // Own a same-facing overlap once. Opposing contact
                        // patches are internal to the union and disappear.
                        if side == 0 && shared == Some(true) {
                            output.push(polygon.clone());
                        } else {
                            output.extend(partition(polygon, cutters, &mut budget)?.0);
                        }
                    }
                    BrepBooleanOperation::Intersection => {
                        if shared == Some(false) || (side == 1 && shared.is_some()) {
                            continue;
                        }
                        if let Some(inside) = partition(polygon, cutters, &mut budget)?.1 {
                            output.push(inside);
                        }
                    }
                    BrepBooleanOperation::Difference if side == 0 => {
                        if shared == Some(false) {
                            output.push(polygon.clone());
                        } else {
                            output.extend(partition(polygon, cutters, &mut budget)?.0);
                        }
                    }
                    BrepBooleanOperation::Difference => {
                        if shared.is_none()
                            && let Some(inside) = partition(polygon, cutters, &mut budget)?.1
                        {
                            output.push(inside.reverse());
                        }
                    }
                }
                if output.len() > MAX_OUTPUT_FACES {
                    return Err(GeometryError::BrepBooleanWorkLimit);
                }
            }
        }
        if output.is_empty() {
            return Ok(None);
        }
        rebuild(output, tolerance, &mut budget).map(Some)
    }
}

fn unsupported(context: &'static str) -> GeometryError {
    GeometryError::UnsupportedConvexBrepBoolean { context }
}

fn extract<'a>(brep: &'a Brep, budget: &mut Budget) -> Result<Vec<Polygon<'a>>, GeometryError> {
    if brep.faces.len() > MAX_INPUT_FACES {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    if !brep.is_solid() || brep.edge_connected_face_components().len() != 1 {
        return Err(unsupported("one closed manifold shell is required"));
    }
    let mut polygons = Vec::with_capacity(brep.faces.len());
    for face in &brep.faces {
        let s = &face.surface;
        let controls = s.control_points();
        if s.degree_u() != 1 || s.degree_v() != 1 || controls.len() != 4 {
            return Err(unsupported("exact affine bilinear faces are required"));
        }
        let p = controls
            .iter()
            .map(|c| point(c.point()))
            .collect::<Vec<_>>();
        if (0..3).any(|i| &p[3][i] + &p[0][i] != &p[1][i] + &p[2][i]) {
            return Err(unsupported("nonaffine or inexact surface control net"));
        }
        let extracted = extract_face(face, &mut budget.0).ok_or_else(|| {
            if budget.0 == 0 {
                GeometryError::BrepBooleanWorkLimit
            } else {
                unsupported("exact affine polygon trims are required")
            }
        })?;
        let [boundary] = face.loops.as_slice() else {
            return Err(unsupported("input faces with holes"));
        };
        let [ring] = extracted.loops.as_slice() else {
            return Err(unsupported("input faces with holes"));
        };
        if ring.len() != boundary.trims.len() {
            return Err(unsupported("one straight segment per trim is required"));
        }
        if ring.iter().collect::<BTreeSet<_>>().len() != ring.len() {
            return Err(unsupported("repeated polygon vertices"));
        }
        for (p, trim) in ring.iter().zip(&boundary.trims) {
            budget.spend(1)?;
            if *p != point(brep.vertices[trim.vertices[0]].point) {
                return Err(unsupported("inexact surface/vertex correspondence"));
            }
            let edge = &brep.edges[trim.edge.ok_or_else(|| unsupported("singular trims"))?];
            let controls = edge.curve.control_points();
            if edge.curve.degree() != 1
                || controls.len() != 2
                || controls[0].weight().is_sign_positive()
                    != controls[1].weight().is_sign_positive()
                || controls[0].point() != brep.vertices[edge.vertices[0]].point
                || controls[1].point() != brep.vertices[edge.vertices[1]].point
            {
                return Err(unsupported("exact two-control straight edges are required"));
            }
        }
        for i in 0..ring.len() {
            budget.spend(1)?;
            let a = &ring[i];
            let b = &ring[(i + 1) % ring.len()];
            let c = &ring[(i + 2) % ring.len()];
            let turn = cross(&sub(b, a), &sub(c, b));
            if a == b
                || dot(&extracted.normal, &turn).is_negative()
                || (zero(&turn) && !dot(&sub(b, a), &sub(c, b)).is_positive())
            {
                return Err(unsupported("nonconvex face polygon"));
            }
            for p in ring {
                budget.spend(1)?;
                if dot(&extracted.normal, &cross(&sub(b, a), &sub(p, a))).is_negative() {
                    return Err(unsupported("nonconvex or self-intersecting face polygon"));
                }
            }
        }
        let mut polygon = Polygon {
            ring: ring.clone(),
            normal: extracted.normal,
            source: face,
            reversed: extracted.reversed,
        };
        if polygon.reversed {
            polygon.ring.reverse();
            polygon.normal = polygon.normal.map(|v| -v);
        }
        polygons.push(polygon);
    }
    let vertices = brep
        .vertices
        .iter()
        .map(|v| point(v.point))
        .collect::<Vec<_>>();
    let mut outward = None;
    for polygon in &polygons {
        let mut positive = false;
        let mut negative = false;
        for vertex in &vertices {
            budget.spend(1)?;
            let distance = polygon.plane_side(vertex);
            positive |= distance.is_positive();
            negative |= distance.is_negative();
        }
        if positive == negative || outward.is_some_and(|previous| previous != negative) {
            return Err(unsupported(
                "shell is not a consistently oriented convex polyhedron",
            ));
        }
        outward = Some(negative);
    }
    if outward == Some(false) {
        polygons = polygons.into_iter().map(Polygon::reverse).collect();
    }
    Ok(polygons)
}

/// Some(true) for the same outward plane, Some(false) for an opposing plane.
fn coplanar_sense(
    polygon: &Polygon<'_>,
    cutters: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<Option<bool>, GeometryError> {
    for cutter in cutters {
        budget.spend(1)?;
        if zero(&cross(&polygon.normal, &cutter.normal))
            && cutter.plane_side(&polygon.ring[0]).is_zero()
        {
            return Ok(Some(dot(&polygon.normal, &cutter.normal).is_positive()));
        }
    }
    Ok(None)
}

type Partition<'a> = (Vec<Polygon<'a>>, Option<Polygon<'a>>);

/// Disjoint convex outside fragments and the remaining convex inside fragment.
fn partition<'a>(
    polygon: &Polygon<'a>,
    cutters: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<Partition<'a>, GeometryError> {
    let mut candidate = polygon.ring.clone();
    let mut outside = Vec::new();
    for cutter in cutters {
        let (inside, piece) = split(&candidate, cutter, budget)?;
        if let Some(piece) = piece {
            outside.push(polygon.with_ring(piece));
        }
        let Some(inside) = inside else {
            return Ok((outside, None));
        };
        candidate = inside;
    }
    Ok((outside, Some(polygon.with_ring(candidate))))
}

type RingSplit = (Option<Vec<ExactPoint>>, Option<Vec<ExactPoint>>);

fn split(
    ring: &[ExactPoint],
    plane: &Polygon<'_>,
    budget: &mut Budget,
) -> Result<RingSplit, GeometryError> {
    split_plane(ring, &plane.ring[0], &plane.normal, budget)
}

fn split_plane(
    ring: &[ExactPoint],
    anchor: &ExactPoint,
    normal: &ExactPoint,
    budget: &mut Budget,
) -> Result<RingSplit, GeometryError> {
    budget.spend(ring.len())?;
    let sides = ring
        .iter()
        .map(|p| dot(normal, &sub(p, anchor)))
        .collect::<Vec<_>>();
    if sides.iter().all(|s| !s.is_positive()) {
        return Ok((Some(ring.to_vec()), None));
    }
    if sides.iter().all(|s| !s.is_negative()) {
        return Ok((None, Some(ring.to_vec())));
    }
    let mut inside = Vec::new();
    let mut outside = Vec::new();
    for i in 0..ring.len() {
        let j = (i + 1) % ring.len();
        if !sides[i].is_positive() {
            inside.push(ring[i].clone());
        }
        if !sides[i].is_negative() {
            outside.push(ring[i].clone());
        }
        if (sides[i].is_positive() && sides[j].is_negative())
            || (sides[i].is_negative() && sides[j].is_positive())
        {
            let t = &sides[i] / (&sides[i] - &sides[j]);
            let p: ExactPoint =
                std::array::from_fn(|axis| &ring[i][axis] + &t * (&ring[j][axis] - &ring[i][axis]));
            check_point(&p)?;
            inside.push(p.clone());
            outside.push(p);
        }
    }
    Ok((clean_ring(inside, budget)?, clean_ring(outside, budget)?))
}

fn check_scalar(value: &Rational) -> Result<(), GeometryError> {
    if value.numer().bits() > MAX_RATIONAL_BITS || value.denom().bits() > MAX_RATIONAL_BITS {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    Ok(())
}

fn check_point(p: &ExactPoint) -> Result<(), GeometryError> {
    if p.iter()
        .any(|v| v.numer().bits() > MAX_RATIONAL_BITS || v.denom().bits() > MAX_RATIONAL_BITS)
    {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    Ok(())
}

fn clean_ring(
    mut ring: Vec<ExactPoint>,
    budget: &mut Budget,
) -> Result<Option<Vec<ExactPoint>>, GeometryError> {
    ring.dedup();
    if ring.first() == ring.last() {
        ring.pop();
    }
    loop {
        if ring.len() < 3 {
            return Ok(None);
        }
        budget.spend(ring.len())?;
        let removable = (0..ring.len()).find(|&i| {
            let a = &ring[(i + ring.len() - 1) % ring.len()];
            let b = &ring[i];
            let c = &ring[(i + 1) % ring.len()];
            zero(&cross(&sub(b, a), &sub(c, b)))
        });
        match removable {
            Some(i) => {
                ring.remove(i);
            }
            None => return Ok(Some(ring)),
        }
    }
}

fn subdivide(
    polygons: &mut [Polygon<'_>],
    budget: &mut Budget,
) -> Result<BTreeSet<ExactPoint>, GeometryError> {
    let points = polygons
        .iter()
        .flat_map(|p| p.ring.iter().cloned())
        .collect::<BTreeSet<_>>();
    // Exact shared subdivisions resolve T-junctions created by successive cuts.
    for polygon in polygons.iter_mut() {
        let mut split_ring = Vec::new();
        for i in 0..polygon.ring.len() {
            let a = &polygon.ring[i];
            let b = &polygon.ring[(i + 1) % polygon.ring.len()];
            let direction = sub(b, a);
            let axis = direction
                .iter()
                .position(|v| !v.is_zero())
                .ok_or(GeometryError::UnrepresentableBrepBoolean)?;
            let mut on_segment = Vec::new();
            for p in &points {
                budget.spend(1)?;
                if (0..3).any(|k| {
                    p[k] < a[k].clone().min(b[k].clone()) || p[k] > a[k].clone().max(b[k].clone())
                }) {
                    continue;
                }
                let delta = sub(p, a);
                if zero(&cross(&delta, &direction)) {
                    let t = &delta[axis] / &direction[axis];
                    if t >= Rational::zero() && t < rational(1.) {
                        on_segment.push((t, p.clone()));
                    }
                }
            }
            on_segment.sort_by(|a, b| a.0.cmp(&b.0));
            split_ring.extend(on_segment.into_iter().map(|(_, p)| p));
        }
        polygon.ring = split_ring;
    }
    Ok(points)
}

fn rebuild(
    polygons: Vec<Polygon<'_>>,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Brep, GeometryError> {
    let result = rebuild_boundary(polygons, tolerance, budget)?;
    if !result.is_solid() {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    // Solid expressions reject point-shared disconnected shells.
    let mut owners = BTreeMap::new();
    for (component, faces) in result.edge_connected_face_components().iter().enumerate() {
        for &face in faces {
            for vertex in result.faces[face]
                .loops
                .iter()
                .flat_map(|l| &l.trims)
                .flat_map(|t| t.vertices)
            {
                if owners
                    .insert(vertex, component)
                    .is_some_and(|previous| previous != component)
                {
                    return Err(GeometryError::UnrepresentableBrepBoolean);
                }
            }
        }
    }
    Ok(result)
}

/// Validated boundary topology, including intentional nonmanifold contacts.
/// Exact point collapse during rounding is still rejected. Solid callers add
/// their manifold certificates separately.
fn rebuild_boundary(
    mut polygons: Vec<Polygon<'_>>,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Brep, GeometryError> {
    let points = subdivide(&mut polygons, budget)?;
    let mut vertices = Vec::new();
    let mut rounded = BTreeSet::new();
    let mut vertex_map = BTreeMap::new();
    for p in points {
        let coordinates = [scalar(&p[0])?, scalar(&p[1])?, scalar(&p[2])?];
        let key = coordinates.map(|v| if v == 0. { 0 } else { v.to_bits() });
        if !rounded.insert(key) {
            return Err(GeometryError::UnrepresentableBrepBoolean);
        }
        vertex_map.insert(p, vertices.len());
        vertices.push(BrepVertex::try_new(Point3::try_from(coordinates)?, 0.)?);
    }
    let mut edges = Vec::new();
    let mut edge_map = BTreeMap::new();
    let mut edge_uses: Vec<usize> = Vec::new();
    let mut faces = Vec::new();
    for polygon in polygons {
        let surface = &polygon.source.surface;
        let mut ring = polygon.ring;
        if polygon.reversed {
            ring.reverse();
        }
        let mut trims = Vec::with_capacity(ring.len());
        for i in 0..ring.len() {
            let j = (i + 1) % ring.len();
            let indices = [vertex_map[&ring[i]], vertex_map[&ring[j]]];
            let key = [indices[0].min(indices[1]), indices[0].max(indices[1])];
            let edge = if let Some(&edge) = edge_map.get(&key) {
                edge
            } else {
                let edge = edges.len();
                edge_map.insert(key, edge);
                edges.push(BrepEdge::try_new(
                    key,
                    NurbsCurve::try_new(
                        1,
                        key.map(|v| vertices[v].point).to_vec(),
                        vec![0., 0., 1., 1.],
                    )?,
                    0.,
                )?);
                edge_uses.push(0);
                edge
            };
            edge_uses[edge] += 1;
            trims.push(BrepTrim::try_new(
                indices,
                Some(edge),
                indices != key,
                NurbsCurve2::try_line(uv(surface, &ring[i])?, uv(surface, &ring[j])?)?,
                BrepTrimType::Mated,
                SurfaceIso::NotIso,
                [0., 0.],
            )?);
        }
        faces.push(BrepFace::try_from_polygon_boundaries(
            surface.clone(),
            polygon.reversed,
            vec![trims],
        )?);
    }
    if edge_uses.iter().any(|&n| n < 2) {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    Brep::try_new(vertices, edges, faces, tolerance)
}

fn uv(surface: &NurbsSurface, p: &ExactPoint) -> Result<Point2, GeometryError> {
    let values = uv_exact(surface, p)?;
    Point2::try_new(scalar(&values[0])?, scalar(&values[1])?)
}

fn uv_exact(surface: &NurbsSurface, p: &ExactPoint) -> Result<[Rational; 2], GeometryError> {
    let c = surface.control_points();
    let origin = point(c[0].point());
    let u = sub(&point(c[1].point()), &origin);
    let v = sub(&point(c[2].point()), &origin);
    let delta = sub(p, &origin);
    for a in 0..3 {
        let b = (a + 1) % 3;
        let determinant = &u[a] * &v[b] - &u[b] * &v[a];
        if determinant.is_zero() {
            continue;
        }
        let fractions = [
            (&delta[a] * &v[b] - &delta[b] * &v[a]) / &determinant,
            (&u[a] * &delta[b] - &u[b] * &delta[a]) / &determinant,
        ];
        let mut values = std::array::from_fn(|_| Rational::zero());
        for (i, domain) in [surface.domain_u(), surface.domain_v()].iter().enumerate() {
            let value = rational(*domain.start())
                + &fractions[i] * (rational(*domain.end()) - rational(*domain.start()));
            values[i] = value;
        }
        return Ok(values);
    }
    Err(GeometryError::UnrepresentableBrepBoolean)
}
