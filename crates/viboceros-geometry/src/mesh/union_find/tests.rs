use super::*;

#[test]
fn index_representatives_match_label_partitions_for_all_short_union_sequences() {
    for length in 0..=4 {
        for mut code in 0..16usize.pow(length) {
            let pairs = (0..length)
                .map(|_| {
                    let pair = (code % 4, code / 4 % 4);
                    code /= 16;
                    pair
                })
                .collect::<Vec<_>>();
            for later in [false, true] {
                let mut parents = [0, 1, 2, 3];
                let mut labels = [0, 1, 2, 3];
                for &(first, second) in &pairs {
                    if later {
                        union_indices_keep_later(&mut parents, first, second);
                    } else {
                        union_indices_keep_earlier(&mut parents, first, second);
                    }
                    let old = labels[second];
                    let new = labels[first];
                    for label in &mut labels {
                        if *label == old {
                            *label = new;
                        }
                    }
                }
                for index in 0..4 {
                    let peers = (0..4).filter(|&peer| labels[peer] == labels[index]);
                    let expected = if later { peers.max() } else { peers.min() }.unwrap();
                    assert_eq!(
                        index_root(&mut parents, index),
                        expected,
                        "pairs={pairs:?}, later={later}, index={index}"
                    );
                }
                let compressed = parents;
                for index in 0..4 {
                    index_root(&mut parents, index);
                }
                assert_eq!(parents, compressed);
            }
        }
    }
}

#[test]
fn long_chains_are_fully_compressed_without_recursion() {
    let count: usize = 100_000;
    for descending in [false, true] {
        let mut parents = (0..count)
            .map(|index| {
                if descending {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(count - 1)
                }
            })
            .collect::<Vec<_>>();
        let (start, root) = if descending {
            (count - 1, 0)
        } else {
            (0, count - 1)
        };
        assert_eq!(index_root(&mut parents, start), root);
        assert!(parents.iter().all(|&parent| parent == root));
    }
}
