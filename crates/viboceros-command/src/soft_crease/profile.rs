//! Directed weight profiles retain discontinuities and split interpolation at bends.
use viboceros_geometry::{GeometryError, SubD, SubDEdgeChain};

#[derive(Clone, Debug)]
pub(super) struct Profile {
    pub topology: SubDEdgeChain,
    weights: Vec<[f64; 2]>,
    hard: Vec<bool>,
}

impl Profile {
    pub fn new(source: &SubD, topology: SubDEdgeChain) -> Self {
        let weights = topology
            .edges
            .iter()
            .map(|e| {
                let mut w = source.edge_sharpness()[e.index];
                if e.reversed {
                    w.swap(0, 1);
                }
                w
            })
            .collect();
        let hard = topology
            .edges
            .iter()
            .map(|e| source.edge_tags()[e.index] == 2)
            .collect();
        Self {
            topology,
            weights,
            hard,
        }
    }

    pub fn constant(&mut self, percent: f64) -> Result<(), GeometryError> {
        // The constant getter uses the slider factory directly, rather than
        // the chain interpolation factory (which has different snapping).
        let sharpness = SubD::soft_crease_sharpness(percent)?;
        let stored = sharpness as f32 as f64;
        self.weights.fill([stored; 2]);
        self.hard.fill(false);
        Ok(())
    }

    pub fn crease(&mut self) {
        self.hard.fill(true);
    }

    /// Virtualize a hard run between its adjacent soft endpoints. An open end
    /// contributes 100; a completely hard closed loop is uniformly 100.
    fn virtual_weights(&self) -> Vec<[f64; 2]> {
        let n = self.weights.len();
        let mut weights = self.weights.clone();
        if self.hard.iter().all(|&h| h) {
            weights.fill([4.; 2]);
            return weights;
        }
        let closed = self.topology.is_closed();
        // Start immediately after a soft edge to keep a wrapping hard run whole.
        let first = if closed {
            (self.hard.iter().position(|&h| !h).unwrap() + 1) % n
        } else {
            0
        };
        let mut offset = 0;
        while offset < n {
            let start = (first + offset) % n;
            if !self.hard[start] {
                offset += 1;
                continue;
            }
            let mut count = 1;
            while offset + count < n && self.hard[(start + count) % n] {
                count += 1;
            }
            let end = (start + count) % n;
            let a = if !closed && start == 0 {
                4.
            } else {
                weights[(start + n - 1) % n][1]
            };
            let b = if !closed && start + count == n {
                4.
            } else {
                weights[end][0]
            };
            for k in 0..count {
                let at = |i: usize| a + (b - a) * i as f64 / count as f64;
                weights[(start + k) % n] = [at(k), at(k + 1)];
            }
            offset += count;
        }
        weights
    }

    pub fn vertex_weights(&self) -> Vec<f64> {
        let weights = self.virtual_weights();
        let mut values = weights.iter().map(|w| w[0]).collect::<Vec<_>>();
        if !self.topology.is_closed() {
            values.push(weights.last().unwrap()[1]);
        }
        values
    }

    pub fn vertex(&mut self, position: usize, percent: f64) -> Result<(), GeometryError> {
        let requested = SubD::soft_crease_sharpness(percent)?;
        let n = self.weights.len();
        let closed = self.topology.is_closed();
        let current = self.virtual_weights();
        // Recompute bends from the current accepted floats. A no-op pick must
        // not add a constraint, and a newly collinear bend may disappear.
        let bend = |v: usize| {
            if !closed && (v == 0 || v == n) {
                return true;
            }
            let prev = current[(v + n - 1) % n];
            let next = current[v % n];
            if prev[1] != next[0] {
                return true;
            }
            let scale = prev
                .into_iter()
                .chain(next)
                .fold(0_f64, |a, b| a.max(b.abs()));
            ((prev[1] - prev[0]) - (next[1] - next[0])).abs() > 8. * f32::EPSILON as f64 * scale
        };
        // Both walks use the same pre-edit profile. On an odd closed loop the
        // backward interval overwrites the shared middle edge, as observed.
        for forward in [true, false] {
            if !closed && ((forward && position == n) || (!forward && position == 0)) {
                continue;
            }
            let max = if closed {
                n
            } else if forward {
                n - position
            } else {
                position
            };
            let mut count = 1;
            while count < max {
                let v = if forward {
                    (position + count) % n
                } else {
                    (position + n - count) % n
                };
                if bend(v) {
                    break;
                }
                count += 1;
            }
            if closed && count == n {
                count = n.div_ceil(2);
            }
            let end = if forward {
                (position + count - 1) % n
            } else {
                (position + n - count) % n
            };
            let anchor = current[end][usize::from(forward)];
            let intervals = SubD::interpolate_edge_sharpness([requested, anchor], count)?;
            for (k, mut w) in intervals.into_iter().enumerate() {
                let edge = if forward {
                    (position + k) % n
                } else {
                    (position + n - 1 - k) % n
                };
                if !forward {
                    w.swap(0, 1);
                }
                self.weights[edge] = w;
                self.hard[edge] = false;
            }
        }
        Ok(())
    }

    pub fn edits(&self) -> impl Iterator<Item = (usize, bool, [f64; 2])> + '_ {
        self.topology.edges.iter().enumerate().map(|(i, edge)| {
            let mut w = self.weights[i];
            if edge.reversed {
                w.swap(0, 1);
            }
            (edge.index, self.hard[i], w)
        })
    }
}
