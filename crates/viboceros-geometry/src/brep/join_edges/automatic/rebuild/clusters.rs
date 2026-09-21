//! No vertex cluster may contain both distinct endpoints of an existing edge.
use super::*;
use std::collections::BTreeSet;

pub(super) fn groups(
    source: &Brep,
    contacts: &[(usize, usize)],
    budget: &mut Budget,
) -> Result<Vec<Vec<usize>>, GeometryError> {
    let n = source.vertices.len();
    budget.charge(n.saturating_add(source.edges.len()))?;
    // Charge the two bounded sorts before allocating a contact copy. Exact
    // distance comparisons preserve tiny features at arbitrary translations.
    let sort_work = contacts
        .len()
        .saturating_mul(contacts.len().max(1).ilog2() as usize + 1);
    budget.charge(sort_work.saturating_mul(2))?;
    let mut contacts = contacts
        .iter()
        .map(|&(a, b)| (a.min(b), a.max(b)))
        .collect::<Vec<_>>();
    contacts.sort_unstable();
    contacts.dedup();
    contacts.sort_unstable_by(|&(a, b), &(c, d)| {
        certificate::compare_pair_distances(
            [source.vertices[a].point, source.vertices[b].point],
            [source.vertices[c].point, source.vertices[d].point],
        )
        .then_with(|| (a, b).cmp(&(c, d)))
    });
    let mut roots = (0..n).collect::<Vec<_>>();
    let mut sizes = vec![1_usize; n];
    let mut adjacent = vec![BTreeSet::new(); n];
    for edge in &source.edges {
        let [a, b] = edge.vertices;
        if a != b {
            adjacent[a].insert(b);
            adjacent[b].insert(a);
        }
    }
    for (a, b) in contacts {
        budget.charge(1)?;
        let (mut a, mut b) = (root(&mut roots, a), root(&mut roots, b));
        if a == b || adjacent[a].contains(&b) {
            continue;
        }
        if sizes[a] < sizes[b] || (sizes[a] == sizes[b] && a > b) {
            std::mem::swap(&mut a, &mut b);
        }
        budget.charge(adjacent[b].len().saturating_mul(4))?;
        // Small-to-large unions bound repeated adjacency updates. Every set
        // contains live roots; a direct edge between them forbids the union.
        roots[b] = a;
        sizes[a] += sizes[b];
        for neighbor in std::mem::take(&mut adjacent[b]) {
            adjacent[neighbor].remove(&b);
            adjacent[neighbor].insert(a);
            adjacent[a].insert(neighbor);
        }
    }
    let mut groups = vec![Vec::new(); n];
    for i in 0..n {
        let r = root(&mut roots, i);
        groups[r].push(i);
    }
    Ok(groups)
}

fn root(roots: &mut [usize], mut i: usize) -> usize {
    while roots[i] != i {
        roots[i] = roots[roots[i]];
        i = roots[i];
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contact_order_and_duplicates_do_not_change_edge_protection() {
        let a = super::super::tests::sheet([0., 0.0001], 0.);
        let source = Brep::try_combine(vec![a.clone(), a], Tolerance::DEFAULT).unwrap();
        let contacts = (0..4)
            .flat_map(|a| (4..8).map(move |b| (a, b)))
            .filter(|&(a, b)| {
                certificate::point_bound(source.vertices[a].point, source.vertices[b].point, 0.002)
                    .is_some()
            })
            .collect::<Vec<_>>();
        let expected = groups(&source, &contacts, &mut Budget(MAX_WORK)).unwrap();
        let mut duplicate = contacts.clone();
        duplicate.extend(contacts.iter().map(|&(a, b)| (b, a)));
        duplicate.reverse();
        assert_eq!(
            groups(&source, &duplicate, &mut Budget(MAX_WORK)).unwrap(),
            expected
        );
        for edge in &source.edges {
            assert!(
                !expected
                    .iter()
                    .any(|g| g.contains(&edge.vertices[0]) && g.contains(&edge.vertices[1]))
            );
        }
        assert_eq!(expected.iter().filter(|g| !g.is_empty()).count(), 4);
        let before = source.clone();
        assert!(groups(&source, &contacts, &mut Budget(0)).is_err());
        assert_eq!(source, before);
    }
}
