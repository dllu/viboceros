//! Positional Pipe execution and option compatibility.
use super::*;

impl Command for PipeCommand {
    fn name(&self) -> &'static str {
        "Pipe"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (source_id, arguments) = if let Some(id) = arguments
            .first()
            .and_then(|value| value.parse::<ObjectId>().ok())
        {
            (id, &arguments[1..])
        } else {
            let selected = document.selected_object_ids().collect::<Vec<_>>();
            let [id] = selected.as_slice() else {
                return Err(CommandError::Usage(USAGE));
            };
            (*id, arguments)
        };
        let Some(start_radius) = arguments.first() else {
            return Err(CommandError::Usage(USAGE));
        };
        let start_radius = positive_radius(start_radius)?;
        let mut next = 1;
        let end_radius = if let Some(value) = arguments.get(next)
            && !value.contains('=')
        {
            next += 1;
            positive_radius(value)?
        } else {
            start_radius
        };
        let mut cap = None;
        let mut output = None;
        let mut blend = None;
        let mut fit_rail = None;
        let mut thick = None;
        let mut wall_thickness = None;
        let mut stations = None;
        for argument in &arguments[next..] {
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            if option_name_eq(name, "Output") && output.is_none() {
                output = Some(Output::parse(value)?);
            } else if option_name_eq(name, "Cap") && cap.is_none() {
                cap = Some(
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "none" => PipeCap::None,
                        "flat" => PipeCap::Flat,
                        "round" => PipeCap::Round,
                        _ => return Err(CommandError::Usage(USAGE)),
                    },
                );
            } else if option_name_eq(name, "ShapeBlending") && blend.is_none() {
                blend = Some(
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "local" => SweepBlend::Local,
                        "global" => SweepBlend::Global,
                        _ => return Err(CommandError::Usage(USAGE)),
                    },
                );
            } else if option_name_eq(name, "FitRail") && fit_rail.is_none() {
                fit_rail = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            } else if option_name_eq(name, "Thick") && thick.is_none() {
                thick = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            } else if option_name_eq(name, "WallThickness") && wall_thickness.is_none() {
                let value = value
                    .parse::<Real>()
                    .map_err(|_| CommandError::InvalidNumber(value.to_owned()))?;
                if !value.is_finite() || value == 0.0 {
                    return Err(CommandError::Usage(USAGE));
                }
                wall_thickness = Some(value);
            } else if option_name_eq(name, "Stations") && stations.is_none() {
                let parsed = value
                    .split(',')
                    .map(|station| {
                        let (fraction, radius) =
                            station.split_once(':').ok_or(CommandError::Usage(USAGE))?;
                        let fraction = fraction
                            .parse::<Real>()
                            .map_err(|_| CommandError::InvalidNumber(fraction.to_owned()))?;
                        if !fraction.is_finite() || fraction <= 0.0 || fraction >= 1.0 {
                            return Err(CommandError::Usage(USAGE));
                        }
                        Ok((fraction, positive_radius(radius)?))
                    })
                    .collect::<Result<Vec<_>, CommandError>>()?;
                if parsed.is_empty()
                    || parsed.len() > 254
                    || parsed.windows(2).any(|pair| pair[0].0 >= pair[1].0)
                {
                    return Err(CommandError::Usage(USAGE));
                }
                stations = Some(parsed);
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        let cap = cap.unwrap_or(PipeCap::Flat);
        let blend = blend.unwrap_or(SweepBlend::Local);
        let fit_rail = fit_rail.unwrap_or(false);
        if thick == Some(false) && wall_thickness.is_some()
            || thick == Some(true) && wall_thickness.is_none()
        {
            return Err(CommandError::Usage(USAGE));
        }
        let second_radii = wall_thickness
            .map(|thickness| {
                let first = start_radius + thickness;
                let second = end_radius + thickness;
                if !first.is_finite() || !second.is_finite() || first <= 0.0 || second <= 0.0 {
                    return Err(CommandError::Usage(USAGE));
                }
                Ok([first, second])
            })
            .transpose()?;
        let stations = stations.unwrap_or_default();
        if let Some(thickness) = wall_thickness
            && stations
                .iter()
                .any(|(_, radius)| radius + thickness <= 0.0 || !(radius + thickness).is_finite())
        {
            return Err(CommandError::Usage(USAGE));
        }
        // Rhino 8's double-wall Pipe uses planar annular caps for Round too.
        let cap = if second_radii.is_some() && cap == PipeCap::Round {
            PipeCap::Flat
        } else {
            cap
        };
        let options = PipeOptions {
            blend,
            fit_rail,
            cap,
            wall_thickness,
        };
        if !document.is_object_selectable(source_id) {
            return Err(CommandError::Usage(USAGE));
        }
        let source = document
            .object(source_id)
            .map(|object| object.geometry())
            .ok_or(CommandError::Usage(USAGE))?;
        let profile = Profile::new([start_radius, end_radius], stations);
        let tolerance = document.tolerance();
        if output == Some(Output::SubD) {
            let rail = source.curve_ref().ok_or(CommandError::Usage(USAGE))?;
            let second = wall_thickness.zip(second_radii).map(|(offset, endpoints)| {
                Profile::new(
                    endpoints,
                    profile
                        .stations
                        .iter()
                        .map(|&(f, r)| (f, r + offset))
                        .collect(),
                )
            });
            let outputs = subd::build_walls(rail, &profile, second.as_ref(), options, tolerance)?;
            let count = outputs.len();
            for geometry in outputs {
                document.add_geometry(geometry)?;
            }
            return Ok(format!("Added {count} SubD pipe object(s)"));
        }
        match build(source, &profile, options, self.0.get().0, tolerance)? {
            BuiltPipe::Straight(outputs) => {
                let count = outputs.len();
                for brep in outputs {
                    document.add_geometry(Geometry::Brep(brep))?;
                }
                Ok(format!(
                    "Added {count} {} pipe object(s)",
                    if cap == PipeCap::Flat {
                        "closed"
                    } else {
                        "open"
                    }
                ))
            }
            BuiltPipe::Sweep(result) => {
                let closed = matches!(&result, Geometry::Brep(brep) if brep.is_closed())
                    || matches!(&result, Geometry::NurbsSurface(surface) if surface.is_closed_u()? && surface.is_closed_v()?);
                let id = document.add_geometry(result)?;
                Ok(format!(
                    "Added {} pipe {id} (start radius {start_radius:.6}, end radius {end_radius:.6})",
                    if closed { "closed" } else { "open" }
                ))
            }
        }
    }
}

fn positive_radius(value: &str) -> Result<Real, CommandError> {
    let radius = value
        .parse::<Real>()
        .map_err(|_| CommandError::InvalidNumber(value.to_owned()))?;
    if !radius.is_finite() || radius <= 0.0 {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(radius)
}
