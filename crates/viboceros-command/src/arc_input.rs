//! Pure Arc construction parsing, shared by output modes and command execution.
use crate::{
    CommandContext, CommandError, arc_tangent_input, arc_tilted_input, option_name_eq,
    parse_finite_real, parse_point, require_consumed,
};
use viboceros_document::Document;
use viboceros_geometry::{Circle3, CircularArc3, Point3, Real};
/// Final point constructions use these same geometry constructors on preview.
#[derive(Clone, Copy, Debug)]
pub enum Preview {
    Center {
        center: Point3,
        start: Point3,
        midpoint: bool,
        clockwise: bool,
    },
    ThreePoints {
        start: Point3,
        through: Point3,
    },
    StartEnd {
        start: Point3,
        end: Point3,
    },
    Direction {
        start: Point3,
        tangent: Point3,
    },
}
impl Preview {
    pub fn arc(
        self,
        point: Point3,
        plane: viboceros_geometry::Frame3,
        tolerance: viboceros_geometry::Tolerance,
    ) -> Result<CircularArc3, viboceros_geometry::GeometryError> {
        match self {
            Self::Center {
                center,
                start,
                midpoint,
                clockwise,
            } => {
                let normal = if clockwise {
                    plane.z_axis().opposite()
                } else {
                    plane.z_axis()
                };
                if midpoint {
                    CircularArc3::try_from_center_midpoint_end_on_plane(
                        center, start, point, normal, tolerance,
                    )
                } else {
                    CircularArc3::try_from_center_start_end_on_plane(
                        center, start, point, normal, tolerance,
                    )
                }
            }
            Self::ThreePoints { start, through } => {
                CircularArc3::try_from_three_points(start, through, point, tolerance)
            }
            Self::StartEnd { start, end } => {
                CircularArc3::try_from_three_points(start, point, end, tolerance)
            }
            Self::Direction { start, tangent } => CircularArc3::try_from_start_tangent_end(
                start,
                start.vector_to(tangent)?.normalized(tolerance)?,
                point,
                tolerance,
            ),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Definition {
    pub arc: CircularArc3,
    pub tangent_radius: Option<Real>,
    pub release_selection: bool,
    deformable_arc: Option<CircularArc3>,
    pub signed_sweep_degrees: Real,
}
impl Definition {
    pub fn output_arc(self, deformable: bool) -> CircularArc3 {
        if deformable {
            self.deformable_arc.unwrap_or(self.arc)
        } else {
            self.arc
        }
    }
    fn new(arc: CircularArc3) -> Self {
        Self {
            arc,
            tangent_radius: None,
            release_selection: false,
            deformable_arc: None,
            signed_sweep_degrees: arc.sweep_radians().to_degrees(),
        }
    }
}
pub fn from_arguments(
    document: &Document,
    arguments: &[&str],
    context: CommandContext,
    tangent_radius: Real,
) -> Result<Definition, CommandError> {
    if arguments
        .first()
        .is_some_and(|w| option_name_eq(w, "Extension"))
    {
        return Ok(Definition {
            release_selection: true,
            ..Definition::new(crate::arc_extension_input::from_arguments(
                document,
                &arguments[1..],
            )?)
        });
    }
    if arguments
        .first()
        .is_some_and(|w| option_name_eq(w, "Tangent"))
    {
        let d = arc_tangent_input::from_arguments(
            document,
            &arguments[1..],
            context.construction_plane,
            tangent_radius,
        )?;
        return Ok(Definition {
            tangent_radius: Some(d.arc.radius()),
            ..Definition::new(d.arc)
        });
    }
    if arguments
        .first()
        .is_some_and(|option| option_name_eq(option, "StartPoint"))
    {
        let (start, count) = parse_point(&arguments[1..])?;
        let remaining = &arguments[1 + count..];
        if let Some((name, value)) = remaining.first().and_then(|token| token.split_once('='))
            && option_name_eq(name, "Center")
        {
            let (_, used) = parse_point(&[value])?;
            require_consumed(&[value], used, ARC_START_USAGE)?;
            let mut reordered = vec!["Center", value];
            reordered.extend_from_slice(&arguments[1..1 + count]);
            reordered.extend_from_slice(&remaining[1..]);
            return from_arguments(document, &reordered, context, tangent_radius);
        }
        if remaining
            .first()
            .is_some_and(|token| option_name_eq(token, "Center"))
        {
            let (_, used) = parse_point(&remaining[1..])?;
            let mut reordered = vec!["Center"];
            reordered.extend_from_slice(&remaining[1..1 + used]);
            reordered.extend_from_slice(&arguments[1..1 + count]);
            reordered.extend_from_slice(&remaining[1 + used..]);
            return from_arguments(document, &reordered, context, tangent_radius);
        }
        if remaining
            .first()
            .is_some_and(|token| option_name_eq(token, "ThroughPoint"))
        {
            let (through, used) = parse_point(&remaining[1..])?;
            let (end, end_used) = parse_point(&remaining[1 + used..])?;
            require_consumed(&remaining[1..], used + end_used, ARC_START_USAGE)?;
            let arc =
                CircularArc3::try_from_three_points(start, through, end, document.tolerance())?;
            return Ok(Definition::new(arc));
        }
        let arc = if let Some((name, value)) =
            remaining.first().and_then(|token| token.split_once('='))
            && option_name_eq(name, "Direction")
        {
            let (direction, used) = parse_point(&[value])?;
            require_consumed(&[value], used, ARC_START_USAGE)?;
            let (end, used) = parse_point(&remaining[1..])?;
            require_consumed(&remaining[1..], used, ARC_START_USAGE)?;
            let tangent = start
                .vector_to(direction)?
                .normalized(document.tolerance())?;
            CircularArc3::try_from_start_tangent_end(start, tangent, end, document.tolerance())?
        } else if remaining
            .first()
            .is_some_and(|token| option_name_eq(token, "Direction"))
        {
            let (direction, used) = parse_point(&remaining[1..])?;
            let (end, end_used) = parse_point(&remaining[1 + used..])?;
            require_consumed(&remaining[1..], used + end_used, ARC_START_USAGE)?;
            let tangent = start
                .vector_to(direction)?
                .normalized(document.tolerance())?;
            CircularArc3::try_from_start_tangent_end(start, tangent, end, document.tolerance())?
        } else {
            let (end, used) = parse_point(remaining)?;
            let (through, through_used) = parse_point(&remaining[used..])?;
            require_consumed(remaining, used + through_used, ARC_START_USAGE)?;
            CircularArc3::try_from_three_points(start, through, end, document.tolerance())?
        };
        return Ok(Definition::new(arc));
    }
    if arguments
        .first()
        .is_some_and(|option| option_name_eq(option, "Center"))
    {
        let arguments = &arguments[1..];
        let (center, center_count) = parse_point(arguments)?;
        let after_center = &arguments[center_count..];
        if after_center
            .first()
            .is_some_and(|w| option_name_eq(w, "Tilted"))
        {
            let arc =
                arc_tilted_input::from_arguments(center, &after_center[1..], document.tolerance())?;
            return Ok(Definition::new(arc));
        }
        if after_center
            .first()
            .is_some_and(|option| option_name_eq(option, "Midpoint"))
        {
            return run_arc_midpoint(document, center, &after_center[1..], context);
        }
        let (start, start_count) = parse_point(&arguments[center_count..])?;
        let remaining = &arguments[center_count + start_count..];
        let length_input = remaining
            .first()
            .is_some_and(|w| option_name_eq(w.split('=').next().unwrap(), "Length"));
        let (remaining, endpoint_direction) = if !length_input
            && let Some((name, value)) = remaining.last().and_then(|token| token.split_once('='))
            && option_name_eq(name, "Direction")
        {
            let direction = if option_name_eq(value, "Clockwise") || option_name_eq(value, "CW") {
                Some(true)
            } else if option_name_eq(value, "Counterclockwise") || option_name_eq(value, "CCW") {
                Some(false)
            } else {
                return Err(CommandError::Usage(ARC_CENTER_USAGE));
            };
            (&remaining[..remaining.len() - 1], direction)
        } else {
            (remaining, None)
        };
        let radial = center.vector_to(start)?;
        let radius = radial.length()?;
        let endpoint_tokens = match remaining {
            [single] if single.contains(',') && !single.contains('=') => Some(remaining),
            [name, rest @ ..] if option_name_eq(name, "End") => Some(rest),
            [_, _, _] if !length_input => Some(remaining),
            _ => None,
        };
        let inline_endpoint = match remaining {
            [single] => single
                .split_once('=')
                .filter(|(name, _)| option_name_eq(name, "End"))
                .map(|(_, value)| value),
            _ => None,
        };
        let endpoint = if let Some(value) = inline_endpoint {
            let (end, consumed) = parse_point(&[value])?;
            require_consumed(&[value], consumed, ARC_CENTER_USAGE)?;
            Some(end)
        } else if let Some(tokens) = endpoint_tokens {
            let (end, consumed) = parse_point(tokens)?;
            require_consumed(tokens, consumed, ARC_CENTER_USAGE)?;
            Some(end)
        } else {
            None
        };
        if let Some(end) = endpoint {
            let normal = if endpoint_direction.unwrap_or(true) {
                context.construction_plane.z_axis().opposite()
            } else {
                context.construction_plane.z_axis()
            };
            let arc = CircularArc3::try_from_center_start_end_on_plane(
                center,
                start,
                end,
                normal,
                document.tolerance(),
            )?;
            return Ok(Definition::new(arc));
        }
        if endpoint_direction.is_some() {
            return Err(CommandError::Usage(ARC_CENTER_USAGE));
        }
        let signed_sweep = sweep_arguments(remaining, radius, ARC_CENTER_USAGE)?;
        let normal = if signed_sweep < 0.0 {
            context.construction_plane.z_axis().opposite()
        } else {
            context.construction_plane.z_axis()
        };
        let circle = Circle3::try_from_frame(
            center,
            radius,
            radial.normalized(document.tolerance())?,
            normal,
            document.tolerance(),
        )?;
        let sweep = signed_sweep.abs().min(std::f64::consts::TAU);
        let arc = CircularArc3::try_from_circle_sweep(circle, sweep)?;
        return Ok(Definition {
            signed_sweep_degrees: sweep.to_degrees().copysign(signed_sweep),
            ..Definition::new(arc)
        });
    }
    let mut centered = Vec::with_capacity(arguments.len() + 1);
    centered.push("Center");
    centered.extend_from_slice(arguments);
    from_arguments(document, &centered, context, tangent_radius)
}

const ARC_CENTER_USAGE: &str = "Arc Center center start end-point [Direction=Clockwise|Counterclockwise] | angle-degrees | Length=arc-length";
const ARC_START_USAGE: &str = "Arc StartPoint start end through | start ThroughPoint through end | start Direction=tangent-point end | start Center=center angle|Length=distance|End=point";
const ARC_MIDPOINT_USAGE: &str = "Arc Center center Midpoint midpoint end-point [Direction=Clockwise|Counterclockwise] | angle-degrees | Length=arc-length";

fn run_arc_midpoint(
    document: &Document,
    center: Point3,
    arguments: &[&str],
    context: CommandContext,
) -> Result<Definition, CommandError> {
    let (midpoint, count) = parse_point(arguments)?;
    let remaining = &arguments[count..];
    let length_input = remaining
        .first()
        .is_some_and(|w| option_name_eq(w.split('=').next().unwrap(), "Length"));
    let (remaining, endpoint_direction) = if !length_input
        && let Some((name, value)) = remaining.last().and_then(|token| token.split_once('='))
        && option_name_eq(name, "Direction")
    {
        let clockwise = if option_name_eq(value, "Clockwise") || option_name_eq(value, "CW") {
            true
        } else if option_name_eq(value, "Counterclockwise") || option_name_eq(value, "CCW") {
            false
        } else {
            return Err(CommandError::Usage(ARC_MIDPOINT_USAGE));
        };
        (&remaining[..remaining.len() - 1], Some(clockwise))
    } else {
        (remaining, None)
    };
    let endpoint_tokens = match remaining {
        [single] if single.contains(',') && !single.contains('=') => Some(remaining),
        [name, rest @ ..] if option_name_eq(name, "End") => Some(rest),
        [_, _, _] if !length_input => Some(remaining),
        _ => None,
    };
    let inline_endpoint = match remaining {
        [single] => single
            .split_once('=')
            .filter(|(name, _)| option_name_eq(name, "End"))
            .map(|(_, value)| value),
        _ => None,
    };
    let endpoint = if let Some(value) = inline_endpoint {
        let (point, used) = parse_point(&[value])?;
        require_consumed(&[value], used, ARC_MIDPOINT_USAGE)?;
        Some(point)
    } else if let Some(tokens) = endpoint_tokens {
        let (point, used) = parse_point(tokens)?;
        require_consumed(tokens, used, ARC_MIDPOINT_USAGE)?;
        Some(point)
    } else {
        None
    };
    let mut deformable_arc = None;
    let arc = if let Some(end) = endpoint {
        let normal = if endpoint_direction.unwrap_or(false) {
            context.construction_plane.z_axis().opposite()
        } else {
            context.construction_plane.z_axis()
        };
        CircularArc3::try_from_center_midpoint_end_on_plane(
            center,
            midpoint,
            end,
            normal,
            document.tolerance(),
        )?
    } else {
        if endpoint_direction.is_some() {
            return Err(CommandError::Usage(ARC_MIDPOINT_USAGE));
        }
        let radial = center.vector_to(midpoint)?;
        let radius = radial.length()?;
        let axis = radial.normalized(document.tolerance())?;
        let signed_sweep = sweep_arguments(remaining, radius, ARC_MIDPOINT_USAGE)?;
        let normal = if signed_sweep < 0.0 {
            context.construction_plane.z_axis().opposite()
        } else {
            context.construction_plane.z_axis()
        };
        let output_sweep = if signed_sweep.abs() >= std::f64::consts::TAU {
            std::f64::consts::PI
        } else {
            signed_sweep.abs()
        };
        deformable_arc = Some(CircularArc3::try_from_circle_sweep(
            Circle3::try_from_frame(center, radius, axis, normal, document.tolerance())?,
            output_sweep,
        )?);
        if signed_sweep.abs() >= std::f64::consts::TAU {
            let circle =
                Circle3::try_from_frame(center, radius, axis, normal, document.tolerance())?;
            CircularArc3::try_from_circle_sweep(circle, std::f64::consts::PI)?
        } else {
            CircularArc3::try_from_center_midpoint_sweep_on_plane(
                center,
                midpoint,
                normal,
                signed_sweep.abs(),
                document.tolerance(),
            )?
        }
    };
    Ok(Definition {
        deformable_arc,
        ..Definition::new(arc)
    })
}

/// A length getter measures either a signed number or two world-space points.
fn sweep_arguments(args: &[&str], radius: Real, usage: &'static str) -> Result<Real, CommandError> {
    if let [angle] = args
        && !angle.contains('=')
    {
        return Ok(parse_finite_real(angle)?.to_radians());
    }
    let Some(first) = args.first() else {
        return Err(CommandError::Usage(usage));
    };
    let (name, inline) = first
        .split_once('=')
        .map_or((*first, None), |(n, v)| (n, Some(v)));
    if !option_name_eq(name, "Length") {
        return Err(CommandError::Usage(usage));
    }
    let mut clockwise = false;
    let mut values = Vec::new();
    if let Some(value) = inline {
        values.push(value);
    }
    for &word in &args[1..] {
        if let Some((n, v)) = word.split_once('=')
            && option_name_eq(n, "Direction")
        {
            clockwise = if option_name_eq(v, "Clockwise") || option_name_eq(v, "CW") {
                true
            } else if option_name_eq(v, "Counterclockwise") || option_name_eq(v, "CCW") {
                false
            } else {
                return Err(CommandError::Usage(usage));
            };
        } else {
            values.push(word);
        }
    }
    let distance = if let [value] = values.as_slice() {
        parse_finite_real(value)?
    } else {
        let (a, used) = parse_point(&values)?;
        let (b, used_b) = parse_point(&values[used..])?;
        require_consumed(&values, used + used_b, usage)?;
        a.distance_to(b)?
    };
    Ok(if clockwise {
        -distance / radius
    } else {
        distance / radius
    })
}
