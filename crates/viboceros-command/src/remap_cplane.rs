//! Rigid placement between construction planes; named targets belong to the host.
use super::*;
use construction_plane::WorldPlane;

pub const USAGE: &str =
    "RemapCPlane (CPlane=name | View=name | TargetFrame=origin;x-axis;y-axis) [Copy=Yes|No]";

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    CPlane(String),
    View(String),
    Frame(Frame3),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub target: Option<Target>,
    pub copy: bool,
}

/// Preserve quoted names, including names that themselves spell Copy.
pub fn arguments(input: &str) -> Result<Vec<&str>, CommandError> {
    let mut tokens = Vec::new();
    let mut rest = input;
    while !rest.trim_start().is_empty() {
        rest = rest.trim_start();
        let mut quoted = false;
        let mut end = rest.len();
        for (i, c) in rest.char_indices() {
            if c == '"' {
                quoted = !quoted;
            } else if c.is_whitespace() && !quoted {
                end = i;
                break;
            }
        }
        if quoted {
            return Err(CommandError::Usage(USAGE));
        }
        let mut token = &rest[..end];
        let mut consumed = end;
        if option_name_eq(token, "CPlane") || option_name_eq(token, "View") {
            let tail = rest[end..].trim_start();
            if tail.is_empty() {
                return Err(CommandError::Usage(USAGE));
            }
            let length = if let Some(value) = tail.strip_prefix('"') {
                value.find('"').ok_or(CommandError::Usage(USAGE))? + 2
            } else {
                tail.find(char::is_whitespace).unwrap_or(tail.len())
            };
            consumed = rest.len() - tail.len() + length;
            token = &rest[..consumed];
        }
        tokens.push(token);
        rest = &rest[consumed..];
    }
    Ok(tokens)
}

pub fn world_frame(name: &str) -> Option<Frame3> {
    WorldPlane::ALL
        .into_iter()
        .find(|p| option_name_eq(name, p.label()))
        .map(|plane| {
            // RemapCPlane's built-in Bottom uses -X,+Y, unlike the
            // construction-plane editor's Bottom preset.
            if plane == WorldPlane::Bottom {
                Frame3::try_from_directions(
                    Point3::try_new(0., 0., 0.).unwrap(),
                    Vector3::try_new(-1., 0., 0.).unwrap(),
                    Vector3::try_new(0., 1., 0.).unwrap(),
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .expect("orthonormal Bottom target")
            } else {
                plane.frame()
            }
        })
}

pub fn options(arguments: &[&str], default_copy: bool) -> Result<Options, CommandError> {
    let mut result = Options {
        target: None,
        copy: default_copy,
    };
    let mut copy_seen = false;
    let mut i = 0;
    while i < arguments.len() {
        let (name, value, count) =
            if let Some(end) = arguments[i].find(|c: char| c == '=' || c.is_whitespace()) {
                let length = arguments[i][end..].chars().next().unwrap().len_utf8();
                (&arguments[i][..end], arguments[i][end + length..].trim(), 1)
            } else {
                orient_option(arguments, i, USAGE)?
            };
        if option_name_eq(name, "Copy") && !copy_seen {
            result.copy = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            copy_seen = true;
        } else if result.target.is_none() {
            let target = if option_name_eq(name, "TargetFrame") {
                Target::Frame(parse_frame(value)?)
            } else {
                let value = value.strip_prefix('"').map_or(Ok(value), |v| {
                    v.strip_suffix('"').ok_or(CommandError::Usage(USAGE))
                })?;
                if value.is_empty() || value.contains('"') || value.chars().any(char::is_control) {
                    return Err(CommandError::Usage(USAGE));
                }
                if option_name_eq(name, "CPlane") {
                    Target::CPlane(value.into())
                } else if option_name_eq(name, "View") {
                    Target::View(value.into())
                } else {
                    return Err(CommandError::Usage(USAGE));
                }
            };
            result.target = Some(target);
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        i += count;
    }
    Ok(result)
}

fn parse_frame(value: &str) -> Result<Frame3, CommandError> {
    let points = value
        .split(';')
        .map(|v| {
            let (point, consumed) = parse_point(&[v])?;
            if consumed != 1 {
                return Err(CommandError::Usage(USAGE));
            }
            Ok(point)
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    let [origin, x, y] = points.as_slice() else {
        return Err(CommandError::Usage(USAGE));
    };
    Ok(Frame3::try_from_unit_directions(
        *origin,
        Vector3::try_from(x.to_array())?.normalized_nonzero()?,
        Vector3::try_from(y.to_array())?.normalized_nonzero()?,
        Tolerance::NUMERICAL_VALIDATION,
    )?)
}

pub fn frame_argument(frame: Frame3) -> String {
    let coordinates = [
        frame.origin().to_array(),
        frame.x_axis().as_vector().to_array(),
        frame.y_axis().as_vector().to_array(),
    ];
    format!(
        "TargetFrame={}",
        coordinates
            .map(|v| format!("{},{},{}", v[0], v[1], v[2]))
            .join(";")
    )
}

pub fn transform(source: Frame3, target: Frame3) -> Result<AffineTransform3, CommandError> {
    // Reconstructing an already unit frame can add a few rounding bits.
    if source.origin() == target.origin()
        && [source.x_axis(), source.y_axis()]
            .into_iter()
            .zip([target.x_axis(), target.y_axis()])
            .all(|(a, b)| {
                a.as_vector()
                    .to_array()
                    .into_iter()
                    .zip(b.as_vector().to_array())
                    .all(|(a, b)| (a - b).abs() <= 64. * Real::EPSILON)
            })
    {
        return Err(CommandError::Usage("specify a different CPlane"));
    }
    Ok(AffineTransform3::try_frame_mapping(
        source, target, [1.; 3],
    )?)
}

pub(super) struct RemapCPlaneCommand;
impl Command for RemapCPlaneCommand {
    fn name(&self) -> &'static str {
        "RemapCPlane"
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        arguments(input)
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        document: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (args, sources) = affine_transform_arguments(document, args, USAGE)?;
        let options = options(&args, false)?;
        let target = match options.target {
            Some(Target::Frame(frame)) => frame,
            Some(Target::CPlane(name)) => world_frame(&name).ok_or(CommandError::Usage(
                "named CPlane targets require application view context",
            ))?,
            _ => return Err(CommandError::Usage(USAGE)),
        };
        let transform = transform(context.construction_plane, target)?;
        let (count, copies) =
            apply_transform_with_renewal(document, &sources, transform, options.copy)?;
        Ok(format!(
            "Remapped {count} object(s), creating {copies} copy object(s)"
        ))
    }
}

#[cfg(test)]
mod tests;
