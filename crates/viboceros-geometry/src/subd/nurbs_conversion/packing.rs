//! Tensor-grid face packing, preserving source topology and local chart uses.
use super::*;
mod graph;
mod join;
mod layout;
mod refine;

/// One original quad or n-gon sector in a packed surface's unit cell grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubDNurbsTile {
    pub face: usize,
    pub corner: Option<usize>,
    pub cell: [usize; 2],
    /// Counterclockwise rotation from the local source patch to the pack chart.
    pub quarter_turns: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubDNurbsPackedPatch {
    pub grid_size: [usize; 2],
    /// Row-major uses, with U varying fastest.
    pub tiles: Vec<SubDNurbsTile>,
    pub surface: NurbsSurface,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Cell {
    face: usize,
    rotation: u8,
}
struct Plan {
    grid_size: [usize; 2],
    tiles: Vec<SubDNurbsTile>,
    skew: bool,
}
fn failure(context: &'static str) -> GeometryError {
    GeometryError::InvalidBrepTopology { context }
}

fn plan(source: &SubD) -> Result<Vec<Plan>, GeometryError> {
    let graph = graph::Graph::new(source)?;
    let records = viboceros_opennurbs::subd_face_packs(source.archive())?;
    if let Some(plans) = layout::stored(&graph, &records) {
        return Ok(plans);
    }
    automatic_plan(&graph)
}

fn automatic_plan(graph: &graph::Graph<'_>) -> Result<Vec<Plan>, GeometryError> {
    let source = graph.source;
    let mut groups = graph.components();
    // Native packing finds two-dimensional regular blocks before strips and
    // singleton faces. Source indices provide deterministic tie breaking.
    let mut group_of = vec![0; source.faces().len()];
    for (i, faces) in groups.iter().enumerate() {
        for &f in faces {
            group_of[f] = i;
        }
    }
    let mut interior = vec![None; groups.len()];
    for (v, edges) in graph.vertex_edges.iter().enumerate() {
        if source.vertex_tags()[v] != 1 || edges.len() != 4 {
            continue;
        }
        let Some(first) = graph.uses[edges[0]].first().map(|u| u.face) else {
            continue;
        };
        let group = group_of[first];
        if edges
            .iter()
            .all(|&e| graph.uses[e].iter().all(|u| group_of[u.face] == group))
        {
            interior[group].get_or_insert(v);
        }
    }
    groups.sort_by_cached_key(|faces| {
        let seed = interior[group_of[faces[0]]];
        (
            seed.is_none(),
            seed.unwrap_or(usize::MAX),
            *faces.iter().min().unwrap(),
        )
    });
    let mut plans = Vec::new();
    for group in groups {
        if group.len() == 1 && source.faces()[group[0]].len() != 4 {
            plans.extend(layout::star(source, group[0]));
        } else {
            plans.extend(layout::automatic(graph, &group)?);
        }
    }
    Ok(plans)
}

impl SubD {
    /// Reset the retained face packs from current topology, tags and creases.
    /// A deterministic grid atlas is assigned independently of Rhino's texture
    /// rectangle allocator. Control geometry and component identities survive.
    pub fn try_repacked_faces(&self) -> Result<Self, GeometryError> {
        let graph = graph::Graph::new(self)?;
        let mut plans = automatic_plan(&graph)?;
        // An n-gon's NURBS sectors belong to one source-face pack.
        plans.retain(|p| p.tiles[0].corner.is_none_or(|c| c == 0));
        let root = plans.len().isqrt();
        let width = root + usize::from(root * root != plans.len());
        let height = plans.len().div_ceil(width);
        let mut records = vec![None; self.faces().len()];
        for (i, plan) in plans.into_iter().enumerate() {
            let [w, h] = plan.grid_size;
            for tile in plan.tiles {
                let [x, y] = tile.cell;
                let cell = [(i % width) * w + x, (i / width) * h + y];
                let extent = [(w * width) as f64, (h * height) as f64];
                let lower = [cell[0] as f64 / extent[0], cell[1] as f64 / extent[1]];
                records[tile.face] = Some(viboceros_opennurbs::SubDFacePack {
                    id: (i + 1) as u32,
                    origin: lower,
                    size: std::array::from_fn(|a| (cell[a] + 1) as f64 / extent[a] - lower[a]),
                    quarter_turns: (4 - tile.quarter_turns) % 4,
                });
            }
        }
        let records = records
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| failure("Unassigned SubD source face pack"))?;
        Self::from_native(viboceros_opennurbs::subd_set_face_packs(
            self.archive(),
            &records,
        )?)
    }
}

pub(super) fn build(
    source: &SubD,
    patches: Vec<SubDNurbsPatch>,
) -> Result<Vec<SubDNurbsPackedPatch>, GeometryError> {
    let (mut plans, merged): (Vec<_>, Vec<_>) =
        plan(source)?.into_iter().partition(|p| p.tiles.len() == 1);
    // Rhino retains unmerged faces in source order, then appends the merged
    // grids in pack order. N-gon sectors keep their source corner order.
    plans.sort_by_key(|p| (p.tiles[0].face, p.tiles[0].corner));
    plans.extend(merged);
    let mut patches = patches
        .into_iter()
        .map(|p| ((p.face, p.corner), p.surface))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut result = Vec::with_capacity(plans.len());
    let mut points = 0usize;
    for plan in plans {
        let surface = join::surface(&plan, &mut patches, 4_000_000 - points)?;
        points += surface.control_points().len();
        result.push(SubDNurbsPackedPatch {
            grid_size: plan.grid_size,
            tiles: plan.tiles,
            surface,
        });
    }
    if !patches.is_empty() {
        return Err(failure("Unassigned SubD packing patches"));
    }
    Ok(result)
}
