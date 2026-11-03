//! Script bindings for the document settings shared by analysis panels.
use super::*;
use viboceros_geometry::{NurbsMeshDensity, SubDMeshDensity};

const USAGE: &str = "Analysis mesh: NurbsDensity=0..1 SubDLevel=0..6 SubDAdaptive=Yes|No";
pub(crate) struct MeshOptions<'a> {
    pub args: Vec<&'a str>,
    level: Option<u8>,
    adaptive: Option<bool>,
    nurbs: Option<NurbsMeshDensity>,
}
impl<'a> MeshOptions<'a> {
    pub fn parse(args: &'a [&'a str]) -> Result<Self, CommandError> {
        let mut result = Self {
            args: vec![],
            level: None,
            adaptive: None,
            nurbs: None,
        };
        for arg in args {
            let Some((key, value)) = arg.trim_start_matches(['_', '-']).split_once('=') else {
                result.args.push(arg);
                continue;
            };
            if key.eq_ignore_ascii_case("NurbsDensity") {
                let density = value
                    .parse::<Real>()
                    .ok()
                    .and_then(NurbsMeshDensity::new)
                    .ok_or(CommandError::Usage(USAGE))?;
                if result.nurbs.replace(density).is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
            } else if key.eq_ignore_ascii_case("SubDLevel") {
                let level = value
                    .parse::<u8>()
                    .ok()
                    .filter(|v| *v <= 6)
                    .ok_or(CommandError::Usage(USAGE))?;
                if result.level.replace(level).is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
            } else if key.eq_ignore_ascii_case("SubDAdaptive") {
                let adaptive = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                if result.adaptive.replace(adaptive).is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
            } else {
                result.args.push(arg);
            }
        }
        Ok(result)
    }
    pub fn configure_only(&self) -> bool {
        self.args.is_empty()
            && (self.nurbs.is_some() || self.level.is_some() || self.adaptive.is_some())
    }
    pub fn apply(&self, doc: &mut Document) {
        if let Some(density) = self.nurbs {
            doc.set_analysis_nurbs_density(density);
        }
        let old = doc.analysis_subd_density();
        let density = SubDMeshDensity::new(
            self.level.unwrap_or(old.level()),
            self.adaptive.unwrap_or(old.is_adaptive()),
        )
        .unwrap();
        doc.set_analysis_subd_density(density);
    }
}
