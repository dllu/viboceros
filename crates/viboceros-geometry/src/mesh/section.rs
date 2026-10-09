//! Exact dyadic triangle/plane intersections with shared topological endpoints.
use super::*;
use crate::{Frame3, Polyline3};
use num_rational::BigRational;
use num_traits::{ToPrimitive, Zero};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    Vertex(u32),
    Edge(u32, u32),
}
fn rational(value: f64) -> BigRational {
    BigRational::from_float(value).expect("validated finite coordinate")
}
fn edge(a: u32, b: u32) -> Node {
    Node::Edge(a.min(b), a.max(b))
}
impl TriangleMesh {
    pub fn section_with_plane(
        &self,
        frame: Frame3,
        validation: Tolerance,
    ) -> Result<Vec<Polyline3>, GeometryError> {
        let origin = frame.origin().to_array().map(rational);
        let normal = frame.z_axis().as_vector().to_array().map(rational);
        let distances = self
            .vertices()
            .iter()
            .map(|p| {
                p.to_array()
                    .map(rational)
                    .into_iter()
                    .zip(&origin)
                    .zip(&normal)
                    .map(|((p, o), n)| (p - o) * n)
                    .sum::<BigRational>()
            })
            .collect::<Vec<_>>();
        let mut points = BTreeMap::<Node, Point3>::new();
        let mut segments = BTreeSet::<(Node, Node)>::new();
        let mut directed = BTreeMap::<(Node, Node), (Node, Node)>::new();
        let mut coplanar = BTreeMap::<(u32, u32), usize>::new();
        for face in self.triangles() {
            let vertices = face.map(|i| i as usize);
            if vertices.iter().all(|i| distances[*i].is_zero()) {
                for (a, b) in [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])] {
                    *coplanar.entry((a.min(b), a.max(b))).or_default() += 1;
                }
                continue;
            }
            let mut hits = BTreeSet::new();
            for (a, b) in [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])] {
                let da = &distances[a as usize];
                let db = &distances[b as usize];
                if da.is_zero() {
                    hits.insert(Node::Vertex(a));
                    points.insert(Node::Vertex(a), self.vertices()[a as usize]);
                }
                if db.is_zero() {
                    hits.insert(Node::Vertex(b));
                    points.insert(Node::Vertex(b), self.vertices()[b as usize]);
                }
                if !da.is_zero()
                    && !db.is_zero()
                    && (da < &BigRational::zero()) != (db < &BigRational::zero())
                {
                    let key = edge(a, b);
                    hits.insert(key);
                    if let std::collections::btree_map::Entry::Vacant(entry) = points.entry(key) {
                        let t = da / (da - db);
                        let p = self.vertices()[a as usize].to_array().map(rational);
                        let q = self.vertices()[b as usize].to_array().map(rational);
                        let xyz = std::array::from_fn::<_, 3, _>(|i| {
                            (&p[i] + (&q[i] - &p[i]) * &t).to_f64()
                        });
                        let coordinate = |i: usize| {
                            xyz[i].ok_or(GeometryError::NonFinite {
                                context: "mesh section",
                            })
                        };
                        entry.insert(Point3::try_new(
                            coordinate(0)?,
                            coordinate(1)?,
                            coordinate(2)?,
                        )?);
                    }
                }
            }
            if hits.len() == 2 {
                let nodes = hits.into_iter().collect::<Vec<_>>();
                segments.insert((nodes[0], nodes[1]));
                let a = self.vertices()[face[0] as usize];
                let b = self.vertices()[face[1] as usize];
                let c = self.vertices()[face[2] as usize];
                let normal = a.vector_to(b)?.cross(a.vector_to(c)?)?;
                let direction = frame.z_axis().as_vector().cross(normal)?;
                let forward = points[&nodes[0]]
                    .vector_to(points[&nodes[1]])?
                    .dot(direction)?
                    >= 0.;
                directed.insert(
                    (nodes[0], nodes[1]),
                    if forward {
                        (nodes[0], nodes[1])
                    } else {
                        (nodes[1], nodes[0])
                    },
                );
            }
        }
        for ((a, b), count) in coplanar {
            if count == 1 {
                let akey = Node::Vertex(a);
                let bkey = Node::Vertex(b);
                points.insert(akey, self.vertices()[a as usize]);
                points.insert(bkey, self.vertices()[b as usize]);
                segments.insert((akey.min(bkey), akey.max(bkey)));
            }
        }
        let mut adjacency = BTreeMap::<Node, Vec<Node>>::new();
        for (a, b) in &segments {
            adjacency.entry(*a).or_default().push(*b);
            adjacency.entry(*b).or_default().push(*a);
        }
        let mut result = Vec::new();
        while !segments.is_empty() {
            let start = adjacency
                .iter()
                .find(|(node, neighbors)| {
                    neighbors.len() != 2
                        && neighbors.iter().any(|other| {
                            let pair = ((**node).min(*other), (**node).max(*other));
                            segments.contains(&pair)
                                && directed.get(&pair).is_none_or(|(from, _)| from == *node)
                        })
                })
                .map(|(node, _)| *node)
                .unwrap_or(segments.first().unwrap().0);
            let mut nodes = vec![start];
            let mut current = start;
            loop {
                let next = adjacency[&current]
                    .iter()
                    .copied()
                    .find(|other| segments.contains(&(current.min(*other), current.max(*other))));
                let Some(next) = next else { break };
                segments.remove(&(current.min(next), current.max(next)));
                nodes.push(next);
                current = next;
                if current == start || adjacency[&current].len() != 2 {
                    break;
                }
            }
            if nodes.len() >= 2 {
                result.push(Polyline3::try_new(
                    nodes.into_iter().map(|node| points[&node]).collect(),
                    validation,
                )?);
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
