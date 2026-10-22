//! Explicit sources and temporary-range targets share the interactive plan.
use super::*;
pub struct Invocation {
    pub options: Options,
    pub targets: Option<Vec<Target>>,
    pub include_selected: bool,
}
pub fn parse(args: &[&str]) -> Result<Invocation, CommandError> {
    let usage = options::USAGE;
    let mut option_args = Vec::new();
    let mut targets = Vec::new();
    let mut explicit = false;
    let mut sources = false;
    let mut i = 0;
    while i < args.len() {
        let (name, value, n) = orient_option(args, i, usage)?;
        if option_name_eq(name, "SubCrv") {
            let input = crate::uv_inputs::SubcurveInput::parse(value, usage)?;
            targets.push(Target::CurveRange(input.object, input.parameters));
            explicit = true;
        } else if option_name_eq(name, "Sources") {
            if sources {
                return Err(CommandError::Usage(usage));
            }
            sources = true;
            explicit = true;
            for id in value.split(',').take(MAX_SPAN_OUTPUT_OBJECTS + 1) {
                targets.push(Target::Whole(
                    id.parse().map_err(|_| CommandError::Usage(usage))?,
                ));
            }
        } else {
            option_args.extend_from_slice(&args[i..i + n]);
        }
        if targets.len() > MAX_SPAN_OUTPUT_OBJECTS {
            return Err(too_many_span_outputs("ExtractControlPolygon"));
        }
        i += n;
    }
    Ok(Invocation {
        options: options::parse(&option_args)?,
        targets: explicit.then_some(targets),
        include_selected: !sources,
    })
}
