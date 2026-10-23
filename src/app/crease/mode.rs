//! Command policies for the shared component getter.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(in crate::app) enum Mode {
    Crease(CreaseMode),
    Vertices(VertexSeamMode),
    Edges(EdgeSeamMode),
}
pub(super) struct Outcome {
    pub message: String,
    pub processed_objects: Vec<ObjectId>,
    pub retained_edges: Vec<(ObjectId, usize)>,
}
impl Mode {
    pub(in crate::app) fn command(self) -> &'static str {
        match self {
            Self::Crease(m) => m.command(),
            Self::Vertices(m) => m.command(),
            Self::Edges(m) => m.command(),
        }
    }
    pub(super) fn eligible(self, kind: ComponentSelectionKind) -> bool {
        match self {
            Self::Crease(_) => eligible(kind),
            Self::Vertices(_) => kind == ComponentSelectionKind::SubDVertex,
            Self::Edges(_) => matches!(
                kind,
                ComponentSelectionKind::MeshEdge | ComponentSelectionKind::SubDEdge
            ),
        }
    }
    pub(super) fn commit(
        self,
        doc: &mut Document,
        targets: Vec<CreaseTarget>,
    ) -> Result<Outcome, CommandError> {
        if let Self::Crease(m) = self {
            let out = CreaseSelection::prepare(doc, m, targets)?.commit(doc)?;
            return Ok(Outcome {
                message: out.message,
                processed_objects: out.processed_objects,
                retained_edges: Vec::new(),
            });
        }
        let mode: viboceros_command::component_seams::SeamMode = match self {
            Self::Vertices(m) => m.into(),
            Self::Edges(m) => m.into(),
            _ => unreachable!(),
        };
        let targets = targets.into_iter().filter_map(|t| match (self, t) {
            (Self::Vertices(_), CreaseTarget::Vertex(object, index))
            | (Self::Edges(_), CreaseTarget::Edge(object, index)) => {
                Some(SeamTarget { object, index })
            }
            _ => None,
        });
        let out = ComponentSeamEdit::prepare(doc, mode, targets)?.commit(doc)?;
        Ok(Outcome {
            message: out.message,
            processed_objects: out.released_objects,
            retained_edges: out
                .retained_edges
                .into_iter()
                .map(|t| (t.object, t.index))
                .collect(),
        })
    }
    pub(super) fn parse(name: &str, words: &mut std::str::SplitWhitespace<'_>) -> Option<Self> {
        let is = |aliases: &[&str]| aliases.iter().any(|s| name.eq_ignore_ascii_case(s));
        let mode = if is(&["Crease"]) {
            Self::Crease(CreaseMode::Add)
        } else if is(&["RemoveCrease"]) {
            Self::Crease(CreaseMode::Remove)
        } else if is(&["WeldVertices", "WeldVertex", "WeldMeshVertex"]) {
            Self::Vertices(VertexSeamMode::Weld)
        } else if is(&["WeldEdge", "WeldMeshEdge"]) {
            Self::Edges(EdgeSeamMode::Weld)
        } else if is(&[
            "UnweldVertex",
            "UnweldVertices",
            "UnweldMeshVertex",
            "UnweldEdge",
            "UnweldMeshEdge",
        ]) {
            let modify_normals = if let Some(option) = words.next() {
                let (key, value) = if let Some(pair) = option.split_once('=') {
                    pair
                } else {
                    (option, words.next()?)
                };
                if !key
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("ModifyNormals")
                {
                    return None;
                }
                match value.trim_start_matches('_') {
                    s if s.eq_ignore_ascii_case("Yes") => true,
                    s if s.eq_ignore_ascii_case("No") => false,
                    _ => return None,
                }
            } else {
                true
            };
            if is(&["UnweldEdge", "UnweldMeshEdge"]) {
                Self::Edges(EdgeSeamMode::Unweld { modify_normals })
            } else {
                Self::Vertices(VertexSeamMode::Unweld { modify_normals })
            }
        } else {
            return None;
        };
        words.next().is_none().then_some(mode)
    }
}
