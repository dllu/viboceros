//! Native-domain seam relocation, directed subcurves, and reparameterization.

use super::*;

pub(super) const CURVE_SEAM_USAGE: &str = "CrvSeam point | CrvSeam Parameter=value";

#[derive(Clone, Copy, Debug, PartialEq)]
enum CurveSeamLocation {
    Point(Point3),
    Parameter(Real),
}

pub(super) struct CurveSeamCommand;

impl Command for CurveSeamCommand {
    fn name(&self) -> &'static str {
        "CrvSeam"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let location = parse_curve_seam_location(arguments)?;
        let mut candidates = Vec::new();
        for object in document.selected_objects() {
            let Some(curve_ref) = geometry_curve_ref(object.geometry()) else {
                continue;
            };
            if !curve_ref.is_closed()? {
                continue;
            }
            candidates.push((object.id(), curve_ref.to_owned()));
        }
        if candidates.len() != 1 {
            return Err(CommandError::CurveSeamRequiresOneClosedCurve {
                actual: candidates.len(),
            });
        }

        let (id, curve) = candidates
            .pop()
            .expect("one closed curve candidate was required");
        let parameter = match location {
            CurveSeamLocation::Point(point) => curve
                .as_ref()
                .closest_parameter(point, document.tolerance())?,
            CurveSeamLocation::Parameter(parameter) => parameter,
        };
        let relocated = curve.try_change_closed_seam(parameter)?;
        document.replace_object_geometries([(id, Geometry::from(relocated))])?;
        Ok(format!(
            "Set the selected closed curve seam at parameter {parameter}"
        ))
    }
}

fn parse_curve_seam_location(arguments: &[&str]) -> Result<CurveSeamLocation, CommandError> {
    let Some(first) = arguments.first() else {
        return Err(CommandError::Usage(CURVE_SEAM_USAGE));
    };
    let first_name = first.split_once('=').map_or(*first, |(name, _)| name);
    if option_name_eq(first_name, "Parameter") {
        let (name, value, consumed) = orient_option(arguments, 0, CURVE_SEAM_USAGE)?;
        if !option_name_eq(name, "Parameter") || consumed != arguments.len() {
            return Err(CommandError::Usage(CURVE_SEAM_USAGE));
        }
        return Ok(CurveSeamLocation::Parameter(parse_finite_real(value)?));
    }

    let (point, consumed) = parse_point(arguments).map_err(|error| match error {
        CommandError::Usage(_) => CommandError::Usage(CURVE_SEAM_USAGE),
        error => error,
    })?;
    require_consumed(arguments, consumed, CURVE_SEAM_USAGE)?;
    Ok(CurveSeamLocation::Point(point))
}

pub(super) const SUBCURVE_USAGE: &str = "SubCrv Parameter=start,end [Copy=Yes|No] | SubCrv start_point end_point [Copy=Yes|No] | SubCrv Numeric=anchor,length,confirmation [Copy=Yes|No] [Mode=Shorten|MarkEnds] [FromMidpoint=Yes|No] [Locked=Forward|Backward]";

#[derive(Clone, Copy, Debug, PartialEq)]
enum SubcurveLocation {
    Parameters([Real; 2]),
    Points([Point3; 2]),
    Numeric([Real; 3]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SubcurveOptions {
    location: SubcurveLocation,
    copy: bool,
    mode: subcurve_input::SubcurveMode,
    from_midpoint: bool,
    locked_forward: Option<bool>,
}

pub(super) struct SubcurveCommand(pub(super) subcurve_input::SubcurvePreferences);

impl Command for SubcurveCommand {
    fn name(&self) -> &'static str {
        "SubCrv"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_subcurve_options(arguments, self.0.get())?;
        self.0.update(
            Some(options.copy),
            Some(options.mode),
            Some(options.from_midpoint),
        );
        let mut candidates = document
            .selected_objects()
            .filter_map(|object| {
                object
                    .geometry()
                    .curve_ref()
                    .map(|curve| (object.id(), curve.to_owned()))
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(CommandError::SubcurveRequiresOneCurve {
                actual: candidates.len(),
            });
        }
        let (id, curve) = candidates.pop().expect("one subcurve source was required");
        let (geometry, start, end) = match options.location {
            SubcurveLocation::Numeric([anchor, length, confirmation]) => {
                let Some(piece) = if options.from_midpoint {
                    subcurve_input::midpoint_piece(
                        curve.as_ref(),
                        anchor,
                        length,
                        document.tolerance(),
                    )
                } else if let Some(forward) = options.locked_forward {
                    subcurve_input::locked_piece(
                        curve.as_ref(),
                        anchor,
                        length,
                        forward,
                        document.tolerance(),
                    )
                } else {
                    subcurve_input::piece(
                        curve.as_ref(),
                        anchor,
                        confirmation,
                        length,
                        document.tolerance(),
                    )
                }?
                else {
                    document.clear_selection();
                    return Ok("No subcurve created".into());
                };
                let domain = piece.as_ref().domain();
                (Geometry::from(piece), *domain.start(), *domain.end())
            }
            location => {
                let [start, end] = match location {
                    SubcurveLocation::Parameters(parameters) => parameters,
                    SubcurveLocation::Points(points) => [
                        curve
                            .as_ref()
                            .closest_parameter(points[0], document.tolerance())?,
                        curve
                            .as_ref()
                            .closest_parameter(points[1], document.tolerance())?,
                    ],
                    SubcurveLocation::Numeric(_) => unreachable!(),
                };
                if !options.from_midpoint
                    && !curve.as_ref().is_closed()?
                    && let Some(forward) = options.locked_forward
                    && (start == end || (end > start) != forward)
                {
                    document.clear_selection();
                    return Ok("No subcurve created on the locked side".into());
                }
                if options.from_midpoint {
                    let radius = subcurve_input::midpoint_radius(
                        curve.as_ref(),
                        start,
                        end,
                        document.tolerance(),
                    )?;
                    let Some(piece) = subcurve_input::midpoint_piece(
                        curve.as_ref(),
                        start,
                        radius,
                        document.tolerance(),
                    )?
                    else {
                        document.clear_selection();
                        return Ok("No subcurve created".into());
                    };
                    let domain = piece.as_ref().domain();
                    (Geometry::from(piece), *domain.start(), *domain.end())
                } else {
                    let [start, end] = if matches!(location, SubcurveLocation::Points(_))
                        && !curve.as_ref().is_closed()?
                        && start > end
                    {
                        [end, start]
                    } else {
                        [start, end]
                    };
                    (Geometry::from(curve.try_subcurve(start, end)?), start, end)
                }
            }
        };
        document.release_command_selection_on_history_replay([id])?;
        if options.mode == subcurve_input::SubcurveMode::MarkEnds {
            let curve = geometry.curve_ref().expect("subcurve result is a curve");
            let markers = [curve.start_point()?, curve.end_point()?];
            for point in markers {
                document.add_geometry(Geometry::Point(point))?;
            }
            document.clear_selection();
            return Ok("Marked both subcurve ends; retained the input".into());
        } else if options.copy {
            let outputs = document.copy_object_geometries_into_source_groups([(id, geometry)])?;
            document.select_command_results(outputs)?;
        } else {
            document.replace_object_geometries([(id, geometry)])?;
            document.clear_selection();
        }
        Ok(format!(
            "Created a directed subcurve from parameter {start} to {end}, {} the input",
            if options.copy {
                "retaining"
            } else {
                "replacing"
            }
        ))
    }
}

fn parse_subcurve_options(
    arguments: &[&str],
    defaults: subcurve_input::SubcurveDefaults,
) -> Result<SubcurveOptions, CommandError> {
    let Some(first) = arguments.first() else {
        return Err(CommandError::Usage(SUBCURVE_USAGE));
    };
    let first_name = first.split_once('=').map_or(*first, |(name, _)| name);
    let (location, mut index) = if option_name_eq(first_name, "Numeric") {
        let (_, value, consumed) = orient_option(arguments, 0, SUBCURVE_USAGE)?;
        let values = value.split(',').collect::<Vec<_>>();
        let [anchor, length, confirmation] = values.as_slice() else {
            return Err(CommandError::Usage(SUBCURVE_USAGE));
        };
        (
            SubcurveLocation::Numeric([
                parse_finite_real(anchor)?,
                parse_finite_real(length)?,
                parse_finite_real(confirmation)?,
            ]),
            consumed,
        )
    } else if option_name_eq(first_name, "Parameter") {
        let (name, value, consumed) = orient_option(arguments, 0, SUBCURVE_USAGE)?;
        if !option_name_eq(name, "Parameter") {
            return Err(CommandError::Usage(SUBCURVE_USAGE));
        }
        let values = value.split(',').collect::<Vec<_>>();
        if values.len() != 2 || values.iter().any(|value| value.is_empty()) {
            return Err(CommandError::Usage(SUBCURVE_USAGE));
        }
        (
            SubcurveLocation::Parameters([
                parse_finite_real(values[0])?,
                parse_finite_real(values[1])?,
            ]),
            consumed,
        )
    } else {
        let (start, start_consumed) = parse_point(arguments).map_err(|error| match error {
            CommandError::Usage(_) => CommandError::Usage(SUBCURVE_USAGE),
            error => error,
        })?;
        let (end, end_consumed) =
            parse_point(&arguments[start_consumed..]).map_err(|error| match error {
                CommandError::Usage(_) => CommandError::Usage(SUBCURVE_USAGE),
                error => error,
            })?;
        (
            SubcurveLocation::Points([start, end]),
            start_consumed + end_consumed,
        )
    };

    let mut copy = defaults.copy;
    let mut copy_seen = false;
    let mut mode = defaults.mode;
    let mut mode_seen = false;
    let mut from_midpoint = defaults.from_midpoint;
    let mut midpoint_seen = false;
    let mut locked_forward = None;
    let mut locked_seen = false;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, SUBCURVE_USAGE)?;
        if option_name_eq(name, "Locked") && !locked_seen {
            let value = value.trim_start_matches('_');
            locked_forward = Some(if value.eq_ignore_ascii_case("Forward") {
                true
            } else if value.eq_ignore_ascii_case("Backward") {
                false
            } else {
                return Err(CommandError::Usage(SUBCURVE_USAGE));
            });
            locked_seen = true;
            index += consumed;
            continue;
        }
        if option_name_eq(name, "FromMidpoint") && !midpoint_seen {
            from_midpoint = parse_yes_no(value).ok_or(CommandError::Usage(SUBCURVE_USAGE))?;
            midpoint_seen = true;
            index += consumed;
            continue;
        }
        if option_name_eq(name, "Mode") && !mode_seen {
            mode = subcurve_input::SubcurveMode::parse(value)
                .ok_or(CommandError::Usage(SUBCURVE_USAGE))?;
            mode_seen = true;
            index += consumed;
            continue;
        }
        if !option_name_eq(name, "Copy") || copy_seen {
            return Err(CommandError::Usage(SUBCURVE_USAGE));
        }
        copy = parse_yes_no(value).ok_or(CommandError::Usage(SUBCURVE_USAGE))?;
        copy_seen = true;
        index += consumed;
    }
    Ok(SubcurveOptions {
        location,
        copy,
        mode,
        from_midpoint,
        locked_forward,
    })
}

pub(super) const REPARAMETERIZE_USAGE: &str = "Reparameterize Automatic | Reparameterize start end | Reparameterize u_start u_end v_start v_end";

#[derive(Clone, Debug, PartialEq)]
enum ReparameterizeOptions {
    Automatic,
    Explicit(Vec<Real>),
}

enum ReparameterizeTarget {
    Curve(Curve3),
    Surface(NurbsSurface),
}

pub(super) struct ReparameterizeCommand;

impl Command for ReparameterizeCommand {
    fn name(&self) -> &'static str {
        "Reparameterize"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_reparameterize_options(arguments)?;
        let mut candidates = document
            .selected_objects()
            .filter_map(|object| {
                let target = match object.geometry() {
                    Geometry::NurbsSurface(surface) => {
                        Some(ReparameterizeTarget::Surface(surface.clone()))
                    }
                    geometry => geometry
                        .curve_ref()
                        .map(|curve| ReparameterizeTarget::Curve(curve.to_owned())),
                };
                target.map(|target| (object.id(), target))
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(CommandError::ReparameterizeRequiresOneObject {
                actual: candidates.len(),
            });
        }
        let (id, target) = candidates
            .pop()
            .expect("one reparameterization target was required");

        let (geometry, description) = match (target, options) {
            (ReparameterizeTarget::Curve(curve), ReparameterizeOptions::Automatic) => {
                let domain_end = curve.as_ref().length(document.tolerance())?;
                let curve =
                    reparameterize_native_curve(&curve, 0.0..=domain_end, document.tolerance())?;
                (
                    Geometry::from(curve),
                    format!("curve domain to 0..{domain_end}"),
                )
            }
            (ReparameterizeTarget::Curve(curve), ReparameterizeOptions::Explicit(values))
                if values.len() == 2 =>
            {
                let [start, end] = [values[0], values[1]];
                let curve = reparameterize_native_curve(&curve, start..=end, document.tolerance())?;
                (
                    Geometry::from(curve),
                    format!("curve domain to {start}..{end}"),
                )
            }
            (ReparameterizeTarget::Surface(surface), ReparameterizeOptions::Automatic) => {
                let [width, height] = surface.estimated_size()?;
                let surface = surface.try_reparameterized(0.0..=width, 0.0..=height)?;
                (
                    Geometry::NurbsSurface(surface),
                    format!("surface domains to U 0..{width}, V 0..{height}"),
                )
            }
            (ReparameterizeTarget::Surface(surface), ReparameterizeOptions::Explicit(values))
                if values.len() == 4 =>
            {
                let [u_start, u_end, v_start, v_end] = [values[0], values[1], values[2], values[3]];
                let surface = surface.try_reparameterized(u_start..=u_end, v_start..=v_end)?;
                (
                    Geometry::NurbsSurface(surface),
                    format!("surface domains to U {u_start}..{u_end}, V {v_start}..{v_end}"),
                )
            }
            _ => return Err(CommandError::Usage(REPARAMETERIZE_USAGE)),
        };
        document.replace_object_geometries([(id, geometry)])?;
        Ok(format!("Reparameterized the selected {description}"))
    }
}

fn reparameterize_native_curve(
    curve: &Curve3,
    domain: std::ops::RangeInclusive<Real>,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    match curve {
        Curve3::PolyCurve(c) => Ok(Curve3::PolyCurve(
            c.try_reparameterized_by_length(domain, tolerance)?,
        )),
        _ => curve.try_reparameterized(domain),
    }
}

fn parse_reparameterize_options(arguments: &[&str]) -> Result<ReparameterizeOptions, CommandError> {
    if let [argument] = arguments {
        let automatic = argument.trim_start_matches(['_', '-']);
        if automatic.eq_ignore_ascii_case("Automatic") {
            return Ok(ReparameterizeOptions::Automatic);
        }
    }
    if arguments.iter().any(|argument| {
        argument
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Automatic")
    }) {
        return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
    }

    let raw_values = arguments
        .iter()
        .flat_map(|argument| argument.split(','))
        .collect::<Vec<_>>();
    if !matches!(raw_values.len(), 2 | 4) || raw_values.iter().any(|value| value.is_empty()) {
        return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
    }
    Ok(ReparameterizeOptions::Explicit(
        raw_values
            .into_iter()
            .map(parse_finite_real)
            .collect::<Result<_, _>>()?,
    ))
}

#[cfg(test)]
mod tests;
