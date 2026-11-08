use super::*;
use viboceros_geometry::{
    CurveMatchPreserveEnd, SurfaceBlendContinuity, SurfaceMatchIso, SurfaceMatchOptions,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub policy: SurfaceMatchOptions,
}
impl Options {
    pub fn arguments(self) -> String {
        let yes = |v| if v { "Yes" } else { "No" };
        let iso = match self.policy.iso {
            SurfaceMatchIso::Automatic => "Automatic",
            SurfaceMatchIso::MatchTarget => "MatchTarget",
            SurfaceMatchIso::Perpendicular => "Perpendicular",
            SurfaceMatchIso::Preserve => "Keep",
        };
        format!(
            "Mode={:?} PreserveOtherEnd=Position MatchByClosestPoints={} RefineMatch={} IsocurveDirection={iso}",
            self.policy.continuity,
            yes(self.policy.closest_points),
            yes(self.policy.refine)
        )
    }
}
impl Default for Options {
    fn default() -> Self {
        Self {
            policy: SurfaceMatchOptions {
                continuity: SurfaceBlendContinuity::Curvature,
                other_end: CurveMatchPreserveEnd::Position,
                ..Default::default()
            },
        }
    }
}
pub fn parse(args: &[&str], mut options: Options) -> Result<Options, CommandError> {
    let mut seen = BTreeSet::new();
    let mut i = 0;
    while i < args.len() {
        let (key, value, n) = if let Some((k, v)) = args[i].split_once('=') {
            (k, v, 1)
        } else {
            (
                args[i],
                *args.get(i + 1).ok_or(CommandError::Usage(USAGE))?,
                2,
            )
        };
        let key = key.trim_start_matches('_').to_ascii_lowercase();
        let value = value.trim_start_matches('_').to_ascii_lowercase();
        if !seen.insert(key.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        match key.as_str() {
            "mode" => {
                options.policy.continuity = match value.as_str() {
                    "position" => SurfaceBlendContinuity::Position,
                    "tangency" => SurfaceBlendContinuity::Tangency,
                    "curvature" => SurfaceBlendContinuity::Curvature,
                    _ => return Err(CommandError::Usage(USAGE)),
                }
            }
            // Rhino 8.32's command accepts these choices but retains Position,
            // including on a quintic source. The geometry API separately supports
            // all four preserve policies; command captures pin this behavior.
            "preserveotherend"
                if ["nothing", "position", "tangency", "curvature"].contains(&value.as_str()) => {}
            "matchbyclosestpoints" => {
                options.policy.closest_points =
                    parse_yes_no(&value).ok_or(CommandError::Usage(USAGE))?
            }
            "refinematch" => {
                options.policy.refine = parse_yes_no(&value).ok_or(CommandError::Usage(USAGE))?
            }
            "averagesurfaces" if parse_yes_no(&value) == Some(false) => {}
            "averagesurfaces" => {
                return Err(CommandError::Usage(
                    "MatchSrf AverageSurfaces=Yes is not implemented yet",
                ));
            }
            "isocurvedirection" => {
                options.policy.iso = match value.as_str() {
                    "automatic" => SurfaceMatchIso::Automatic,
                    "keep" => SurfaceMatchIso::Preserve,
                    "matchtarget" => SurfaceMatchIso::MatchTarget,
                    "perpendicular" => SurfaceMatchIso::Perpendicular,
                    _ => return Err(CommandError::Usage(USAGE)),
                }
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
        i += n;
    }
    Ok(options)
}
