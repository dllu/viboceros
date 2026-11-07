use super::*;
pub(super) fn parse(
    doc: &Document,
    args: &[&str],
    preferences: Preferences,
) -> Result<Input, CommandError> {
    let mut input = Input::with_preferences(doc.tolerance(), preferences);

    let mut i = 0;
    while i < args.len() {
        if let Some(binding) = crate::curve_reference::parse_portion_binding(args[i]) {
            let p = binding?;
            input.add_subcurve(doc, p.object, p.edge, p.parameters, p.wrap)?;
            i += 1;
            continue;
        }
        if let Some((key, value)) = args[i].split_once('=') {
            if option_name_eq(key, "Point") {
                let (point, n) = parse_point(&[value])?;
                debug_assert_eq!(n, 1);
                input.add_point(point)?;
                i += 1;
                continue;
            }
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
                let (pick, n) = parse_point(&args[i + 1..])?;
                input.add(doc, id, edge, pick)?;
                i += n + 1;
                continue;
            }
        }
        if !input.apply_option(&options::normalized_option(args[i], doc.units())?)? {
            return Err(CommandError::Usage(USAGE));
        }
        i += 1;
    }
    if !input.ready() {
        return Err(CommandError::Usage(USAGE));
    }
    input.rebuild()?;
    Ok(input)
}
