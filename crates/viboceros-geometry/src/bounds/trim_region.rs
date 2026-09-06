//! Conservative even/odd region tests using rational Bezier hulls. No fixed
//! tessellation, model-space tolerance, or merged scan-line roots are used.
use super::{
    bezier::{self, Budget, Net},
    parameter_curves,
};
use crate::{BoundingBox3, BrepFace, GeometryError, Point3, WeightedPoint3};

pub(super) type Rect = [[f64; 2]; 2];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Location {
    Inside,
    Outside,
    Uncertain,
}

struct Node {
    net: Net,
    hull: Option<BoundingBox3>,
    ends: [Point3; 2],
    children: Option<[usize; 2]>,
    input_scale: f64,
}

impl Node {
    fn new(net: Net) -> Result<Self, GeometryError> {
        let ends = [
            net.project(net.controls[0])?,
            net.project(*net.controls.last().unwrap())?,
        ];
        Ok(Self {
            hull: net.hull(),
            input_scale: net
                .controls
                .iter()
                .flat_map(|h| &h[..3])
                .map(|x| x.abs())
                .fold(1., f64::max),
            net,
            ends,
            children: None,
        })
    }

    fn padding(&self) -> f64 {
        // Guard accumulated extraction, projection, and subdivision roundoff
        // in normalized UV. This is deliberately independent of trim/model
        // tolerances: a narrow hole must not disappear in a loose model.
        let denominator = self
            .net
            .controls
            .iter()
            .map(|h| h[3].abs())
            .fold(f64::INFINITY, f64::min);
        64. * f64::EPSILON
            * ((self.net.degrees[0] + 1) as f64).powi(2)
            * (1. + f64::from(self.net.depth))
            * (self.input_scale / denominator).max(1.)
    }
}

pub(super) struct Region {
    nodes: Vec<Node>,
    loops: Vec<Vec<usize>>,
}

impl Region {
    pub(super) fn new(
        face: &BrepFace,
        domains: Rect,
        budget: &mut Budget,
    ) -> Result<Self, GeometryError> {
        let mut result = Self {
            nodes: Vec::new(),
            loops: Vec::new(),
        };
        for boundary in face.loops() {
            let trims = boundary.trims();
            let mut roots = Vec::new();
            for (index, trim) in trims.iter().enumerate() {
                let curve = trim.curve();
                let next = trims[(index + 1) % trims.len()].curve();
                // A tolerance-closed wire is not an exact closed UV region.
                // Do not silently add geometry to close an imported gap.
                if curve.evaluate(*curve.domain().end())?
                    != next.evaluate(*next.domain().start())?
                {
                    return Err(open_loop());
                }
                let p = curve.degree();
                for k in p + 1..curve.control_points().len() {
                    if curve.knots()[k] != curve.knots()[k - 1]
                        && curve.knots()[k..=k + p]
                            .iter()
                            .all(|t| *t == curve.knots()[k])
                        && curve.control_points()[k - 1].point()
                            != curve.control_points()[k].point()
                    {
                        return Err(open_loop());
                    }
                }
                for net in parameter_curves::spans(curve, domains, budget)? {
                    roots.push(result.push(net)?);
                }
            }
            // Adjacent extracted spans represent the same mathematical
            // endpoint, but can project to slightly different floating-point
            // values. Include their tiny connecting uncertainty boxes in the
            // classifier; never discard a query intersecting one of them.
            let mut joints = Vec::new();
            for i in 0..roots.len() {
                let a = &result.nodes[roots[i]];
                let b = &result.nodes[roots[(i + 1) % roots.len()]];
                let [start, end] = [a.ends[1], b.ends[0]];
                if start != end {
                    if (0..2).any(|axis| {
                        (start.to_array()[axis] - end.to_array()[axis]).abs()
                            > a.padding() + b.padding()
                    }) {
                        return Err(GeometryError::BoundingBoxDidNotConverge);
                    }
                    budget.initial(2)?;
                    joints.push(Net::new(
                        [1, 0],
                        &[
                            WeightedPoint3::try_new(start, 1.)?,
                            WeightedPoint3::try_new(end, 1.)?,
                        ],
                    )?);
                }
            }
            for joint in joints {
                roots.push(result.push(joint)?);
            }
            result.loops.push(roots);
        }
        Ok(result)
    }

    fn push(&mut self, net: Net) -> Result<usize, GeometryError> {
        let index = self.nodes.len();
        self.nodes.push(Node::new(net)?);
        Ok(index)
    }

    pub(super) fn locate(
        &mut self,
        rect: Rect,
        budget: &mut Budget,
    ) -> Result<Location, GeometryError> {
        let mut uncertain = false;
        for index in 0..self.loops.len() {
            let location = self.loop_location(index, rect, budget)?;
            if (index == 0 && location == Location::Outside)
                || (index != 0 && location == Location::Inside)
            {
                return Ok(Location::Outside);
            }
            uncertain |= location == Location::Uncertain;
        }
        Ok(if uncertain {
            Location::Uncertain
        } else {
            Location::Inside
        })
    }

    fn loop_location(
        &mut self,
        index: usize,
        rect: Rect,
        budget: &mut Budget,
    ) -> Result<Location, GeometryError> {
        let y = rect[1][0] * 0.5 + rect[1][1] * 0.5;
        let mut pending = self.loops[index].clone();
        let mut parity = false;
        let mut visited = 0;
        let mut refinements = 0;
        while let Some(index) = pending.pop() {
            budget.visit()?;
            visited += 1;
            let node = &self.nodes[index];
            budget.charge(node.net.controls.len())?;
            if let Some(hull) = node.hull {
                let low = hull.min().to_array();
                let high = hull.max().to_array();
                let pad = node.padding();
                if high[0] + pad < rect[0][0]
                    || high[1] + pad < rect[1][0]
                    || low[1] - pad > rect[1][1]
                {
                    continue;
                }
                if low[0] - pad > rect[0][1] {
                    // The entire hull is to the right of the query. Replacing
                    // the curve by its chord cannot cross the query rectangle;
                    // its ray-crossing parity depends only on the endpoint Y's.
                    parity ^= (node.ends[0].y() > y) != (node.ends[1].y() > y);
                    continue;
                }
            }
            if visited >= 256
                || node.net.depth == bezier::MAX_DEPTH
                || node
                    .ends
                    .iter()
                    .any(|p| (0..2).all(|i| (rect[i][0]..=rect[i][1]).contains(&p.to_array()[i])))
            {
                return Ok(Location::Uncertain);
            }
            let children = if let Some(children) = node.children {
                children
            } else {
                if refinements == 12 {
                    return Ok(Location::Uncertain);
                }
                refinements += 1;
                budget.charge(node.net.controls.len().saturating_pow(2))?;
                let input_scale = node.input_scale;
                let (a, b) = node.net.clone().split(0);
                let children = [self.push(a)?, self.push(b)?];
                for child in children {
                    self.nodes[child].input_scale = input_scale;
                }
                self.nodes[index].children = Some(children);
                children
            };
            pending.push(children[1]);
            pending.push(children[0]);
        }
        Ok(if parity {
            Location::Inside
        } else {
            Location::Outside
        })
    }
}

fn open_loop() -> GeometryError {
    GeometryError::InvalidBrepTopology {
        context: "tight bounds require closed, continuous parameter-space loops",
    }
}

#[cfg(test)]
mod tests;
