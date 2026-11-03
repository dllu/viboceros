//! Preserve unsplit command curves while later networks subdivide their edges.
use super::*;

#[derive(Default)]
pub(super) struct CurveOutputs {
    records: Vec<NetworkCurves>,
    owners: Vec<Option<usize>>,
}

impl CurveOutputs {
    pub fn push(&mut self, record: NetworkCurves, edge_count: usize) {
        self.owners.resize(edge_count, None);
        for &edge in &record.edges {
            self.owners[edge] = Some(self.records.len());
        }
        self.records.push(record);
    }

    pub fn inherit(&mut self, splits: &[(usize, Vec<usize>)], edge_count: usize) {
        if self.records.is_empty() {
            return;
        }
        self.owners.resize(edge_count, None);
        for (parent, fragments) in splits {
            if let Some(owner) = self.owners[*parent] {
                for &edge in fragments {
                    if self.owners[edge].is_none() {
                        self.records[owner].edges.push(edge);
                        self.owners[edge] = Some(owner);
                    }
                }
            }
        }
    }

    pub fn into_records(self) -> Vec<NetworkCurves> {
        self.records
    }
}
