//! Conservative spatial queries on original finite coordinates.
//! Pruning uses rounded subtraction, avoiding translated grids and inflated bounds.
use crate::{Point3, Real};
const LEAF_SIZE: usize = 8;
#[derive(Clone, Copy, Debug)]
struct Node {
    minimum: [Real; 3],
    maximum: [Real; 3],
    start: usize,
    end: usize,
    children: Option<[usize; 2]>,
}
#[derive(Clone, Debug)]
pub(crate) struct PointTree {
    points: Vec<Point3>,
    indices: Vec<usize>,
    nodes: Vec<Node>,
}
impl PointTree {
    pub(crate) fn new(points: Vec<Point3>) -> Self {
        let mut tree = Self {
            indices: (0..points.len()).collect(),
            nodes: Vec::new(),
            points,
        };
        if !tree.points.is_empty() {
            build(&tree.points, &mut tree.indices, 0, &mut tree.nodes);
        }
        tree
    }
    pub(crate) fn visit_pairs<E>(
        &self,
        radius: Real,
        check: &mut impl FnMut() -> Result<(), E>,
        visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        if self.nodes.is_empty() {
            return Ok(());
        }
        self.pairs(0, 0, radius, check, visit)
    }
    fn pairs<E>(
        &self,
        left: usize,
        right: usize,
        radius: Real,
        check: &mut impl FnMut() -> Result<(), E>,
        visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        check()?;
        let a = self.nodes[left];
        let b = self.nodes[right];
        if left != right && separated(a, b, radius) {
            return Ok(());
        }
        if left == right {
            if let Some([low, high]) = a.children {
                self.pairs(low, low, radius, check, visit)?;
                self.pairs(low, high, radius, check, visit)?;
                self.pairs(high, high, radius, check, visit)?;
            } else {
                for i in a.start..a.end {
                    for j in a.start..i {
                        visit(self.indices[j], self.indices[i])?;
                    }
                }
            }
        } else if a.children.is_some()
            && (b.children.is_none() || a.end - a.start >= b.end - b.start)
        {
            let [low, high] = a.children.unwrap();
            self.pairs(low, right, radius, check, visit)?;
            self.pairs(high, right, radius, check, visit)?;
        } else if let Some([low, high]) = b.children {
            self.pairs(left, low, radius, check, visit)?;
            self.pairs(left, high, radius, check, visit)?;
        } else {
            for i in a.start..a.end {
                for j in b.start..b.end {
                    visit(self.indices[i], self.indices[j])?;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn visit_near<E>(
        &self,
        point: Point3,
        radius: Real,
        check: &mut impl FnMut() -> Result<(), E>,
        visit: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        if self.nodes.is_empty() {
            return Ok(());
        }
        self.near(0, point, radius, check, visit)
    }
    fn near<E>(
        &self,
        node: usize,
        point: Point3,
        radius: Real,
        check: &mut impl FnMut() -> Result<(), E>,
        visit: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        check()?;
        let a = self.nodes[node];
        let p = point.to_array();
        if (0..3).any(|i| a.minimum[i] - p[i] > radius || p[i] - a.maximum[i] > radius) {
            return Ok(());
        }
        if let Some([low, high]) = a.children {
            self.near(low, point, radius, check, visit)?;
            self.near(high, point, radius, check, visit)?;
        } else {
            for i in a.start..a.end {
                visit(self.indices[i])?;
            }
        }
        Ok(())
    }
}
fn separated(a: Node, b: Node, radius: Real) -> bool {
    (0..3).any(|i| a.minimum[i] - b.maximum[i] > radius || b.minimum[i] - a.maximum[i] > radius)
}
fn build(points: &[Point3], indices: &mut [usize], start: usize, nodes: &mut Vec<Node>) -> usize {
    let mut minimum = [Real::INFINITY; 3];
    let mut maximum = [Real::NEG_INFINITY; 3];
    for &index in indices.iter() {
        let p = points[index].to_array();
        for i in 0..3 {
            minimum[i] = minimum[i].min(p[i]);
            maximum[i] = maximum[i].max(p[i]);
        }
    }
    let root = nodes.len();
    nodes.push(Node {
        minimum,
        maximum,
        start,
        end: start + indices.len(),
        children: None,
    });
    if indices.len() > LEAF_SIZE {
        let axis = (0..3)
            .max_by(|&a, &b| (maximum[a] - minimum[a]).total_cmp(&(maximum[b] - minimum[b])))
            .unwrap();
        let middle = indices.len() / 2;
        indices.select_nth_unstable_by(middle, |&a, &b| {
            points[a].to_array()[axis]
                .total_cmp(&points[b].to_array()[axis])
                .then_with(|| a.cmp(&b))
        });
        let (low, high) = indices.split_at_mut(middle);
        nodes[root].children = Some([
            build(points, low, start, nodes),
            build(points, high, start + middle, nodes),
        ]);
    }
    root
}

#[cfg(test)]
mod tests;
