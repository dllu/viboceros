use std::collections::{BTreeMap, HashMap};
use std::f64::consts::TAU;

use crate::{
    AffineTransform3, BoundingBox3, Circle3, GeometryError, LineSegment, NurbsCurve, Point3, Real,
    Tolerance, UnitVector3, Vector3, require_finite, vector::product_three,
};

/// Allocation guard for regular-polygon construction.
pub const MAX_REGULAR_POLYGON_SIDES: usize = 100_000;

/// One connected result from [`join_polylines`].
#[derive(Clone, Debug, PartialEq)]
pub struct JoinedPolyline3 {
    polyline: Polyline3,
    source_indices: Vec<usize>,
}

impl JoinedPolyline3 {
    #[inline]
    pub const fn polyline(&self) -> &Polyline3 {
        &self.polyline
    }

    /// Indices into the input slice, sorted into input order.
    #[inline]
    pub fn source_indices(&self) -> &[usize] {
        &self.source_indices
    }
}

/// A finite piecewise-linear curve with validated, non-degenerate segments.
///
/// Closed polylines repeat their first vertex at the end. Non-adjacent
/// duplicate vertices and self-intersections are intentionally permitted.
#[derive(Clone, Debug, PartialEq)]
pub struct Polyline3 {
    vertices: Vec<Point3>,
    parameters: Vec<Real>,
}

impl Polyline3 {
    pub fn try_new(vertices: Vec<Point3>, tolerance: Tolerance) -> Result<Self, GeometryError> {
        let parameters = (0..vertices.len()).map(|index| index as Real).collect();
        Self::try_with_parameters(vertices, parameters, tolerance)
    }

    /// Retains a native parameter at every vertex. Newly constructed
    /// polylines use vertex indices; joined curves use chord lengths.
    pub fn try_with_parameters(
        vertices: Vec<Point3>,
        parameters: Vec<Real>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if vertices.len() < 2 {
            return Err(GeometryError::InsufficientPolylineVertices);
        }
        if parameters.len() != vertices.len()
            || !(parameters.last().copied().unwrap_or(0.0)
                - parameters.first().copied().unwrap_or(0.0))
            .is_finite()
            || parameters.iter().any(|value| !value.is_finite())
            || parameters
                .windows(2)
                .any(|pair| pair[0] >= pair[1] || !(pair[1] - pair[0]).is_finite())
        {
            return Err(GeometryError::InvalidPolylineParameters);
        }
        for (segment, points) in vertices.windows(2).enumerate() {
            match LineSegment::try_new(points[0], points[1], tolerance) {
                Ok(_) => {}
                Err(GeometryError::Degenerate { .. }) => {
                    return Err(GeometryError::DegeneratePolylineSegment { segment });
                }
                Err(error) => return Err(error),
            }
        }
        Ok(Self {
            vertices,
            parameters,
        })
    }

    /// Constructs a closed regular polygon in the plane described by
    /// `normal`. The first vertex is projected into that plane and fixes both
    /// the circumradius and angular phase.
    pub fn try_regular_polygon(
        side_count: usize,
        center: Point3,
        first_vertex: Point3,
        normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !(3..=MAX_REGULAR_POLYGON_SIDES).contains(&side_count) {
            return Err(GeometryError::InvalidRegularPolygonSides {
                actual: side_count,
                maximum: MAX_REGULAR_POLYGON_SIDES,
            });
        }
        let circle = Circle3::try_from_center_point(center, first_vertex, normal, tolerance)?;
        let mut vertices = Vec::with_capacity(side_count + 1);
        for index in 0..side_count {
            let angle = TAU * index as Real / side_count as Real;
            vertices.push(circle.point_at_angle(angle)?);
        }
        vertices.push(vertices[0]);
        Self::try_new(vertices, tolerance)
    }

    #[inline]
    pub fn vertices(&self) -> &[Point3] {
        &self.vertices
    }

    pub fn parameters(&self) -> &[Real] {
        &self.parameters
    }

    pub fn domain(&self) -> std::ops::RangeInclusive<Real> {
        self.parameters[0]..=*self.parameters.last().expect("a polyline has parameters")
    }

    pub(crate) fn parameter_location(
        &self,
        parameter: Real,
    ) -> Result<(usize, Real), GeometryError> {
        let domain = self.domain();
        if !domain.contains(&parameter) {
            return Err(GeometryError::ParameterOutOfDomain {
                parameter,
                domain_start: *domain.start(),
                domain_end: *domain.end(),
            });
        }
        let index = self
            .parameters
            .partition_point(|value| *value <= parameter)
            .saturating_sub(1)
            .min(self.segment_count() - 1);
        let fraction = (parameter - self.parameters[index])
            / (self.parameters[index + 1] - self.parameters[index]);
        Ok((index, fraction))
    }

    pub fn evaluate(&self, parameter: Real) -> Result<Point3, GeometryError> {
        let (index, fraction) = self.parameter_location(parameter)?;
        LineSegment::from_validated(
            self.vertices[index],
            self.vertices[index + 1],
            [self.parameters[index], self.parameters[index + 1]],
        )
        .point_at(fraction)
    }

    /// Exact NURBS form without the chord-length reparameterization performed
    /// by the `ToNURBS` command.
    pub fn to_native_nurbs(&self) -> Result<NurbsCurve, GeometryError> {
        let mut knots = Vec::with_capacity(self.parameters.len() + 2);
        knots.push(self.parameters[0]);
        knots.extend_from_slice(&self.parameters);
        knots.push(*self.parameters.last().expect("a polyline has parameters"));
        NurbsCurve::try_new(1, self.vertices.clone(), knots)
    }

    pub fn try_chord_length_parameterized(&self) -> Result<Self, GeometryError> {
        let nurbs = self.to_nurbs()?;
        let parameters = nurbs.knots()[1..nurbs.knots().len() - 1].to_vec();
        if parameters.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(GeometryError::InvalidPolylineParameters);
        }
        Ok(Self {
            vertices: self.vertices.clone(),
            parameters,
        })
    }

    #[inline]
    pub fn segment_count(&self) -> usize {
        self.vertices.len() - 1
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        self.segment_count() >= 3 && self.vertices.first() == self.vertices.last()
    }

    pub fn segments(&self) -> impl ExactSizeIterator<Item = LineSegment> + '_ {
        self.vertices
            .windows(2)
            .zip(self.parameters.windows(2))
            .map(|(points, parameters)| {
                LineSegment::from_validated(points[0], points[1], [parameters[0], parameters[1]])
            })
    }

    pub fn bounds(&self) -> BoundingBox3 {
        BoundingBox3::from_points(self.vertices.iter().copied())
            .expect("a validated polyline has vertices")
    }

    pub fn length(&self) -> Result<Real, GeometryError> {
        // Neumaier summation retains small segments beside large ones while
        // still reporting an unrepresentable total instead of storing inf.
        let mut sum = 0.0;
        let mut correction = 0.0;
        for segment in self.segments() {
            let length = segment.length()?;
            let next = sum + length;
            if sum.abs() >= length.abs() {
                correction += (sum - next) + length;
            } else {
                correction += (length - next) + sum;
            }
            sum = next;
        }
        let length = sum + correction;
        require_finite([length], "polyline length")?;
        Ok(length)
    }

    /// Returns the point on this polyline nearest to `target`, breaking exact
    /// ties by segment order.
    pub fn closest_point(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<Point3, GeometryError> {
        let mut best = None;
        for segment in self.segments() {
            let point = segment.closest_point(target, tolerance)?;
            let distance = point.distance_to(target)?;
            if best.is_none_or(|(best_distance, _)| distance < best_distance) {
                best = Some((distance, point));
            }
        }
        Ok(best
            .expect("a validated polyline has at least one segment")
            .1)
    }

    pub fn reversed(&self) -> Self {
        let mut vertices = self.vertices.clone();
        vertices.reverse();
        let parameters = self
            .parameters
            .iter()
            .rev()
            .map(|parameter| -parameter)
            .collect();
        Self {
            vertices,
            parameters,
        }
    }

    /// Returns the absolute algebraic area of a closed planar polyline.
    /// Self-intersecting regions follow the usual signed-boundary convention.
    pub fn planar_area(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        if !self.is_closed() {
            return Err(GeometryError::OpenPolylineArea);
        }
        let origin = self.vertices[0];
        let relative = self
            .vertices
            .iter()
            .map(|point| origin.vector_to(*point))
            .collect::<Result<Vec<_>, _>>()?;
        let first_direction = relative
            .iter()
            .find_map(|vector| vector.normalized(tolerance).ok())
            .ok_or(GeometryError::DegeneratePlanarRegion)?;
        let normal = relative
            .iter()
            .filter_map(|vector| vector.normalized(tolerance).ok())
            .find_map(|direction| {
                first_direction
                    .as_vector()
                    .cross(direction.as_vector())
                    .ok()
                    .filter(|cross| cross.length().is_ok_and(|sine| sine > tolerance.angular()))
                    .and_then(|cross| cross.normalized_nonzero().ok())
            })
            .ok_or(GeometryError::DegeneratePlanarRegion)?;

        let mut scale = 0.0_f64;
        let mut maximum_radius = 0.0_f64;
        for vector in &relative {
            scale = vector
                .to_array()
                .into_iter()
                .map(Real::abs)
                .fold(scale, Real::max);
            maximum_radius = maximum_radius.max(vector.length()?);
        }
        let planar_tolerance = tolerance
            .absolute()
            .max(tolerance.relative() * maximum_radius);
        for vector in &relative {
            if vector.dot(normal.as_vector())?.abs() > planar_tolerance {
                return Err(GeometryError::NonPlanarPolyline);
            }
        }
        if let Some(area) = direct_planar_area(&relative, normal) {
            return Ok(area);
        }

        let mut sum = [0.0; 3];
        let mut correction = [0.0; 3];
        for vectors in relative.windows(2) {
            let left = vectors[0].to_array().map(|coordinate| coordinate / scale);
            let right = vectors[1].to_array().map(|coordinate| coordinate / scale);
            let cross = [
                left[1].mul_add(right[2], -left[2] * right[1]),
                left[2].mul_add(right[0], -left[0] * right[2]),
                left[0].mul_add(right[1], -left[1] * right[0]),
            ];
            for coordinate in 0..3 {
                let next = sum[coordinate] + cross[coordinate];
                if sum[coordinate].abs() >= cross[coordinate].abs() {
                    correction[coordinate] += (sum[coordinate] - next) + cross[coordinate];
                } else {
                    correction[coordinate] += (cross[coordinate] - next) + sum[coordinate];
                }
                sum[coordinate] = next;
            }
        }
        let sum =
            std::array::from_fn::<_, 3, _>(|coordinate| sum[coordinate] + correction[coordinate]);
        let normal = normal.as_vector().to_array();
        let unit_area = 0.5
            * sum
                .into_iter()
                .zip(normal)
                .map(|(component, normal)| component * normal)
                .sum::<Real>()
                .abs();
        require_finite([unit_area], "polyline area")?;
        if unit_area == 0.0 {
            return Ok(0.0);
        }
        product_three(unit_area, scale, scale, "polyline area")
    }

    pub fn transformed(
        &self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let vertices = self
            .vertices
            .iter()
            .map(|vertex| transform.transform_point(*vertex))
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_with_parameters(vertices, self.parameters.clone(), tolerance)
    }

    /// Returns Rhino's exact degree-one, chord-length-parameterized NURBS form.
    pub fn to_nurbs(&self) -> Result<NurbsCurve, GeometryError> {
        let mut knots = Vec::with_capacity(self.vertices.len() + 2);
        knots.extend([0.0, 0.0]);
        let mut sum = 0.0;
        let mut correction = 0.0;
        for segment in self.segments() {
            let length = segment.length()?;
            let next = sum + length;
            if sum.abs() >= length.abs() {
                correction += (sum - next) + length;
            } else {
                correction += (length - next) + sum;
            }
            sum = next;
            let cumulative = sum + correction;
            require_finite([cumulative], "polyline NURBS knots")?;
            knots.push(cumulative);
        }
        knots.push(*knots.last().expect("a polyline has at least one segment"));
        NurbsCurve::try_new(1, self.vertices.clone(), knots)
    }
}

fn direct_planar_area(relative: &[Vector3], normal: UnitVector3) -> Option<Real> {
    let mut sum = [0.0; 3];
    let mut correction = [0.0; 3];
    for vectors in relative.windows(2) {
        let cross = vectors[0].cross(vectors[1]).ok()?.to_array();
        for coordinate in 0..3 {
            let next = sum[coordinate] + cross[coordinate];
            if !next.is_finite() {
                return None;
            }
            if sum[coordinate].abs() >= cross[coordinate].abs() {
                correction[coordinate] += (sum[coordinate] - next) + cross[coordinate];
            } else {
                correction[coordinate] += (cross[coordinate] - next) + sum[coordinate];
            }
            sum[coordinate] = next;
        }
    }
    let normal = normal.as_vector().to_array();
    let twice_area = (0..3)
        .map(|coordinate| (sum[coordinate] + correction[coordinate]) * normal[coordinate])
        .sum::<Real>();
    twice_area.is_finite().then(|| 0.5 * twice_area.abs())
}

/// Joins every unambiguous connected component of open polylines.
///
/// Input order does not need to follow the curve chain, and individual curves
/// may be reversed. Endpoints within the absolute tolerance are represented by
/// their midpoint. Closed and disconnected inputs are returned as separate
/// components. A junction of three or more endpoints is rejected because it
/// has no unique single-polyline traversal.
pub fn join_polylines(
    polylines: &[Polyline3],
    tolerance: Tolerance,
) -> Result<Vec<JoinedPolyline3>, GeometryError> {
    if polylines.is_empty() {
        return Ok(Vec::new());
    }

    let mut endpoints = Vec::with_capacity(polylines.len().saturating_mul(2));
    let mut curve_endpoints = vec![None; polylines.len()];
    for (curve_index, polyline) in polylines.iter().enumerate() {
        if polyline.is_closed() {
            continue;
        }
        let start = endpoints.len();
        endpoints.push(JoinEndpoint {
            point: polyline.vertices[0],
            curve_index,
            is_start: true,
        });
        let end = endpoints.len();
        endpoints.push(JoinEndpoint {
            point: *polyline.vertices.last().expect("a polyline has vertices"),
            curve_index,
            is_start: false,
        });
        curve_endpoints[curve_index] = Some([start, end]);
    }

    let mut sets = DisjointSets::new(endpoints.len());
    union_near_endpoints(&endpoints, tolerance, &mut sets);
    let roots = (0..endpoints.len())
        .map(|endpoint| sets.root(endpoint))
        .collect::<Vec<_>>();
    let mut nodes = BTreeMap::<usize, JoinNode>::new();
    for (endpoint_index, root) in roots.iter().copied().enumerate() {
        nodes
            .entry(root)
            .or_default()
            .endpoint_indices
            .push(endpoint_index);
    }
    for node in nodes.values_mut() {
        if node.endpoint_indices.len() > 2 {
            return Err(GeometryError::AmbiguousPolylineJoin {
                endpoint_count: node.endpoint_indices.len(),
            });
        }
        node.representative = Some(join_node_representative(node, &endpoints)?);
    }

    let mut neighbors = vec![Vec::new(); polylines.len()];
    for node in nodes.values() {
        if let [left, right] = node.endpoint_indices.as_slice() {
            let left_curve = endpoints[*left].curve_index;
            let right_curve = endpoints[*right].curve_index;
            if left_curve != right_curve {
                neighbors[left_curve].push(right_curve);
                neighbors[right_curve].push(left_curve);
            }
        }
    }

    let mut visited = vec![false; polylines.len()];
    let mut results = Vec::new();
    for first_curve in 0..polylines.len() {
        if visited[first_curve] {
            continue;
        }
        if curve_endpoints[first_curve].is_none() {
            visited[first_curve] = true;
            results.push(JoinedPolyline3 {
                polyline: polylines[first_curve].clone(),
                source_indices: vec![first_curve],
            });
            continue;
        }

        let component = connected_component(first_curve, &neighbors, &visited);
        if component.len() == 1 {
            visited[first_curve] = true;
            results.push(JoinedPolyline3 {
                polyline: polylines[first_curve].clone(),
                source_indices: vec![first_curve],
            });
            continue;
        }
        let joined = join_component(
            polylines,
            &component,
            &endpoints,
            &curve_endpoints,
            &roots,
            &nodes,
            tolerance,
            &mut visited,
        )?;
        results.push(joined);
    }
    Ok(results)
}

#[derive(Clone, Copy)]
struct JoinEndpoint {
    point: Point3,
    curve_index: usize,
    is_start: bool,
}

#[derive(Default)]
struct JoinNode {
    endpoint_indices: Vec<usize>,
    representative: Option<Point3>,
}

impl JoinNode {
    fn point(&self) -> Point3 {
        self.representative
            .expect("join nodes receive a representative before traversal")
    }
}

fn join_node_representative(
    node: &JoinNode,
    endpoints: &[JoinEndpoint],
) -> Result<Point3, GeometryError> {
    let first = endpoints[node.endpoint_indices[0]].point;
    if node.endpoint_indices.len() == 1 {
        return Ok(first);
    }
    let second = endpoints[node.endpoint_indices[1]].point;
    first.translated(first.vector_to(second)?.scaled(0.5)?)
}

fn connected_component(first: usize, neighbors: &[Vec<usize>], visited: &[bool]) -> Vec<usize> {
    let mut component = Vec::new();
    let mut discovered = std::collections::BTreeSet::new();
    let mut pending = vec![first];
    discovered.insert(first);
    while let Some(curve) = pending.pop() {
        if visited[curve] {
            continue;
        }
        component.push(curve);
        for neighbor in neighbors[curve].iter().copied() {
            if discovered.insert(neighbor) {
                pending.push(neighbor);
            }
        }
    }
    component.sort_unstable();
    component
}

#[allow(clippy::too_many_arguments)]
fn join_component(
    polylines: &[Polyline3],
    component: &[usize],
    endpoints: &[JoinEndpoint],
    curve_endpoints: &[Option<[usize; 2]>],
    roots: &[usize],
    nodes: &BTreeMap<usize, JoinNode>,
    tolerance: Tolerance,
    visited: &mut [bool],
) -> Result<JoinedPolyline3, GeometryError> {
    let start_endpoint = component
        .iter()
        .flat_map(|curve| curve_endpoints[*curve].expect("component curves are open"))
        .filter(|endpoint| nodes[&roots[*endpoint]].endpoint_indices.len() == 1)
        .min_by_key(|endpoint| {
            let endpoint = endpoints[*endpoint];
            (endpoint.curve_index, !endpoint.is_start)
        })
        .unwrap_or_else(|| curve_endpoints[component[0]].expect("component curves are open")[0]);

    let mut entered_at = start_endpoint;
    let mut joined_vertices = Vec::new();
    let mut source_indices = Vec::with_capacity(component.len());
    loop {
        let endpoint = endpoints[entered_at];
        let curve_index = endpoint.curve_index;
        if visited[curve_index] {
            break;
        }
        visited[curve_index] = true;
        source_indices.push(curve_index);

        let [curve_start, curve_end] =
            curve_endpoints[curve_index].expect("component curves are open");
        let leaving_at = if endpoint.is_start {
            curve_end
        } else {
            curve_start
        };
        let start_root = roots[entered_at];
        let end_root = roots[leaving_at];
        let mut vertices = if endpoint.is_start {
            polylines[curve_index].vertices.clone()
        } else {
            polylines[curve_index]
                .vertices
                .iter()
                .rev()
                .copied()
                .collect()
        };
        vertices[0] = nodes[&start_root].point();
        *vertices.last_mut().expect("a polyline has vertices") = nodes[&end_root].point();
        if joined_vertices.is_empty() {
            joined_vertices.extend(vertices);
        } else {
            joined_vertices.extend(vertices.into_iter().skip(1));
        }

        let next = nodes[&end_root]
            .endpoint_indices
            .iter()
            .copied()
            .find(|candidate| {
                let curve = endpoints[*candidate].curve_index;
                curve != curve_index && !visited[curve]
            });
        let Some(next) = next else {
            break;
        };
        entered_at = next;
    }

    if source_indices.len() != component.len() {
        return Err(GeometryError::AmbiguousPolylineJoin {
            endpoint_count: component.len(),
        });
    }
    source_indices.sort_unstable();
    Ok(JoinedPolyline3 {
        polyline: Polyline3::try_new(joined_vertices, tolerance)?,
        source_indices,
    })
}

fn union_near_endpoints(endpoints: &[JoinEndpoint], tolerance: Tolerance, sets: &mut DisjointSets) {
    let cell_size = tolerance.absolute();
    let cells = endpoints
        .iter()
        .map(|endpoint| join_cell(endpoint.point, cell_size))
        .collect::<Option<Vec<_>>>();
    let Some(cells) = cells else {
        for right in 0..endpoints.len() {
            for left in 0..right {
                if endpoints[left]
                    .point
                    .is_near(endpoints[right].point, tolerance)
                {
                    sets.union(left, right);
                }
            }
        }
        return;
    };

    let mut grid = HashMap::<[i64; 3], Vec<usize>>::new();
    for (endpoint_index, cell) in cells.into_iter().enumerate() {
        for x_offset in -1_i64..=1 {
            for y_offset in -1_i64..=1 {
                for z_offset in -1_i64..=1 {
                    let Some(neighbor) = offset_cell(cell, [x_offset, y_offset, z_offset]) else {
                        continue;
                    };
                    if let Some(candidates) = grid.get(&neighbor) {
                        for candidate in candidates.iter().copied() {
                            if endpoints[candidate]
                                .point
                                .is_near(endpoints[endpoint_index].point, tolerance)
                            {
                                sets.union(candidate, endpoint_index);
                            }
                        }
                    }
                }
            }
        }
        grid.entry(cell).or_default().push(endpoint_index);
    }
}

fn join_cell(point: Point3, cell_size: Real) -> Option<[i64; 3]> {
    let mut cell = [0_i64; 3];
    for (target, coordinate) in cell.iter_mut().zip(point.to_array()) {
        let scaled = (coordinate / cell_size).floor();
        if !scaled.is_finite() || scaled < i64::MIN as Real || scaled > i64::MAX as Real {
            return None;
        }
        *target = scaled as i64;
    }
    Some(cell)
}

fn offset_cell(cell: [i64; 3], offset: [i64; 3]) -> Option<[i64; 3]> {
    Some([
        cell[0].checked_add(offset[0])?,
        cell[1].checked_add(offset[1])?,
        cell[2].checked_add(offset[2])?,
    ])
}

struct DisjointSets {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl DisjointSets {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count).collect(),
            rank: vec![0; count],
        }
    }

    fn root(&mut self, item: usize) -> usize {
        if self.parent[item] != item {
            self.parent[item] = self.root(self.parent[item]);
        }
        self.parent[item]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left = self.root(left);
        let right = self.root(right);
        if left == right {
            return;
        }
        match self.rank[left].cmp(&self.rank[right]) {
            std::cmp::Ordering::Less => self.parent[left] = right,
            std::cmp::Ordering::Greater => self.parent[right] = left,
            std::cmp::Ordering::Equal => {
                self.parent[right] = left;
                self.rank[left] += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vector3;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn validates_segments_and_closed_state() {
        assert_eq!(
            Polyline3::try_new(vec![point(0.0, 0.0, 0.0)], Tolerance::DEFAULT),
            Err(GeometryError::InsufficientPolylineVertices)
        );
        assert_eq!(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(1.0, 0.0, 0.0),
                    point(1.0, 0.0, 0.0),
                ],
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::DegeneratePolylineSegment { segment: 1 })
        );

        let closed = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(3.0, 4.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(closed.is_closed());
        assert_eq!(closed.segment_count(), 3);
    }

    #[test]
    fn computes_bounds_length_and_exact_linear_nurbs() {
        let polyline = Polyline3::try_new(
            vec![
                point(-2.0, 1.0, 3.0),
                point(1.0, 5.0, 3.0),
                point(1.0, 5.0, 15.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(polyline.length().unwrap(), 17.0);
        assert_eq!(polyline.bounds().min(), point(-2.0, 1.0, 3.0));
        assert_eq!(polyline.bounds().max(), point(1.0, 5.0, 15.0));

        let curve = polyline.to_nurbs().unwrap();
        assert_eq!(curve.degree(), 1);
        assert_eq!(curve.control_points().len(), 3);
        assert_eq!(curve.domain(), 0.0..=17.0);
        assert_eq!(curve.knots(), &[0.0, 0.0, 5.0, 17.0, 17.0]);
        assert_eq!(curve.evaluate(0.0).unwrap(), point(-2.0, 1.0, 3.0));
        assert_eq!(curve.evaluate(5.0).unwrap(), point(1.0, 5.0, 3.0));
        assert_eq!(curve.evaluate(17.0).unwrap(), point(1.0, 5.0, 15.0));

        let reversed = polyline.reversed();
        assert_eq!(
            reversed.vertices(),
            &[
                point(1.0, 5.0, 15.0),
                point(1.0, 5.0, 3.0),
                point(-2.0, 1.0, 3.0),
            ]
        );
        assert_eq!(reversed.reversed(), polyline);
    }

    #[test]
    fn closest_point_checks_every_segment_and_clamps_at_ends() {
        let polyline = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 1.0),
                point(4.0, 0.0, 1.0),
                point(4.0, 3.0, 1.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            polyline
                .closest_point(point(3.0, 2.0, 7.0), Tolerance::DEFAULT)
                .unwrap(),
            point(4.0, 2.0, 1.0)
        );
        assert_eq!(
            polyline
                .closest_point(point(-5.0, -2.0, 1.0), Tolerance::DEFAULT)
                .unwrap(),
            point(0.0, 0.0, 1.0)
        );
    }

    #[test]
    fn computes_rotated_planar_area_and_rejects_invalid_regions() {
        let rectangle = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(3.0, 0.0, 3.0),
                point(3.0, 4.0, 3.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(
            rectangle.planar_area(Tolerance::DEFAULT).unwrap(),
            12.0 * 2.0_f64.sqrt()
        ));

        let slender = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0e307, 0.0, 0.0),
                point(1.0e307, 2.0e-9, 0.0),
                point(0.0, 2.0e-9, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(
            slender.planar_area(Tolerance::DEFAULT).unwrap() / 1.0e298,
            2.0
        ));

        let open = Polyline3::try_new(
            vec![point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            open.planar_area(Tolerance::DEFAULT),
            Err(GeometryError::OpenPolylineArea)
        );
        let nonplanar = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 1.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            nonplanar.planar_area(Tolerance::DEFAULT),
            Err(GeometryError::NonPlanarPolyline)
        );
    }

    #[test]
    fn transform_revalidates_every_segment() {
        let polyline = Polyline3::try_new(
            vec![point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let moved = polyline
            .transformed(
                AffineTransform3::from_translation(Vector3::try_new(1.0, 2.0, 3.0).unwrap()),
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(
            moved.vertices(),
            &[point(1.0, 2.0, 3.0), point(3.0, 2.0, 3.0)]
        );

        let collapsed = AffineTransform3::try_new(
            [[0.0; 3], [0.0; 3], [0.0; 3]],
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        assert!(polyline.transformed(collapsed, Tolerance::DEFAULT).is_err());
    }

    #[test]
    fn regular_polygon_is_closed_oriented_and_bounded() {
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        let polygon = Polyline3::try_regular_polygon(
            6,
            point(1.0, 2.0, 3.0),
            point(5.0, 2.0, 8.0),
            normal,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(polygon.is_closed());
        assert_eq!(polygon.segment_count(), 6);
        assert_eq!(polygon.vertices()[0], point(5.0, 2.0, 3.0));
        assert!(polygon.vertices().iter().all(|vertex| vertex.z() == 3.0));
        for vertex in &polygon.vertices()[..6] {
            assert!(
                Tolerance::DEFAULT
                    .approx_eq(vertex.distance_to(point(1.0, 2.0, 3.0)).unwrap(), 4.0)
            );
        }
    }

    #[test]
    fn regular_polygon_rejects_side_and_tolerance_degeneracy() {
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        assert!(matches!(
            Polyline3::try_regular_polygon(
                2,
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                normal,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidRegularPolygonSides { actual: 2, .. })
        ));
        assert!(
            Polyline3::try_regular_polygon(
                3,
                point(0.0, 0.0, 0.0),
                point(1.0e-12, 0.0, 0.0),
                normal,
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }

    #[test]
    fn joins_shuffled_reversed_chains_and_retains_disconnected_curves() {
        let curves = vec![
            Polyline3::try_new(
                vec![point(1.0, 0.0, 0.0), point(2.0, 0.0, 0.0)],
                Tolerance::DEFAULT,
            )
            .unwrap(),
            Polyline3::try_new(
                vec![point(4.0, 0.0, 0.0), point(3.0, 0.0, 0.0)],
                Tolerance::DEFAULT,
            )
            .unwrap(),
            Polyline3::try_new(
                vec![point(3.0, 0.0, 0.0), point(2.0, 0.0, 0.0)],
                Tolerance::DEFAULT,
            )
            .unwrap(),
            Polyline3::try_new(
                vec![point(10.0, 0.0, 0.0), point(11.0, 0.0, 0.0)],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ];
        let joined = join_polylines(&curves, Tolerance::DEFAULT).unwrap();
        assert_eq!(joined.len(), 2);
        assert_eq!(joined[0].source_indices(), &[0, 1, 2]);
        assert_eq!(
            joined[0].polyline().vertices(),
            &[
                point(1.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
            ]
        );
        assert_eq!(joined[1].source_indices(), &[3]);
        assert_eq!(joined[1].polyline(), &curves[3]);
    }

    #[test]
    fn joins_closed_loops_and_reconciles_near_seams_once() {
        let tolerance = Tolerance::try_new(1.0e-6, 1.0e-12, 1.0e-10).unwrap();
        let curves = vec![
            Polyline3::try_new(vec![point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0)], tolerance)
                .unwrap(),
            Polyline3::try_new(vec![point(1.0, 2.0, 0.0), point(0.0, 0.0, 0.0)], tolerance)
                .unwrap(),
            Polyline3::try_new(
                vec![point(2.0 + 4.0e-7, 0.0, 0.0), point(1.0, 2.0, 0.0)],
                tolerance,
            )
            .unwrap(),
        ];
        let joined = join_polylines(&curves, tolerance).unwrap();
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].source_indices(), &[0, 1, 2]);
        assert!(joined[0].polyline().is_closed());
        assert_eq!(joined[0].polyline().segment_count(), 3);
        assert!(joined[0].polyline().vertices()[1].is_near(
            point(2.0 + 2.0e-7, 0.0, 0.0),
            Tolerance::try_new(1.0e-14, 1.0e-14, 1.0e-14).unwrap(),
        ));
    }

    #[test]
    fn joins_across_negative_spatial_hash_cell_boundaries() {
        let tolerance = Tolerance::try_new(1.0e-6, 1.0e-12, 1.0e-10).unwrap();
        let curves = vec![
            Polyline3::try_new(
                vec![point(-2.0, 0.0, 0.0), point(-2.5e-7, 0.0, 0.0)],
                tolerance,
            )
            .unwrap(),
            Polyline3::try_new(
                vec![point(2.5e-7, 0.0, 0.0), point(2.0, 0.0, 0.0)],
                tolerance,
            )
            .unwrap(),
        ];
        let joined = join_polylines(&curves, tolerance).unwrap();
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].polyline().vertices()[1], point(0.0, 0.0, 0.0));
    }

    #[test]
    fn rejects_branch_nodes_instead_of_choosing_an_arbitrary_path() {
        let curves = vec![
            Polyline3::try_new(
                vec![point(-1.0, 0.0, 0.0), point(0.0, 0.0, 0.0)],
                Tolerance::DEFAULT,
            )
            .unwrap(),
            Polyline3::try_new(
                vec![point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0)],
                Tolerance::DEFAULT,
            )
            .unwrap(),
            Polyline3::try_new(
                vec![point(0.0, 0.0, 0.0), point(0.0, 1.0, 0.0)],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ];
        assert_eq!(
            join_polylines(&curves, Tolerance::DEFAULT),
            Err(GeometryError::AmbiguousPolylineJoin { endpoint_count: 3 })
        );
    }

    #[test]
    fn join_handles_empty_closed_and_extreme_coordinate_inputs() {
        assert!(join_polylines(&[], Tolerance::DEFAULT).unwrap().is_empty());
        let closed = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let extreme_first = Polyline3::try_new(
            vec![point(1.0e200, 0.0, 0.0), point(1.0e200, 1.0e190, 0.0)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let extreme_second = Polyline3::try_new(
            vec![point(1.0e200, 1.0e190, 0.0), point(1.0e200, 2.0e190, 0.0)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let joined = join_polylines(
            &[
                closed.clone(),
                extreme_first.clone(),
                extreme_second.clone(),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(joined.len(), 2);
        assert_eq!(joined[0].polyline(), &closed);
        assert_eq!(joined[1].source_indices(), &[1, 2]);
        assert_eq!(joined[1].polyline().segment_count(), 2);
        assert_eq!(
            joined[1].polyline().vertices(),
            &[
                extreme_first.vertices()[0],
                extreme_first.vertices()[1],
                extreme_second.vertices()[1],
            ]
        );
    }
}
