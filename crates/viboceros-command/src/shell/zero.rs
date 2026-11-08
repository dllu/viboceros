//! The zero-thickness limit shares the opening rim between opposite skins.
use super::*;
use viboceros_geometry::{BrepFace, BrepLoop, BrepTrim, BrepTrimType};

pub(super) fn shell(
    source: &Brep,
    removed: &[usize],
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let removed = removed.iter().copied().collect::<BTreeSet<_>>();
    let retained = (0..source.faces().len())
        .filter(|i| !removed.contains(i))
        .collect::<Vec<_>>();
    let open = source.sub_brep(&retained, tolerance)?;
    let mut boundary = vec![false; open.edges().len()];
    for trim in open
        .faces()
        .iter()
        .flat_map(|f| f.loops())
        .flat_map(|l| l.trims())
    {
        if trim.trim_type() == BrepTrimType::Boundary {
            boundary[trim.edge().expect("boundary edge")] = true;
        }
    }
    let mut edges = open.edges().to_vec();
    let mut inner_edges = Vec::with_capacity(edges.len());
    for (i, edge) in open.edges().iter().enumerate() {
        inner_edges.push(if boundary[i] {
            i
        } else {
            let index = edges.len();
            edges.push(edge.clone());
            index
        });
    }
    let mut faces = Vec::with_capacity(open.faces().len() * 2);
    for inner in [false, true] {
        for face in open.faces() {
            let loops = face
                .loops()
                .iter()
                .map(|l| {
                    let trims = l
                        .trims()
                        .iter()
                        .map(|t| {
                            let edge = t.edge().map(|i| if inner { inner_edges[i] } else { i });
                            BrepTrim::try_new(
                                t.vertices(),
                                edge,
                                t.is_reversed_3d(),
                                t.curve().clone(),
                                if t.trim_type() == BrepTrimType::Boundary {
                                    BrepTrimType::Mated
                                } else {
                                    t.trim_type()
                                },
                                t.iso(),
                                t.tolerance(),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    BrepLoop::try_new(l.loop_type(), trims)
                })
                .collect::<Result<Vec<_>, _>>()?;
            faces.push(BrepFace::try_new(
                face.surface().clone(),
                face.is_reversed() ^ inner,
                loops,
            )?);
        }
    }
    Ok(Brep::try_new(
        open.vertices().to_vec(),
        edges,
        faces,
        tolerance,
    )?)
}
