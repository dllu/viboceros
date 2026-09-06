//! Surface span conversion and its independent, non-undoable option memory.
use super::*;
use viboceros_geometry::MAX_BEZIER_CONTROL_POINTS;

#[cfg(test)]
mod tests;

const USAGE: &str = "ConvertToSingleSpans [Direction=U|V|Both] [DeleteInput=Yes|No] [Toggle]";
#[derive(Clone, Copy, Debug, PartialEq)]
struct Options {
    direction: SurfaceKnotDirection,
    delete_input: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            direction: SurfaceKnotDirection::Both,
            delete_input: false,
        }
    }
}
#[derive(Default)]
pub(super) struct ConvertToSingleSpansCommand {
    options: remembered::Remembered<Options>,
}

impl Command for ConvertToSingleSpansCommand {
    fn name(&self) -> &'static str {
        "ConvertToSingleSpans"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["ConvertSurfaceToSingleSpans"]
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments, self.options.get())?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut eligible = 0;
        let mut sources = Vec::new();
        let mut outputs = Vec::new();
        let mut controls = 0usize;
        for object in document.objects().filter(|o| document.is_selected(o.id())) {
            let surface = match object.geometry() {
                Geometry::NurbsSurface(s) => s,
                Geometry::Brep(b) if b.faces().len() == 1 => b.faces()[0].surface(),
                _ => continue,
            };
            eligible += 1;
            let u = options.direction != SurfaceKnotDirection::V;
            let v = options.direction != SurfaceKnotDirection::U;
            let count = (if u { surface.spans_u().count() } else { 1usize }).saturating_mul(if v {
                surface.spans_v().count()
            } else {
                1
            });
            if count == 1 {
                continue;
            }
            for patch in surface.try_single_span_patches(options.direction)? {
                controls = controls.saturating_add(patch.control_points().len());
                if controls > MAX_BEZIER_CONTROL_POINTS {
                    return Err(GeometryError::BezierDecompositionLimit.into());
                }
                outputs.push(Geometry::NurbsSurface(patch.try_reparameterized(
                    if u { 0.0..=1.0 } else { surface.domain_u() },
                    if v { 0.0..=1.0 } else { surface.domain_v() },
                )?));
            }
            sources.push(object.id());
        }
        if eligible == 0 {
            return Err(CommandError::UnsupportedConvertToSingleSpansGeometry);
        }
        let count = outputs.len();
        for output in outputs {
            document.add_geometry(output)?;
        }
        if options.delete_input {
            for id in &sources {
                document.delete_object(*id)?;
            }
        }
        // An eligible no-op still accepts choices. Geometry failure and invalid
        // options never reach this point; undo only changes document edits.
        self.options.set(options);
        Ok(format!(
            "Converted {} surface(s) into {count} single-span surface(s); {} unchanged{}",
            sources.len(),
            eligible - sources.len(),
            if options.delete_input {
                ""
            } else {
                "; inputs retained"
            }
        ))
    }
}

fn parse(arguments: &[&str], mut options: Options) -> Result<Options, CommandError> {
    let (mut direction_seen, mut delete_seen, mut index) = (false, false, 0);
    while index < arguments.len() {
        if option_name_eq(arguments[index], "Toggle") {
            options.direction = match options.direction {
                SurfaceKnotDirection::U => SurfaceKnotDirection::V,
                SurfaceKnotDirection::V => SurfaceKnotDirection::U,
                SurfaceKnotDirection::Both => return Err(CommandError::Usage(USAGE)),
            };
            index += 1;
            continue;
        }
        let (name, value, consumed) = orient_option(arguments, index, USAGE)?;
        if option_name_eq(name, "Direction") && !direction_seen {
            options.direction = match value
                .trim_start_matches(['_', '-'])
                .to_ascii_lowercase()
                .as_str()
            {
                "u" => SurfaceKnotDirection::U,
                "v" => SurfaceKnotDirection::V,
                "both" => SurfaceKnotDirection::Both,
                _ => return Err(CommandError::Usage(USAGE)),
            };
            direction_seen = true;
        } else if option_name_eq(name, "DeleteInput") && !delete_seen {
            options.delete_input = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            delete_seen = true;
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        index += consumed;
    }
    Ok(options)
}
