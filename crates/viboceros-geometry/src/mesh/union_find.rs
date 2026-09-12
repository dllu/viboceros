//! Shared path compression with rank-based or index-preserving union policies.

#[cfg(test)]
mod tests;

pub(super) fn union_faces(parents: &mut [usize], ranks: &mut [u8], first: usize, second: usize) {
    let first_root = index_root(parents, first);
    let second_root = index_root(parents, second);
    if first_root == second_root {
        return;
    }
    match ranks[first_root].cmp(&ranks[second_root]) {
        std::cmp::Ordering::Less => parents[first_root] = second_root,
        std::cmp::Ordering::Greater => parents[second_root] = first_root,
        std::cmp::Ordering::Equal => {
            parents[second_root] = first_root;
            ranks[first_root] += 1;
        }
    }
}

pub(super) fn index_root(parents: &mut [usize], index: usize) -> usize {
    let mut root = index;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = index;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

pub(super) fn union_indices_keep_later(parents: &mut [usize], first: usize, second: usize) {
    let first = index_root(parents, first);
    let second = index_root(parents, second);
    if first < second {
        parents[first] = second;
    } else if second < first {
        parents[second] = first;
    }
}

pub(super) fn union_indices_keep_earlier(parents: &mut [usize], first: usize, second: usize) {
    let first = index_root(parents, first);
    let second = index_root(parents, second);
    if first < second {
        parents[second] = first;
    } else if second < first {
        parents[first] = second;
    }
}
