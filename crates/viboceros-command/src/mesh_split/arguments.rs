use super::*;
#[derive(Clone, Copy)]
pub(super) struct Settings {
    pub ngons: bool,
    pub discard: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            ngons: true,
            discard: true,
        }
    }
}
pub(super) struct Arguments {
    pub shared: mesh_boolean::Options,
    pub settings: Settings,
    pub first: Option<Vec<ObjectId>>,
    pub second: Vec<ObjectId>,
    pub planes: Vec<String>,
    pub permit: bool,
}
impl Arguments {
    pub fn parse(
        args: &[&str],
        shared: mesh_boolean::Options,
        settings: Settings,
    ) -> Result<Self, CommandError> {
        let mut result = Self {
            shared,
            settings,
            first: None,
            second: vec![],
            planes: vec![],
            permit: false,
        };
        let mut seen = BTreeSet::new();
        let mut i = 0;
        while i < args.len() {
            let token = args[i].trim_start_matches(['_', '-']);
            if token.eq_ignore_ascii_case("PermitSplittingInputsWithEachOther") {
                if !seen.insert("permit") {
                    return Err(CommandError::Usage(USAGE));
                }
                result.permit = true;
                i += 1;
                continue;
            }
            let (name, value, n) = orient_option(args, i, USAGE)?;
            let name = name.trim_start_matches('_').to_ascii_lowercase();
            if name != "infiniteplane"
                && !seen.insert(match name.as_str() {
                    "tolerance" => "tolerance",
                    "coplanarfaces" => "coplanarfaces",
                    "createngons" => "createngons",
                    "discardunsplitmeshes" => "discardunsplitmeshes",
                    "firstset" => "firstset",
                    "secondset" => "secondset",
                    "permitsplittinginputswitheachother" => "permit",
                    _ => return Err(CommandError::Usage(USAGE)),
                })
            {
                return Err(CommandError::Usage(USAGE));
            }
            match name.as_str() {
                "tolerance" => {
                    result.shared.tolerance = ["Auto", "Document", "Zero"]
                        .into_iter()
                        .find(|v| option_name_eq(value, v))
                        .ok_or(CommandError::Usage(USAGE))?
                }
                "coplanarfaces" => {
                    if option_name_eq(value, "Drop") {
                        result.shared.coplanar = "Drop";
                    } else if option_name_eq(value, "Keep") {
                        result.shared.coplanar = "Polylines";
                    } else {
                        return Err(CommandError::Usage(USAGE));
                    }
                }
                "createngons" => {
                    result.settings.ngons = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "discardunsplitmeshes" => {
                    result.settings.discard =
                        parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "firstset" => result.first = Some(boolean_solids::parse_ids(value, USAGE)?),
                "secondset" => result.second = boolean_solids::parse_ids(value, USAGE)?,
                "infiniteplane" => result.planes.push(value.to_owned()),
                "permitsplittinginputswitheachother" => {
                    result.permit = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
            i += n;
        }
        Ok(result)
    }
}
