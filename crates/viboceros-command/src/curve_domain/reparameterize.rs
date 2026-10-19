//! Curve and trimmed-face parameter charts, retaining document identity.
use super::*;

#[cfg(test)]
mod tests;

pub(crate) const REPARAMETERIZE_USAGE: &str = "Reparameterize [Surface=id] [Face=index] Automatic | Reparameterize start end | Reparameterize [Surface=id] [Face=index] u_start u_end v_start v_end";

#[derive(Clone, Debug, PartialEq)]
enum ReparameterizeOptions {
    Automatic,
    Explicit(Vec<Real>),
}

enum ReparameterizeTarget {
    Curve(Curve3),
    Surface(NurbsSurface),
    Face(Brep, usize),
}

pub(crate) struct ReparameterizeCommand;

impl Command for ReparameterizeCommand {
    fn name(&self) -> &'static str {
        "Reparameterize"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (options, source, face) = parse_arguments(arguments)?;
        let objects = if let Some(id) = source {
            vec![
                document
                    .object(id)
                    .filter(|_| document.is_object_selectable(id))
                    .ok_or(CommandError::Usage(REPARAMETERIZE_USAGE))?,
            ]
        } else {
            document.selected_objects().collect()
        };
        let mut candidates = Vec::new();
        for object in objects {
            let target = match object.geometry() {
                Geometry::NurbsSurface(surface) if face.is_none_or(|i| i == 0) => {
                    Some(ReparameterizeTarget::Surface(surface.clone()))
                }
                Geometry::Brep(brep) => {
                    let index = face.or_else(|| (brep.faces().len() == 1).then_some(0));
                    match index {
                        Some(index) if index < brep.faces().len() => {
                            Some(ReparameterizeTarget::Face(brep.clone(), index))
                        }
                        Some(_) => return Err(CommandError::Usage(REPARAMETERIZE_USAGE)),
                        None => None,
                    }
                }
                geometry if source.is_none() && face.is_none() => geometry
                    .curve_ref()
                    .map(|curve| ReparameterizeTarget::Curve(curve.to_owned())),
                _ => None,
            };
            if let Some(target) = target {
                candidates.push((object.id(), target));
            }
        }
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
                let [width, height] = automatic_surface_size(&surface, document.tolerance())?;
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
            (ReparameterizeTarget::Face(brep, index), options) => {
                let values = match options {
                    ReparameterizeOptions::Automatic => {
                        let [width, height] = automatic_surface_size(
                            brep.faces()[index].surface(),
                            document.tolerance(),
                        )?;
                        [0., width, 0., height]
                    }
                    ReparameterizeOptions::Explicit(v) if v.len() == 4 => [v[0], v[1], v[2], v[3]],
                    _ => return Err(CommandError::Usage(REPARAMETERIZE_USAGE)),
                };
                let [u_start, u_end, v_start, v_end] = values;
                let brep = brep.try_reparameterized_face_uv(
                    [(index, [u_start..=u_end, v_start..=v_end])],
                    document.tolerance(),
                )?;
                (
                    Geometry::Brep(brep),
                    format!("surface domains to U {u_start}..{u_end}, V {v_start}..{v_end}"),
                )
            }
            _ => return Err(CommandError::Usage(REPARAMETERIZE_USAGE)),
        };
        let text = document.object(id).unwrap().geometry_user_text().clone();
        document.replace_object_geometries([(id, geometry)])?;
        for (key, value) in text {
            document.set_object_geometry_user_text([id], &key, Some(&value))?;
        }
        Ok(format!("Reparameterized the selected {description}"))
    }
}

fn automatic_surface_size(
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<[Real; 2], GeometryError> {
    use viboceros_geometry::{ArcLengthSampler, CurveRef, FiniteSum, SurfaceExtensionEdge as Edge};
    let mut lengths = [[0.; 11]; 2];
    let [u_lengths, v_lengths] = &mut lengths;
    for (i, (u_length, v_length)) in u_lengths.iter_mut().zip(v_lengths).enumerate() {
        let fraction = i as Real / 10.;
        let curves = [
            surface.isocurve_u(surface.parameter_at_v(fraction)?)?,
            surface.isocurve_v(surface.parameter_at_u(fraction)?)?,
        ];
        for (axis, (curve, length)) in curves.iter().zip([u_length, v_length]).enumerate() {
            let boundary = match (axis, i) {
                (0, 0) => Some(Edge::South),
                (0, 10) => Some(Edge::North),
                (1, 0) => Some(Edge::West),
                (1, 10) => Some(Edge::East),
                _ => None,
            };
            if boundary.is_some_and(|edge| surface.has_coincident_boundary_controls(edge)) {
                continue;
            }
            // Match the command's default length query through the existing
            // bounded Cash–Karp trace. The general precise length API and
            // caller's geometry acceptance tolerance remain independent.
            *length = match ArcLengthSampler::try_new(CurveRef::NurbsCurve(curve), tolerance) {
                Ok(sampler) => sampler.quadrature_length_estimate(1e-8)?,
                Err(GeometryError::Degenerate {
                    context: "arc-length curve",
                }) if curve.length(tolerance)? == 0. => 0.,
                Err(error) => return Err(error),
            };
        }
    }
    let mut size = [0.; 2];
    for (axis, lengths) in lengths.iter().enumerate() {
        let maximum = lengths.iter().copied().fold(0_f64, Real::max);
        if maximum > 0. {
            let mut sum = FiniteSum::default();
            let mut count = 0;
            for length in lengths.iter().filter(|&&length| length > 0.) {
                sum.add(length / maximum)?;
                count += 1;
            }
            size[axis] = (sum.total()? / count as Real) * maximum;
        }
    }
    Ok(size)
}

fn parse_arguments(
    arguments: &[&str],
) -> Result<(ReparameterizeOptions, Option<ObjectId>, Option<usize>), CommandError> {
    let mut source = None;
    let mut face = None;
    let mut values = Vec::new();
    for argument in arguments {
        match argument.split_once('=') {
            Some((name, value)) if option_name_eq(name, "Surface") && source.is_none() => {
                source = Some(
                    value
                        .parse::<ObjectId>()
                        .map_err(|_| CommandError::Usage(REPARAMETERIZE_USAGE))?,
                );
            }
            Some((name, value)) if option_name_eq(name, "Face") && face.is_none() => {
                face = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| CommandError::Usage(REPARAMETERIZE_USAGE))?,
                );
            }
            _ => values.push(*argument),
        }
    }
    Ok((parse_reparameterize_options(&values)?, source, face))
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
