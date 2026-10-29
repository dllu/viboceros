//! Run independent complete-image proofs concurrently and retain every failure.
use serde_json::Value;

pub fn compare(
    operations: &[Value],
    references: &[Value],
    compare: impl Fn(&Value, &Value) -> Option<String> + Sync,
) -> Vec<String> {
    assert_eq!(operations.len(), references.len());
    std::thread::scope(|scope| {
        let compare = &compare;
        let workers = operations
            .iter()
            .zip(references)
            .map(|(op, native)| {
                assert_eq!(op["id"], native["id"]);
                scope.spawn(move || compare(op, native))
            })
            .collect::<Vec<_>>();
        workers
            .into_iter()
            .filter_map(|worker| worker.join().unwrap())
            .collect()
    })
}
