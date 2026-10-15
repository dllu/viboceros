//! Selected-edge chamfer through the shared owned edge-blend transaction.
use super::*;
pub const USAGE: &str = "ChamferEdge [Distance=distance] [RailType=DistBetweenRails|RollingBall] [All=Yes|object-id edge-index[,edge-index...] ...]";
pub(super) struct ChamferEdgeCommand;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChamferRailType {
    DistanceBetweenRails,
    RollingBall,
}
impl ChamferRailType {
    pub fn token(self) -> &'static str {
        match self {
            Self::DistanceBetweenRails => "DistBetweenRails",
            Self::RollingBall => "RollingBall",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChamferOptions {
    pub distance: Real,
    pub rail: ChamferRailType,
}
impl Default for ChamferOptions {
    fn default() -> Self {
        Self {
            distance: 1.,
            rail: ChamferRailType::DistanceBetweenRails,
        }
    }
}
impl ChamferOptions {
    pub fn update(&mut self, words: &[&str]) -> Result<(), CommandError> {
        let mut next = *self;
        let mut keys = BTreeSet::new();
        for word in words {
            let (key, value) = if let Some((k, v)) = word.trim_start_matches('_').split_once('=') {
                (k.to_ascii_lowercase(), v)
            } else {
                ("distance".into(), *word)
            };
            if !keys.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.as_str() {
                "distance" => {
                    let n = parse_finite_real(value)?;
                    if n <= 0. {
                        return Err(CommandError::Usage(USAGE));
                    }
                    next.distance = n;
                }
                "railtype" => {
                    next.rail = if value.eq_ignore_ascii_case("DistBetweenRails")
                        || value.eq_ignore_ascii_case("DistanceBetweenRails")
                    {
                        ChamferRailType::DistanceBetweenRails
                    } else if value.eq_ignore_ascii_case("RollingBall") {
                        ChamferRailType::RollingBall
                    } else {
                        return Err(CommandError::Usage(USAGE));
                    }
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        }
        *self = next;
        Ok(())
    }
}
struct Input {
    options: ChamferOptions,
    all: bool,
    picks: Vec<(ObjectId, usize)>,
}
impl Input {
    fn parse(args: &[&str]) -> Result<Self, CommandError> {
        let mut options = Vec::new();
        let mut refs = Vec::new();
        let mut all = None;
        for word in args {
            if let Some((key, value)) = word.trim_start_matches('_').split_once('=') {
                if key.eq_ignore_ascii_case("All") {
                    if all
                        .replace(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?)
                        .is_some()
                    {
                        return Err(CommandError::Usage(USAGE));
                    }
                } else {
                    options.push(*word);
                }
            } else {
                refs.push(*word);
            }
        }
        // An odd leading numeric token is the distance; the remaining tokens are id/index pairs.
        if !refs.len().is_multiple_of(2) {
            options.push(refs.remove(0));
        }
        let mut parsed = ChamferOptions::default();
        parsed.update(&options)?;
        let mut picks = Vec::new();
        for pair in refs.chunks_exact(2) {
            let id = pair[0].parse().map_err(|_| CommandError::Usage(USAGE))?;
            for edge in pair[1].split(',') {
                if picks.len() >= 100000 {
                    return Err(CommandError::Usage(USAGE));
                }
                picks.push((id, edge.parse().map_err(|_| CommandError::Usage(USAGE))?));
            }
        }
        if all.is_some() && !picks.is_empty() || all == Some(false) {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Self {
            options: parsed,
            all: all.unwrap_or(false),
            picks,
        })
    }
}
impl Command for ChamferEdgeCommand {
    fn name(&self) -> &'static str {
        "ChamferEdge"
    }
    fn component_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        let input = Input::parse(args)?;
        Ok(
            (!input.all && input.picks.is_empty()).then_some(ComponentSelectionPrompt {
                command: "ChamferEdge",
                kind: ComponentSelectionKind::BrepEdge,
                options: vec![],
                numbers: vec![NumberSelectionOption {
                    name: "Distance",
                    value: input.options.distance,
                    minimum: Real::MIN_POSITIVE,
                }],
            }),
        )
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let mut input = Input::parse(args)?;
        if input.all {
            for o in doc.selected_objects() {
                let Geometry::Brep(b) = o.geometry() else {
                    return Err(CommandError::Usage(USAGE));
                };
                input.picks.extend(
                    b.edges_shared_by_distinct_faces()
                        .into_iter()
                        .enumerate()
                        .filter(|(_, shared)| *shared)
                        .map(|(i, _)| (o.id(), i)),
                );
            }
        }
        let selection = EdgeBlendSelection::prepare_chamfer(
            doc,
            input.picks,
            input.options.distance,
            input.options.rail,
        )?;
        let count = selection.apply(doc)?;
        Ok(format!("Chamfered edges on {count} object(s)"))
    }
}
#[cfg(test)]
mod tests;
