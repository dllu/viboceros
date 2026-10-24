use super::*;

pub(super) struct Arguments {
    pub cutters: Option<Vec<ObjectId>>,
    pub planes: Vec<String>,
    pub target: Option<ObjectId>,
    pub point: Option<Point3>,
}
impl Arguments {
    pub(super) fn parse(args: &[&str]) -> Result<Self, CommandError> {
        let mut result = Self {
            cutters: None,
            planes: vec![],
            target: None,
            point: None,
        };
        let mut seen = BTreeSet::new();
        let mut i = 0;
        while i < args.len() {
            let (name, value, n) = orient_option(args, i, USAGE)?;
            let name = name.trim_start_matches('_').to_ascii_lowercase();
            if name != "infiniteplane" && !seen.insert(name.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            match name.as_str() {
                "cutters" => result.cutters = Some(boolean_solids::parse_ids(value, USAGE)?),
                "infiniteplane" => result.planes.push(value.to_owned()),
                "target" => {
                    result.target = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
                }
                "point" => result.point = Some(parse_point(&[value])?.0),
                _ => return Err(CommandError::Usage(USAGE)),
            }
            i += n;
        }
        Ok(result)
    }
}
