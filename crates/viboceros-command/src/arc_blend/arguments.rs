use super::*;
pub(super) fn parse(
    doc: &Document,
    args: &[&str],
    preferences: Preferences,
) -> Result<Input, CommandError> {
    let mut input = Input::with_preferences(doc.tolerance(), preferences)?;
    let mut options = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if let Some((key, value)) = args[i].split_once('=') {
            let binding = if option_name_eq(key, "Curve") {
                Some((
                    value
                        .parse::<ObjectId>()
                        .map_err(|_| CommandError::Usage(USAGE))?,
                    None,
                ))
            } else if option_name_eq(key, "Edge") {
                let (id, e) =
                    crate::curve_reference::parse_edge(value).ok_or(CommandError::Usage(USAGE))?;
                Some((id, Some(e)))
            } else {
                None
            };
            if let Some((id, edge)) = binding {
                let (p, n) = parse_point(&args[i + 1..])?;
                input.add(doc, id, edge, p)?;
                i += n + 1;
                continue;
            }
        }
        options.push(args[i]);
        i += 1;
    }
    if !input.ready() {
        return Err(CommandError::Usage(USAGE));
    }
    for option in options {
        if !input.apply_option(option)? {
            return Err(CommandError::Usage(USAGE));
        }
    }
    input.rebuild()?;
    Ok(input)
}
