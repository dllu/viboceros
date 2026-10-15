//! Shared strict edge-blend options and original-index selectors.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailType {
    DistanceBetweenRails,
    RollingBall,
}
impl RailType {
    pub fn token(self) -> &'static str {
        match self {
            Self::DistanceBetweenRails => "DistBetweenRails",
            Self::RollingBall => "RollingBall",
        }
    }
}

pub(super) fn update_options(
    words: &[&str],
    parameter: &str,
    usage: &'static str,
    size: &mut Real,
    rail: &mut RailType,
) -> Result<(), CommandError> {
    let (mut next_size, mut next_rail) = (*size, *rail);
    let mut keys = BTreeSet::new();
    for word in words {
        let (key, value) = if let Some((k, v)) = word.trim_start_matches('_').split_once('=') {
            (k.to_ascii_lowercase(), v)
        } else {
            (parameter.to_ascii_lowercase(), *word)
        };
        if !keys.insert(key.clone()) {
            return Err(CommandError::Usage(usage));
        }
        match key.as_str() {
            key if key.eq_ignore_ascii_case(parameter) => {
                let n = parse_finite_real(value)?;
                if n <= 0. {
                    return Err(CommandError::Usage(usage));
                }
                next_size = n;
            }
            "railtype" => {
                next_rail = if value.eq_ignore_ascii_case("DistBetweenRails")
                    || value.eq_ignore_ascii_case("DistanceBetweenRails")
                {
                    RailType::DistanceBetweenRails
                } else if value.eq_ignore_ascii_case("RollingBall") {
                    RailType::RollingBall
                } else {
                    return Err(CommandError::Usage(usage));
                }
            }
            _ => return Err(CommandError::Usage(usage)),
        }
    }
    (*size, *rail) = (next_size, next_rail);
    Ok(())
}
pub(super) struct Input {
    pub size: Real,
    pub rail: RailType,
    pub all: bool,
    pub picks: Vec<(ObjectId, usize)>,
}
impl Input {
    pub fn parse(
        args: &[&str],
        parameter: &str,
        usage: &'static str,
        default_rail: RailType,
    ) -> Result<Self, CommandError> {
        let mut options = Vec::new();
        let mut refs = Vec::new();
        let mut all = None;
        for word in args {
            if let Some((key, value)) = word.trim_start_matches('_').split_once('=') {
                if key.eq_ignore_ascii_case("All") {
                    if all
                        .replace(parse_yes_no(value).ok_or(CommandError::Usage(usage))?)
                        .is_some()
                    {
                        return Err(CommandError::Usage(usage));
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
        let (mut size, mut rail) = (1., default_rail);
        update_options(&options, parameter, usage, &mut size, &mut rail)?;
        let mut picks = Vec::new();
        for pair in refs.chunks_exact(2) {
            let id = pair[0].parse().map_err(|_| CommandError::Usage(usage))?;
            for edge in pair[1].split(',') {
                if picks.len() >= 100000 {
                    return Err(CommandError::Usage(usage));
                }
                picks.push((id, edge.parse().map_err(|_| CommandError::Usage(usage))?));
            }
        }
        if all.is_some() && !picks.is_empty() || all == Some(false) {
            return Err(CommandError::Usage(usage));
        }
        Ok(Self {
            size,
            rail,
            all: all.unwrap_or(false),
            picks,
        })
    }
    pub fn resolve_picks(
        mut self,
        doc: &Document,
        usage: &'static str,
    ) -> Result<Vec<(ObjectId, usize)>, CommandError> {
        if self.all {
            for o in doc.selected_objects() {
                let Geometry::Brep(b) = o.geometry() else {
                    return Err(CommandError::Usage(usage));
                };
                self.picks.extend(
                    b.edges_shared_by_distinct_faces()
                        .into_iter()
                        .enumerate()
                        .filter(|(_, shared)| *shared)
                        .map(|(i, _)| (o.id(), i)),
                );
            }
        }
        Ok(self.picks)
    }
    pub fn prompt(
        &self,
        command: &'static str,
        parameter: &'static str,
    ) -> Option<ComponentSelectionPrompt> {
        (!self.all && self.picks.is_empty()).then_some(ComponentSelectionPrompt {
            command,
            kind: ComponentSelectionKind::BrepEdge,
            options: vec![],
            numbers: vec![NumberSelectionOption {
                name: parameter,
                value: self.size,
                minimum: Real::MIN_POSITIVE,
            }],
        })
    }
}
