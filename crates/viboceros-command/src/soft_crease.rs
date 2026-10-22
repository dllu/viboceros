//! Transactional SubDCrease editor shared by scripted commands and viewport getters.
use crate::*;
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{SubD, SubDEdgeChain};
mod arguments;
mod profile;
pub(super) use arguments::SubDCreaseCommand;
use profile::Profile;

const MAX_EDGES: usize = 400_000;
const UNAVAILABLE: &str = "SubDCrease requires interior SubD edges";

#[derive(Clone, Debug)]
struct Source {
    id: ObjectId,
    snapshot: GeometrySnapshot,
    original: SubD,
    candidate: SubD,
    profiles: Vec<Profile>,
}

/// A chain index belongs to this immutable source selection for the session's life.
#[derive(Clone, Debug)]
pub struct SoftCreaseSession {
    tolerance: Tolerance,
    sources: Vec<Source>,
}

#[derive(Clone, Copy, Debug)]
pub struct SoftCreaseLabel {
    pub object: ObjectId,
    pub chain: usize,
    pub vertex: usize,
    pub point: Point3,
    pub weight: f64,
}

impl SoftCreaseSession {
    pub fn prepare(
        document: &Document,
        edges: impl IntoIterator<Item = (ObjectId, usize)>,
    ) -> Result<Self, CommandError> {
        let mut selected = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
        for (n, (id, edge)) in edges.into_iter().enumerate() {
            if n >= MAX_EDGES {
                return Err(CommandError::CreaseResourceLimit);
            }
            selected.entry(id).or_default().insert(edge);
        }
        let mut sources = Vec::new();
        let mut points = 0_usize;
        let mut faces = 0_usize;
        for object in document.selectable_objects() {
            let Some(indices) = selected.remove(&object.id()) else {
                continue;
            };
            let Geometry::SubD(subd) = object.geometry() else {
                return Err(CommandError::Usage(UNAVAILABLE));
            };
            if indices.iter().any(|&e| e >= subd.edges().len()) {
                return Err(CommandError::Usage(UNAVAILABLE));
            }
            let indices = indices
                .into_iter()
                .filter(|&e| subd.edge_face_counts()[e] == 2)
                .collect::<Vec<_>>();
            if indices.is_empty() {
                continue;
            }
            points += subd.control_points().len();
            faces += subd.faces().len();
            if points > 4_000_000 || faces > 1_000_000 {
                return Err(CommandError::CreaseResourceLimit);
            }
            sources.push(Source {
                id: object.id(),
                snapshot: object.geometry_snapshot().clone(),
                original: subd.clone(),
                candidate: subd.clone(),
                profiles: subd
                    .try_edge_chains(&indices)?
                    .into_iter()
                    .map(|c| Profile::new(subd, c))
                    .collect(),
            });
        }
        if !selected.is_empty() || sources.is_empty() {
            return Err(CommandError::Usage(UNAVAILABLE));
        }
        Ok(Self {
            tolerance: document.tolerance(),
            sources,
        })
    }

    pub fn chains(&self) -> impl Iterator<Item = (ObjectId, &SubDEdgeChain)> {
        self.sources
            .iter()
            .flat_map(|s| s.profiles.iter().map(move |p| (s.id, &p.topology)))
    }
    pub fn candidates(&self) -> impl Iterator<Item = (ObjectId, &SubD)> {
        self.sources.iter().map(|s| (s.id, &s.candidate))
    }
    pub fn labels(&self) -> Vec<SoftCreaseLabel> {
        let mut result = Vec::new();
        let mut chain = 0;
        for source in &self.sources {
            for p in &source.profiles {
                let weights = p.vertex_weights();
                let count = p.topology.vertices.len() - usize::from(p.topology.is_closed());
                for (i, &vertex) in p.topology.vertices[..count].iter().enumerate() {
                    result.push(SoftCreaseLabel {
                        object: source.id,
                        chain,
                        vertex,
                        point: source.candidate.surface_points()[vertex],
                        weight: 25. * weights[i],
                    });
                }
                chain += 1;
            }
        }
        result
    }

    /// Global or per-chain constant weight. A soft weight of 100 is not a hard tag.
    pub fn constant(&mut self, chain: Option<usize>, percent: f64) -> Result<(), CommandError> {
        self.edit(chain, |p| p.constant(percent).map_err(Into::into))
    }
    pub fn crease(&mut self, chain: Option<usize>) -> Result<(), CommandError> {
        self.edit(chain, |p| {
            p.crease();
            Ok(())
        })
    }
    pub fn vertex(
        &mut self,
        chain: usize,
        vertex: usize,
        percent: f64,
    ) -> Result<(), CommandError> {
        self.edit(Some(chain), |p| {
            let position = p
                .topology
                .vertices
                .iter()
                .position(|&v| v == vertex)
                .ok_or(CommandError::Usage(
                    "vertex is outside the highlighted SubD chain",
                ))?;
            p.vertex(position, percent).map_err(Into::into)
        })
    }
    fn edit(
        &mut self,
        chain: Option<usize>,
        mut operation: impl FnMut(&mut Profile) -> Result<(), CommandError>,
    ) -> Result<(), CommandError> {
        if chain.is_some_and(|c| c >= self.chains().count()) {
            return Err(CommandError::Usage("SubD chain index out of range"));
        }
        let mut staged = self.clone();
        let mut index = 0;
        for source in &mut staged.sources {
            let mut touched = false;
            for profile in &mut source.profiles {
                if chain.is_none_or(|c| c == index) {
                    operation(profile)?;
                    touched = true;
                }
                index += 1;
            }
            if touched {
                let mut hard = Vec::new();
                let mut soft = Vec::new();
                for (edge, crease, weights) in source.profiles.iter().flat_map(Profile::edits) {
                    if crease {
                        hard.push(edge);
                    } else {
                        soft.push((edge, weights));
                    }
                }
                let candidate = source.original.try_with_stored_edge_sharpness(&soft)?.0;
                source.candidate = candidate.try_crease_components(&hard, &[], true)?.0;
            }
        }
        *self = staged;
        Ok(())
    }

    pub fn validate(&self, document: &Document) -> Result<(), CommandError> {
        let selectable = document
            .selectable_objects()
            .map(|o| (o.id(), o.geometry_snapshot()))
            .collect::<BTreeMap<_, _>>();
        if document.tolerance() != self.tolerance
            || self.sources.iter().any(|s| {
                selectable
                    .get(&s.id)
                    .is_none_or(|snapshot| *snapshot != &s.snapshot)
            })
        {
            return Err(CommandError::CreaseStale);
        }
        Ok(())
    }
    pub fn commit(&self, document: &mut Document) -> Result<Vec<ObjectId>, CommandError> {
        self.validate(document)?;
        // Rhino releases grip selection before recording its Undo snapshot.
        document.clear_selection();
        run_command_transaction(document, "SubDCrease", |doc| {
            let changed = self
                .sources
                .iter()
                .filter(|s| s.candidate != s.original)
                .map(|s| s.id)
                .collect::<Vec<_>>();
            doc.replace_object_geometries_preserving_control_selection(
                self.sources
                    .iter()
                    .filter(|s| s.candidate != s.original)
                    .map(|s| (s.id, Geometry::SubD(s.candidate.clone()))),
            )?;
            doc.move_objects_to_end_in_order(changed.iter().copied())?;
            Ok(changed)
        })
    }
}
