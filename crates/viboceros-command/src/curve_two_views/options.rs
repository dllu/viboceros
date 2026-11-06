use super::*;
#[derive(Clone, Debug, Default)]
pub struct Input {
    pub sources: Option<[ObjectId; 2]>,
    pub directions: [Option<Vector3>; 2],
    pub edges: [Option<usize>; 2],
    pub subcurves: [Option<[Real; 2]>; 2],
    pub wrap_edges: [bool; 2],
}
impl Input {
    pub fn profiles(&self, doc: &Document) -> Result<[Profile; 2], CommandError> {
        let sources = self.sources.map_or_else(
            || {
                doc.selected_objects()
                    .filter(|o| o.geometry().curve_ref().is_some())
                    .map(|o| o.id())
                    .collect::<Vec<_>>()
            },
            |ids| ids.to_vec(),
        );
        let sources: [ObjectId; 2] = sources
            .try_into()
            .map_err(|_| CommandError::Usage("Crv2View requires exactly two profiles"))?;
        Ok(std::array::from_fn(|i| Profile {
            object: sources[i],
            edge: self.edges[i],
            parameters: self.subcurves[i],
            direction: self.directions[i],
            wrap_edge: self.wrap_edges[i],
        }))
    }
}
pub fn parse(args: &[&str]) -> Result<Input, CommandError> {
    let mut input = Input::default();
    let mut seen = BTreeSet::new();
    for arg in args {
        let (key, value) = arg
            .trim_start_matches('_')
            .split_once('=')
            .ok_or(CommandError::Usage(USAGE))?;
        let key = key.to_ascii_lowercase();
        if !seen.insert(key.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        if key == "sources" {
            let ids = value
                .split(',')
                .map(|s| {
                    s.parse::<ObjectId>()
                        .map_err(|_| CommandError::Usage(USAGE))
                })
                .collect::<Result<Vec<_>, _>>()?;
            input.sources = Some(ids.try_into().map_err(|_| CommandError::Usage(USAGE))?);
            continue;
        }
        let index = if key.ends_with('1') {
            0
        } else if key.ends_with('2') {
            1
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        match &key[..key.len() - 1] {
            "direction" => {
                let words = value
                    .split(',')
                    .map(parse_finite_real)
                    .collect::<Result<Vec<_>, _>>()?;
                let values: [Real; 3] = words.try_into().map_err(|_| CommandError::Usage(USAGE))?;
                let direction = Vector3::try_from(values)?;
                direction.normalized_nonzero()?;
                input.directions[index] = Some(direction);
            }
            "edge" | "edgewrap" => {
                if input.edges[index].is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
                input.edges[index] = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?);
                input.wrap_edges[index] = key.starts_with("edgewrap");
            }
            "subcrv" => {
                let words = value
                    .split(',')
                    .map(parse_finite_real)
                    .collect::<Result<Vec<_>, _>>()?;
                input.subcurves[index] =
                    Some(words.try_into().map_err(|_| CommandError::Usage(USAGE))?);
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    Ok(input)
}
