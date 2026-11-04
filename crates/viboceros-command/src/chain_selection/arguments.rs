use super::*;
const USAGE: &str = "SelChain Seed=uuid|Edge=uuid,index [ChainContinuity=Position|Tangency|Curvature Direction=Forward|Backward|Both GapTolerance=distance AngleTolerance=degrees]";
pub fn parse_options(
    input: &str,
    mut options: CurveChainOptions,
    tolerance: Tolerance,
) -> Result<CurveChainOptions, CommandError> {
    let words = input.split_whitespace().collect::<Vec<_>>();
    let mut i = 0;
    while i < words.len() {
        let (name, value, n) = orient_option(&words, i, USAGE)?;
        if option_name_eq(name, "ChainContinuity") {
            options.continuity = if option_name_eq(value, "Position") {
                ChainContinuity::Position
            } else if option_name_eq(value, "Tangency") {
                ChainContinuity::Tangency
            } else if option_name_eq(value, "Curvature") {
                ChainContinuity::Curvature
            } else {
                return Err(CommandError::Usage(USAGE));
            };
        } else if option_name_eq(name, "Direction") {
            options.direction = if option_name_eq(value, "Forward") {
                ChainDirection::Forward
            } else if option_name_eq(value, "Backward") {
                ChainDirection::Backward
            } else if option_name_eq(value, "Both") {
                ChainDirection::Both
            } else {
                return Err(CommandError::Usage(USAGE));
            };
        } else if option_name_eq(name, "GapTolerance") {
            options.gap_tolerance = numeric(value, false, tolerance.absolute())?;
        } else if option_name_eq(name, "AngleTolerance") {
            options.angle_tolerance = numeric(value, true, tolerance.angular())?;
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        i += n;
    }
    options.validate()?;
    Ok(options)
}
fn numeric(value: &str, angle: bool, default: f64) -> Result<f64, CommandError> {
    let n = value
        .parse::<f64>()
        .map_err(|_| CommandError::Usage(USAGE))?;
    if !n.is_finite() || n < 0. || (angle && n > 180.) {
        return Err(CommandError::Usage(USAGE));
    }
    let n = if angle { n.to_radians() } else { n };
    // Public OpenNURBS ON_ZERO_TOLERANCE (2^-32). Native controls bracket
    // this lower bound for both distance and radians; zero restores the model default.
    Ok(if n <= 2.3283064365386963e-10 {
        default
    } else {
        n
    })
}
pub fn defaults(doc: &Document) -> CurveChainOptions {
    CurveChainOptions {
        continuity: ChainContinuity::Tangency,
        direction: ChainDirection::Both,
        gap_tolerance: doc.tolerance().absolute(),
        angle_tolerance: doc.tolerance().angular(),
    }
}
pub(super) fn script(
    args: &[&str],
    mut options: CurveChainOptions,
    tolerance: Tolerance,
) -> Result<(CurveChainOptions, Target), CommandError> {
    let mut seed = None;
    let mut i = 0;
    while i < args.len() {
        let (name, value, n) = orient_option(args, i, USAGE)?;
        if option_name_eq(name, "Seed") || option_name_eq(name, "Edge") {
            let target = if option_name_eq(name, "Seed") {
                Target::Curve(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
            } else {
                let (id, index) =
                    crate::curve_reference::parse_edge(value).ok_or(CommandError::Usage(USAGE))?;
                Target::Edge(id, index)
            };
            if seed.replace(target).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else {
            options = parse_options(&args[i..i + n].join(" "), options, tolerance)?;
        }
        i += n;
    }
    Ok((options, seed.ok_or(CommandError::Usage(USAGE))?))
}
