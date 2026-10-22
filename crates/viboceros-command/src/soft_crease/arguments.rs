use super::*;
const USAGE: &str = "SubDCrease Edges=All|0,1,... [Object=selected-uuid] [Chain=index] [Vertex=index] Weight=0..100|CreaseAll|CreaseThisChain";

pub(crate) struct SubDCreaseCommand;
impl Command for SubDCreaseCommand {
    fn name(&self) -> &'static str {
        "SubDCrease"
    }
    // This editor owns the transaction so that grip selection is released
    // before (rather than inside) the model's Undo snapshot.
    fn records_history(&self) -> bool {
        false
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut object = None;
        let mut edges = None;
        let mut actions = Vec::new();
        let mut started = false;
        let mut i = 0;
        while i < arguments.len() {
            let word = arguments[i].trim_start_matches(['_', '-']);
            let (key, value, consumed) = if let Some((k, v)) = word.split_once('=') {
                (k, Some(v), 1)
            } else if ["Edges", "Object", "Chain", "Vertex", "Weight"]
                .iter()
                .any(|k| word.eq_ignore_ascii_case(k))
            {
                (
                    word,
                    Some(*arguments.get(i + 1).ok_or(CommandError::Usage(USAGE))?),
                    2,
                )
            } else {
                (word, None, 1)
            };
            if key.eq_ignore_ascii_case("Edges") && !started && edges.is_none() {
                edges = Some(parse_surface_face_indices(value.unwrap(), USAGE)?);
            } else if key.eq_ignore_ascii_case("Object") && !started && object.is_none() {
                object = Some(
                    value
                        .unwrap()
                        .parse::<ObjectId>()
                        .map_err(|_| CommandError::Usage(USAGE))?,
                );
            } else {
                started = true;
                actions.push((key, value));
            }
            i += consumed;
        }
        let edges = edges.ok_or(CommandError::Usage(USAGE))?;
        if actions.is_empty() || object.is_some_and(|id| !document.is_selected(id)) {
            return Err(CommandError::Usage(USAGE));
        }
        let mut targets = Vec::new();
        for source in document
            .selected_objects()
            .filter(|o| object.is_none_or(|id| o.id() == id))
        {
            let Geometry::SubD(subd) = source.geometry() else {
                continue;
            };
            match &edges {
                SurfaceFaceIndices::Indices(indices) => {
                    if indices.len() > MAX_EDGES.saturating_sub(targets.len()) {
                        return Err(CommandError::CreaseResourceLimit);
                    }
                    targets.extend(indices.iter().map(|&e| (source.id(), e)));
                }
                SurfaceFaceIndices::All => {
                    if subd.edges().len() > MAX_EDGES.saturating_sub(targets.len()) {
                        return Err(CommandError::CreaseResourceLimit);
                    }
                    targets.extend((0..subd.edges().len()).map(|e| (source.id(), e)));
                }
            }
        }
        let mut session = SoftCreaseSession::prepare(document, targets)?;
        let mut chain = None;
        let mut vertex = None;
        let mut cancelled = false;
        for (key, value) in actions {
            let index = |value: Option<&str>| {
                value
                    .ok_or(CommandError::Usage(USAGE))?
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(USAGE))
            };
            if key.eq_ignore_ascii_case("Chain") {
                let selected = index(value)?;
                if selected >= session.chains().count() {
                    return Err(CommandError::Usage("SubD chain index out of range"));
                }
                chain = Some(selected);
            } else if key.eq_ignore_ascii_case("Vertex") {
                vertex = Some(index(value)?);
            } else if key.eq_ignore_ascii_case("Weight") {
                let weight = value
                    .ok_or(CommandError::Usage(USAGE))?
                    .parse::<f64>()
                    .map_err(|_| CommandError::Usage(USAGE))?;
                if let Some(v) = vertex.take() {
                    let c = match chain {
                        Some(c) => c,
                        None => {
                            let matches = session
                                .chains()
                                .enumerate()
                                .filter(|(_, (_, c))| c.vertices.contains(&v))
                                .map(|(i, _)| i)
                                .collect::<Vec<_>>();
                            if matches.len() != 1 {
                                return Err(CommandError::Usage(
                                    "specify Chain for an ambiguous SubD vertex",
                                ));
                            }
                            matches[0]
                        }
                    };
                    session.vertex(c, v, weight)?;
                } else {
                    session.constant(chain, weight)?;
                }
            } else if value.is_none() && key.eq_ignore_ascii_case("CreaseAll") {
                session.crease(None)?;
            } else if value.is_none() && key.eq_ignore_ascii_case("CreaseThisChain") {
                session.crease(Some(chain.ok_or(CommandError::Usage(USAGE))?))?;
            } else if value.is_none() && key.eq_ignore_ascii_case("Enter") { /* admit below */
            } else if value.is_none() && key.eq_ignore_ascii_case("Cancel") {
                cancelled = true;
                break;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        if cancelled {
            document.clear_selection();
            return Ok("SubDCrease cancelled".into());
        }
        if vertex.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let changed = session.commit(document)?;
        Ok(format!("SubDCrease: changed {} object(s)", changed.len()))
    }
}
