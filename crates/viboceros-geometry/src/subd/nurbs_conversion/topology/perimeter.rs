//! Merge perimeter chains at common pack corners, retaining source edge identity.
use super::*;
use std::collections::BTreeSet;

pub(super) type Key = Vec<(Edge, bool)>;
pub(super) struct Segment {
    pub key: Key,
    pub vertices: [Vertex; 2],
    pub interval: [f64; 2],
    pub side: usize,
    pub reversed: bool,
    pub mated: bool,
}

fn atoms(
    source: &SubD,
    topology: &SourceEdges,
    pack: &SubDNurbsPackedPatch,
    side: usize,
) -> Vec<Side> {
    let [w, h] = pack.grid_size;
    let count = if side.is_multiple_of(2) { w } else { h };
    let mut result = Vec::new();
    for station in 0..count {
        let index = match side {
            0 => station,
            1 => station * w + w - 1,
            2 => (h - 1) * w + w - 1 - station,
            _ => (h - 1 - station) * w,
        };
        let tile = pack.tiles[index];
        let local = (side + 4 - tile.quarter_turns as usize) % 4;
        for mut atom in topology.sides(source, tile.face, tile.corner, local) {
            atom.interval = atom.interval.map(|t| (station as f64 + t) / count as f64);
            result.push(atom);
        }
    }
    result
}

pub(super) fn build(
    source: &SubD,
    packs: &[SubDNurbsPackedPatch],
) -> Result<Vec<Vec<Segment>>, GeometryError> {
    let topology = SourceEdges::new(source)?;
    let boundaries = packs
        .iter()
        .map(|p| std::array::from_fn::<_, 4, _>(|s| atoms(source, &topology, p, s)))
        .collect::<Vec<_>>();
    let corners = boundaries
        .iter()
        .flatten()
        .map(|side| side[0].vertices[0])
        .collect::<BTreeSet<_>>();
    let mut result = Vec::with_capacity(packs.len());
    for boundary in boundaries {
        let mut segments = Vec::new();
        for (side, atoms) in boundary.into_iter().enumerate() {
            let mut begin = 0;
            for end in 0..atoms.len() {
                if !corners.contains(&atoms[end].vertices[1]) {
                    continue;
                }
                let chain = &atoms[begin..=end];
                if chain.iter().any(|a| a.mated != chain[0].mated) {
                    return Err(failure("Inconsistent SubD pack perimeter incidence"));
                }
                let direct = chain
                    .iter()
                    .map(|a| (a.edge, a.vertices[0] > a.vertices[1]))
                    .collect::<Vec<_>>();
                let reverse = direct
                    .iter()
                    .rev()
                    .map(|&(edge, d)| (edge, !d))
                    .collect::<Vec<_>>();
                let reversed = reverse < direct;
                segments.push(Segment {
                    key: if reversed { reverse } else { direct },
                    vertices: [chain[0].vertices[0], chain.last().unwrap().vertices[1]],
                    interval: [chain[0].interval[0], chain.last().unwrap().interval[1]],
                    side,
                    reversed,
                    mated: chain[0].mated,
                });
                begin = end + 1;
            }
            if begin != atoms.len() {
                return Err(failure("Unclosed SubD packed side chain"));
            }
        }
        result.push(segments);
    }
    Ok(result)
}
