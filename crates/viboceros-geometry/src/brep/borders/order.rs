//! Order of surviving fragments in exact, consistently oriented line joining.
//!
//! A new endpoint pair allocates a fragment slot; extending it preserves that
//! slot. Merging two fragments keeps the upstream slot. A disjoint-set forest
//! tracks that history without copying growing chains or renumbering members.
use std::collections::BTreeMap;

pub(super) fn linear_component_ranks(
    ends: &[[usize; 2]],
    at_vertex: &BTreeMap<usize, Vec<usize>>,
) -> Option<Vec<usize>> {
    let mut joins = Vec::new();
    for (&vertex, incident) in at_vertex {
        let [a, b] = incident.as_slice() else {
            if incident.len() > 2 {
                return None;
            }
            continue;
        };
        // Orientation conflicts and branch matching require a different
        // joining policy. Do not silently choose an arbitrary upstream side.
        if (ends[*a][1] == vertex) == (ends[*b][1] == vertex) {
            return None;
        }
        let upstream = if ends[*a][1] == vertex { *a } else { *b };
        joins.push(((*a).max(*b), (*a).min(*b), upstream));
    }
    joins.sort_unstable();
    let mut sets = Fragments::new(ends.len());
    let mut next_slot = 0;
    for (a, b, upstream) in joins {
        let a = sets.root(a);
        let b = sets.root(b);
        if a == b {
            continue;
        }
        let slot = match (sets.slots[a], sets.slots[b]) {
            (None, None) => {
                let slot = next_slot;
                next_slot += 1;
                slot
            }
            (Some(slot), None) | (None, Some(slot)) => slot,
            (Some(_), Some(_)) => {
                let root = sets.root(upstream);
                sets.slots[root].unwrap()
            }
        };
        sets.merge(a, b, slot);
    }
    Some(
        (0..ends.len())
            .map(|edge| {
                let root = sets.root(edge);
                // Isolated edges follow all joined components in original order.
                sets.slots[root].unwrap_or(next_slot + edge)
            })
            .collect(),
    )
}

struct Fragments {
    parents: Vec<usize>,
    sizes: Vec<usize>,
    slots: Vec<Option<usize>>,
}

impl Fragments {
    fn new(count: usize) -> Self {
        Self {
            parents: (0..count).collect(),
            sizes: vec![1; count],
            slots: vec![None; count],
        }
    }

    fn root(&mut self, mut index: usize) -> usize {
        while self.parents[index] != index {
            self.parents[index] = self.parents[self.parents[index]];
            index = self.parents[index];
        }
        index
    }

    fn merge(&mut self, mut a: usize, mut b: usize, slot: usize) {
        if self.sizes[a] < self.sizes[b] {
            std::mem::swap(&mut a, &mut b);
        }
        self.parents[b] = a;
        self.sizes[a] += self.sizes[b];
        self.slots[a] = Some(slot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn incidence(ends: &[[usize; 2]]) -> BTreeMap<usize, Vec<usize>> {
        let mut at_vertex = BTreeMap::<_, Vec<_>>::new();
        for (edge, vertices) in ends.iter().enumerate() {
            for &vertex in vertices {
                at_vertex.entry(vertex).or_default().push(edge);
            }
        }
        at_vertex
    }

    // Small independent reference: enumerate all input pairs and literally
    // prepend/append/concatenate their edge lists. Production never performs
    // these quadratic scans or repeatedly copies a growing fragment.
    fn reference(ends: &[[usize; 2]]) -> Vec<usize> {
        let mut fragments = Vec::<Vec<usize>>::new();
        for a in 1..ends.len() {
            for b in 0..a {
                let (up, down) = if ends[a][1] == ends[b][0] {
                    (a, b)
                } else if ends[b][1] == ends[a][0] {
                    (b, a)
                } else {
                    continue;
                };
                let left = fragments.iter().position(|f| f.contains(&up));
                let right = fragments.iter().position(|f| f.contains(&down));
                match (left, right) {
                    (None, None) => fragments.push(vec![up, down]),
                    (Some(left), None) => fragments[left].push(down),
                    (None, Some(right)) => fragments[right].insert(0, up),
                    (Some(left), Some(right)) if left != right => {
                        let tail = std::mem::take(&mut fragments[right]);
                        fragments[left].extend(tail);
                    }
                    _ => (),
                }
            }
        }
        let mut ranks = (0..ends.len())
            .map(|i| fragments.len() + i)
            .collect::<Vec<_>>();
        for (rank, fragment) in fragments.iter().enumerate() {
            for &edge in fragment {
                ranks[edge] = rank;
            }
        }
        ranks
    }

    #[test]
    fn randomized_cycles_match_independent_fragment_concatenation() {
        let mut state = 4321_u64;
        for _ in 0..512 {
            let mut ends = (0..24)
                .map(|edge| {
                    let base = edge / 8 * 8;
                    [edge, base + (edge - base + 1) % 8]
                })
                .collect::<Vec<_>>();
            for i in (1..ends.len()).rev() {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                ends.swap(i, (state as usize) % (i + 1));
            }
            assert_eq!(
                linear_component_ranks(&ends, &incidence(&ends)).unwrap(),
                reference(&ends)
            );
        }
    }

    #[test]
    fn large_interleaved_cycles_keep_independent_fragment_slots() {
        let ends = (0..12000)
            .map(|index| {
                let edge = (index * 13) % 12000;
                let base = edge / 1000 * 1000;
                [edge, base + (edge - base + 1) % 1000]
            })
            .collect::<Vec<_>>();
        let ranks = linear_component_ranks(&ends, &incidence(&ends)).unwrap();
        let mut by_loop = BTreeMap::new();
        for (edge, rank) in ends.iter().zip(ranks) {
            if let Some(previous) = by_loop.insert(edge[0] / 1000, rank) {
                assert_eq!(previous, rank);
            }
        }
        assert_eq!(
            by_loop
                .values()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            12
        );
    }

    #[test]
    fn conflicts_and_branches_are_not_assigned_an_arbitrary_upstream_direction() {
        for ends in [vec![[0, 1], [2, 1]], vec![[0, 1], [1, 2], [1, 3]]] {
            assert!(linear_component_ranks(&ends, &incidence(&ends)).is_none());
        }
        let ends = [[0, 1], [1, 2], [3, 4]];
        assert_eq!(
            linear_component_ranks(&ends, &incidence(&ends)).unwrap(),
            reference(&ends)
        );
    }
}
