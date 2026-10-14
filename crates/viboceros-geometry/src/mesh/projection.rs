//! Mesh projection candidates and exact closed-triangle point admission.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
use crate::interval::I;
use crate::{NurbsCurve, Vector3};
use num_traits::Zero;

#[cfg(test)]
mod tests;
const MAX_TRIANGLES: usize = 1_000_000;
#[derive(Debug)]
struct Node {
    bounds: BoundingBox3,
    range: Range<usize>,
    children: Option<[usize; 2]>,
}
/// Reusable bounding-volume index using the existing shorter-diagonal quad splitter.
/// Queries are bidirectional. Candidate rejection uses conservative interval
/// slabs with exact fallback; touching bounds are retained.
pub struct MeshProjectionIndex<'a> {
    mesh: &'a TriangleMesh,
    triangles: Vec<[u32; 3]>,
    bounds: Vec<BoundingBox3>,
    order: Vec<usize>,
    nodes: Vec<Node>,
}
impl<'a> MeshProjectionIndex<'a> {
    pub fn new(mesh: &'a TriangleMesh) -> Result<Self, GeometryError> {
        if mesh.triangles().len() > MAX_TRIANGLES {
            return Err(GeometryError::MeshProjectionLimit {
                maximum: MAX_TRIANGLES,
            });
        }
        let triangles = mesh.mass_triangle_indices().collect::<Vec<_>>();
        let bounds = triangles
            .iter()
            .map(|t| BoundingBox3::from_points(t.map(|i| mesh.vertices()[i as usize])))
            .collect::<Result<Vec<_>, _>>()?;
        let mut index = Self {
            mesh,
            triangles,
            bounds,
            order: (0..mesh.triangles().len()).collect(),
            nodes: Vec::new(),
        };
        if !index.order.is_empty() {
            index.build(0..index.order.len())?;
        }
        Ok(index)
    }
    /// Original triangle coordinates, including the chosen diagonal of a quad.
    pub fn triangle_points(&self, index: usize) -> Option<[Point3; 3]> {
        self.triangles
            .get(index)
            .map(|t| t.map(|i| self.mesh.vertices()[i as usize]))
    }
    fn build(&mut self, range: Range<usize>) -> Result<usize, GeometryError> {
        let bounds = self.order[range.clone()]
            .iter()
            .skip(1)
            .try_fold(self.bounds[self.order[range.start]], |b, &i| {
                b.union(self.bounds[i])
            })?;
        let key = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            range: range.clone(),
            children: None,
        });
        if range.len() > 8 {
            let min = bounds.min().to_array();
            let max = bounds.max().to_array();
            let axis = (0..3)
                .max_by(|&a, &b| (max[a] - min[a]).total_cmp(&(max[b] - min[b])))
                .unwrap();
            let mid = range.len() / 2;
            let boxes = &self.bounds;
            self.order[range.clone()].select_nth_unstable_by(mid, |&a, &b| {
                boxes[a].min().to_array()[axis]
                    .total_cmp(&boxes[b].min().to_array()[axis])
                    .then(a.cmp(&b))
            });
            let split = range.start + mid;
            let first = self.build(range.start..split)?;
            let second = self.build(split..range.end)?;
            self.nodes[key].children = Some([first, second]);
        }
        Ok(key)
    }
    /// Triangles whose bounds can meet a line through any point in `source`.
    /// A common-sign rational curve's control bounds are a valid source box.
    pub fn candidates(
        &self,
        source: BoundingBox3,
        direction: Vector3,
    ) -> Result<Vec<usize>, GeometryError> {
        self.query(source, direction, None)
    }
    /// Conservative curve candidates, additionally pruning a common sweep
    /// plane when all original controls lie in it exactly.
    pub fn curve_candidates(
        &self,
        curve: &NurbsCurve,
        direction: Vector3,
    ) -> Result<Vec<usize>, GeometryError> {
        let sign = curve.control_points()[0].weight().is_sign_positive();
        if curve
            .control_points()
            .iter()
            .any(|p| p.weight() == 0. || p.weight().is_sign_positive() != sign)
        {
            return Err(GeometryError::AmbiguousMeshProjection {
                context: "curve bounds require common-sign weights",
            });
        }
        let bounds = BoundingBox3::from_points(curve.control_points().iter().map(|p| p.point()))?;
        let plane = SweepPlane::new(curve, direction);
        self.query(bounds, direction, plane.as_ref())
    }
    fn query(
        &self,
        source: BoundingBox3,
        direction: Vector3,
        plane: Option<&SweepPlane>,
    ) -> Result<Vec<usize>, GeometryError> {
        let direction = direction.to_array();
        if direction.iter().all(|&d| d == 0.) {
            return Err(GeometryError::AmbiguousMeshProjection {
                context: "zero direction",
            });
        }
        let mut result = Vec::new();
        let mut stack = if self.nodes.is_empty() {
            vec![]
        } else {
            vec![0]
        };
        while let Some(key) = stack.pop() {
            let node = &self.nodes[key];
            if !may_meet(node.bounds, source, direction)
                || plane.is_some_and(|p| !p.meets(node.bounds))
            {
                continue;
            }
            if let Some(children) = node.children {
                stack.extend(children);
            } else {
                result.extend(self.order[node.range.clone()].iter().copied().filter(|&i| {
                    may_meet(self.bounds[i], source, direction)
                        && plane.is_none_or(|p| p.meets(self.bounds[i]))
                }));
            }
        }
        result.sort_unstable();
        Ok(result)
    }
    /// Project onto every intersected triangle, including shared edges/vertices.
    /// Exact rational parameters and barycentrics remove coincident face hits
    /// without merging nearby sheets. Only the final XYZ is rounded, and its
    /// Euclidean rounding error must fit the absolute model tolerance.
    pub fn project_point(
        &self,
        point: Point3,
        direction: Vector3,
        tolerance: Tolerance,
    ) -> Result<Vec<Point3>, GeometryError> {
        let candidates = self.candidates(BoundingBox3::from_points([point])?, direction)?;
        let mut hits = BTreeMap::<[Rational; 3], Rational>::new();
        for index in candidates {
            let triangle = self.triangle_points(index).expect("candidate triangle");
            if interval_miss(triangle, point, direction.to_array()) == Some(true) {
                continue;
            }
            if let Some((t, xyz)) = exact_hit(triangle, point, direction.to_array())? {
                hits.entry(xyz).or_insert(t);
            }
        }
        let mut hits = hits.into_iter().collect::<Vec<_>>();
        hits.sort_by(|a, b| a.1.cmp(&b.1));
        hits.into_iter()
            .map(|(xyz, _)| {
                let rounded =
                    Point3::try_new(scalar(&xyz[0])?, scalar(&xyz[1])?, scalar(&xyz[2])?)?;
                let error = (0..3)
                    .map(|i| {
                        let d = rational(rounded.to_array()[i]) - &xyz[i];
                        &d * &d
                    })
                    .sum::<Rational>();
                let limit = rational(tolerance.absolute());
                if error > &limit * &limit {
                    return Err(GeometryError::UnrepresentableMeshProjection);
                }
                Ok(rounded)
            })
            .collect()
    }
}
fn may_meet(target: BoundingBox3, source: BoundingBox3, d: [Real; 3]) -> bool {
    interval_slabs(target, source, d).unwrap_or_else(|| exact_slabs(target, source, d))
}
fn interval_slabs(target: BoundingBox3, source: BoundingBox3, d: [Real; 3]) -> Option<bool> {
    let lo = target.min().to_array();
    let hi = target.max().to_array();
    let s = source.min().to_array();
    let e = source.max().to_array();
    let mut lower = Real::NEG_INFINITY;
    let mut upper = Real::INFINITY;
    for i in 0..3 {
        // Minkowski difference: target - source. The line passes through zero.
        let a = I::point(lo[i])?.sub(I::point(e[i])?)?;
        let b = I::point(hi[i])?.sub(I::point(s[i])?)?;
        if d[i] == 0. {
            if a.lo > 0. || b.hi < 0. {
                return Some(false);
            }
        } else {
            let a = a.div(I::point(d[i])?)?;
            let b = b.div(I::point(d[i])?)?;
            lower = lower.max(a.lo.min(b.lo));
            upper = upper.min(a.hi.max(b.hi));
            if lower > upper {
                return Some(false);
            }
        }
    }
    Some(true)
}
fn exact_slabs(target: BoundingBox3, source: BoundingBox3, d: [Real; 3]) -> bool {
    let lo = target.min().to_array();
    let hi = target.max().to_array();
    let s = source.min().to_array();
    let e = source.max().to_array();
    let mut lower: Option<Rational> = None;
    let mut upper: Option<Rational> = None;
    for i in 0..3 {
        let a = rational(lo[i]) - rational(e[i]);
        let b = rational(hi[i]) - rational(s[i]);
        if d[i] == 0. {
            if a > Rational::zero() || b < Rational::zero() {
                return false;
            }
        } else {
            let a = a / rational(d[i]);
            let b = b / rational(d[i]);
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            lower = Some(lower.map_or(a.clone(), |l| l.max(a)));
            upper = Some(upper.map_or(b.clone(), |u| u.min(b)));
            if lower.as_ref().unwrap() > upper.as_ref().unwrap() {
                return false;
            }
        }
    }
    true
}
type E = [Rational; 3];
fn sub(a: &E, b: &E) -> E {
    std::array::from_fn(|i| &a[i] - &b[i])
}
fn cross(a: &E, b: &E) -> E {
    std::array::from_fn(|i| {
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        &a[j] * &b[k] - &a[k] * &b[j]
    })
}
fn dot(a: &E, b: &E) -> Rational {
    (0..3).map(|i| &a[i] * &b[i]).sum()
}
fn exact_hit(
    [a, b, c]: [Point3; 3],
    point: Point3,
    d: [Real; 3],
) -> Result<Option<(Rational, E)>, GeometryError> {
    let [a, b, c, p] = [a, b, c, point].map(|p| p.to_array().map(rational));
    let d = d.map(rational);
    let ab = sub(&b, &a);
    let ac = sub(&c, &a);
    let s = sub(&p, &a);
    let h = cross(&d, &ac);
    let determinant = dot(&ab, &h);
    if determinant.is_zero() {
        let normal = cross(&ab, &ac);
        if normal.iter().all(Zero::is_zero) {
            return Err(GeometryError::AmbiguousMeshProjection {
                context: "collapsed triangle",
            });
        }
        if dot(&normal, &s).is_zero() {
            return coplanar_hit(&ab, &ac, &s, &a, &d);
        }
        return Ok(None);
    }
    let u = dot(&s, &h) / &determinant;
    let q = cross(&s, &ab);
    let v = dot(&d, &q) / &determinant;
    if u < Rational::zero() || v < Rational::zero() || &u + &v > rational(1.) {
        return Ok(None);
    }
    let t = dot(&ac, &q) / determinant;
    let xyz = std::array::from_fn(|i| &a[i] + &ab[i] * &u + &ac[i] * &v);
    Ok(Some((t, xyz)))
}
fn interval_miss(triangle: [Point3; 3], point: Point3, d: [Real; 3]) -> Option<bool> {
    let make = |p: [Real; 3]| Some([I::point(p[0])?, I::point(p[1])?, I::point(p[2])?]);
    let [a, b, c] = triangle.map(Point3::to_array);
    let (a, b, c, p, d) = (
        make(a)?,
        make(b)?,
        make(c)?,
        make(point.to_array())?,
        make(d)?,
    );
    let diff = |a: [I; 3], b: [I; 3]| Some([a[0].sub(b[0])?, a[1].sub(b[1])?, a[2].sub(b[2])?]);
    let cross = |a: [I; 3], b: [I; 3]| {
        Some([
            a[1].mul(b[2])?.sub(a[2].mul(b[1])?)?,
            a[2].mul(b[0])?.sub(a[0].mul(b[2])?)?,
            a[0].mul(b[1])?.sub(a[1].mul(b[0])?)?,
        ])
    };
    let dot = |a: [I; 3], b: [I; 3]| a[0].mul(b[0])?.add(a[1].mul(b[1])?)?.add(a[2].mul(b[2])?);
    let (ab, ac, s) = (diff(b, a)?, diff(c, a)?, diff(p, a)?);
    let h = cross(d, ac)?;
    let det = dot(ab, h)?;
    let u = dot(s, h)?.div(det)?;
    let q = cross(s, ab)?;
    let v = dot(d, q)?.div(det)?;
    let w = I::point(1.)?.sub(u)?.sub(v)?;
    Some(u.hi < 0. || v.hi < 0. || w.hi < 0.)
}

struct SweepPlane {
    origin: E,
    normal: E,
}
impl SweepPlane {
    fn new(curve: &NurbsCurve, direction: Vector3) -> Option<Self> {
        let origin = curve.control_points()[0].point().to_array().map(rational);
        let direction = direction.to_array().map(rational);
        let normal = curve.control_points().iter().skip(1).find_map(|p| {
            let p = p.point().to_array().map(rational);
            let n = cross(&sub(&p, &origin), &direction);
            (!n.iter().all(Zero::is_zero)).then_some(n)
        })?;
        curve
            .control_points()
            .iter()
            .all(|p| dot(&normal, &sub(&p.point().to_array().map(rational), &origin)).is_zero())
            .then_some(Self { origin, normal })
    }
    fn meets(&self, bounds: BoundingBox3) -> bool {
        self.interval(bounds).unwrap_or_else(|| {
            let lo = bounds.min().to_array().map(rational);
            let hi = bounds.max().to_array().map(rational);
            let minimum = (0..3)
                .map(|i| {
                    let p = if self.normal[i] < Rational::zero() {
                        &hi[i]
                    } else {
                        &lo[i]
                    };
                    &self.normal[i] * (p - &self.origin[i])
                })
                .sum::<Rational>();
            let maximum = (0..3)
                .map(|i| {
                    let p = if self.normal[i] < Rational::zero() {
                        &lo[i]
                    } else {
                        &hi[i]
                    };
                    &self.normal[i] * (p - &self.origin[i])
                })
                .sum::<Rational>();
            minimum <= Rational::zero() && maximum >= Rational::zero()
        })
    }
    fn interval(&self, bounds: BoundingBox3) -> Option<bool> {
        let lo = bounds.min().to_array();
        let hi = bounds.max().to_array();
        let mut minimum = I::point(0.)?;
        let mut maximum = minimum;
        for i in 0..3 {
            let n = I::exact(&self.normal[i])?;
            let origin = I::exact(&self.origin[i])?;
            let a = I::point(lo[i])?.sub(origin)?;
            let b = I::point(hi[i])?.sub(origin)?;
            let (a, b) = if self.normal[i] < Rational::zero() {
                (b, a)
            } else {
                (a, b)
            };
            minimum = minimum.add(n.mul(a)?)?;
            maximum = maximum.add(n.mul(b)?)?;
        }
        Some(minimum.lo <= 0. && maximum.hi >= 0.)
    }
}

fn coplanar_hit(
    ab: &E,
    ac: &E,
    s: &E,
    a: &E,
    d: &E,
) -> Result<Option<(Rational, E)>, GeometryError> {
    let aa = dot(ab, ab);
    let cc = dot(ac, ac);
    let bc = dot(ab, ac);
    let gram = &aa * &cc - &bc * &bc;
    let u = dot(s, ab) * &cc - dot(s, ac) * &bc;
    let v = dot(s, ac) * &aa - dot(s, ab) * &bc;
    let du = dot(d, ab) * &cc - dot(d, ac) * &bc;
    let dv = dot(d, ac) * &aa - dot(d, ab) * &bc;
    let mut low: Option<Rational> = None;
    let mut high: Option<Rational> = None;
    for (base, slope) in [
        (u.clone(), du.clone()),
        (v.clone(), dv.clone()),
        (gram - u - v, -du - dv),
    ] {
        if slope.is_zero() {
            if base < Rational::zero() {
                return Ok(None);
            }
            continue;
        }
        let parameter = -base / &slope;
        if slope > Rational::zero() {
            low = Some(low.map_or(parameter.clone(), |p| p.max(parameter)));
        } else {
            high = Some(high.map_or(parameter.clone(), |p| p.min(parameter)));
        }
    }
    if let (Some(low), Some(high)) = (&low, &high) {
        if low > high {
            return Ok(None);
        }
        if low == high {
            return Ok(Some((
                low.clone(),
                std::array::from_fn(|i| &a[i] + &s[i] + &d[i] * low),
            )));
        }
    }
    Err(GeometryError::AmbiguousMeshProjection {
        context: "line overlaps a triangle region",
    })
}

impl MeshProjectionIndex<'_> {
    /// Exact coplanar candidate regions share a single trim boundary. This
    /// avoids creating artificial gaps at an internal triangulation diagonal.
    /// Existing n-gon discovery and planar face construction are reused; every
    /// proposed planar group is independently checked with exact determinants.
    pub fn curve_faces(
        &self,
        curve: &NurbsCurve,
        direction: Vector3,
        tolerance: Tolerance,
    ) -> Result<Vec<crate::Brep>, GeometryError> {
        let candidates = self.curve_candidates(curve, direction)?;
        if candidates.is_empty() {
            return Ok(vec![]);
        }
        let mut map = BTreeMap::<u32, u32>::new();
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for index in candidates {
            let mut triangle = [0; 3];
            for (j, raw) in self.triangles[index].iter().copied().enumerate() {
                triangle[j] = *map.entry(raw).or_insert_with(|| {
                    let key = vertices.len() as u32;
                    vertices.push(self.mesh.vertices()[raw as usize]);
                    key
                });
            }
            triangles.push(triangle);
        }
        let selected = TriangleMesh::try_new(vertices, triangles, tolerance)?;
        let (selected, _) = selected.add_planar_ngons(0.)?;
        let mut covered = vec![false; selected.face_count()];
        let mut faces = Vec::new();
        for ngon in selected.ngons() {
            let seed = selected.faces()[ngon.faces()[0] as usize].indices();
            let [a, b, c] = [seed[0], seed[1], seed[2]]
                .map(|i| selected.vertices()[i as usize].to_array().map(rational));
            let normal = cross(&sub(&b, &a), &sub(&c, &a));
            let exact = ngon.faces().iter().all(|&i| {
                selected.faces()[i as usize].indices().iter().all(|&v| {
                    dot(
                        &normal,
                        &sub(
                            &selected.vertices()[v as usize].to_array().map(rational),
                            &a,
                        ),
                    )
                    .is_zero()
                })
            });
            if !exact {
                continue;
            }
            let mut border = ngon
                .vertices()
                .iter()
                .map(|&i| selected.vertices()[i as usize])
                .collect::<Vec<_>>();
            border.push(border[0]);
            let curve = Polyline3::try_new(border, tolerance)?.to_nurbs()?;
            faces.push(crate::Brep::try_planar_face(&curve, tolerance)?);
            for &i in ngon.faces() {
                covered[i as usize] = true;
            }
        }
        for (i, &triangle) in selected.triangles().iter().enumerate() {
            if covered[i] {
                continue;
            }
            let points = triangle.map(|j| selected.vertices()[j as usize]);
            let triangle = TriangleMesh::try_new(points.to_vec(), vec![[0, 1, 2]], tolerance)?;
            faces.push(crate::Brep::try_from_mesh(&triangle, true, tolerance)?);
        }
        Ok(faces)
    }
}
