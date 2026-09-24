//! Circular-profile pipe construction from a selected centerline curve.

use super::*;
use viboceros_geometry::{
    Circle3, Frame3, FrameTransportOptions, NurbsSurface, Sweep1, SweepBlend, SweepFrameStyle,
    SweepSection,
};

const USAGE: &str =
    "Pipe [curve-id] start-radius [end-radius] [Cap=None|Flat] [ShapeBlending=Local|Global]";

pub(super) struct PipeCommand;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PipeCap {
    None,
    Flat,
}

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
        let mut blend = None;
        for argument in &arguments[next..] {
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            if option_name_eq(name, "Cap") && cap.is_none() {
                cap = Some(
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "none" => PipeCap::None,
                        "flat" => PipeCap::Flat,
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
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        let cap = cap.unwrap_or(PipeCap::Flat);
        let blend = blend.unwrap_or(SweepBlend::Local);
        if !document.is_object_selectable(source_id) {
            return Err(CommandError::Usage(USAGE));
        }
        let source = document
            .object(source_id)
            .map(|object| object.geometry())
            .ok_or(CommandError::Usage(USAGE))?;
        let rail = source.curve_ref().ok_or(CommandError::Usage(USAGE))?;
        let tolerance = document.tolerance();
        let result = match source {
            Geometry::Line(line) => {
                let frame = Frame3::try_from_normal(
                    line.start(),
                    line.start().vector_to(line.end())?,
                    tolerance,
                )?;
                let height = line.length()?;
                let constant = start_radius == end_radius;
                match (cap, constant) {
                    (PipeCap::Flat, true) => Geometry::Brep(Brep::try_cylinder(
                        frame,
                        start_radius,
                        0.0,
                        height,
                        tolerance,
                    )?),
                    (PipeCap::Flat, false) => Geometry::Brep(Brep::try_truncated_cone(
                        frame,
                        [start_radius, end_radius],
                        height,
                        tolerance,
                    )?),
                    (PipeCap::None, true) => Geometry::NurbsSurface(NurbsSurface::try_cylinder(
                        frame,
                        start_radius,
                        0.0,
                        height,
                    )?),
                    (PipeCap::None, false) => {
                        Geometry::NurbsSurface(NurbsSurface::try_truncated_cone(
                            frame,
                            [start_radius, end_radius],
                            height,
                        )?)
                    }
                }
            }
            Geometry::Circle(circle) => {
                if start_radius != end_radius || start_radius >= circle.radius() {
                    return Err(CommandError::Usage(USAGE));
                }
                let frame = Frame3::try_from_x_and_normal(
                    circle.center(),
                    circle.x_axis().as_vector(),
                    circle.normal()?.as_vector(),
                    tolerance,
                )?;
                Geometry::NurbsSurface(NurbsSurface::try_torus(
                    frame,
                    circle.radius(),
                    start_radius,
                )?)
            }
            _ => {
                if rail.is_closed()? {
                    return Err(CommandError::Usage(USAGE));
                }
                let domain = rail.domain();
                let parameters = [*domain.start(), *domain.end()];
                let angular_tolerance =
                    (0.05 * (tolerance.absolute() / start_radius.max(end_radius))).min(1e-10);
                let frames = rail.rotation_minimizing_frames(
                    &parameters,
                    None,
                    FrameTransportOptions {
                        angular_tolerance,
                        ..Default::default()
                    },
                )?;
                let mut sections = vec![circular_section(
                    parameters[0],
                    frames[0],
                    start_radius,
                    tolerance,
                )?];
                if start_radius != end_radius {
                    sections.push(circular_section(
                        parameters[1],
                        frames[1],
                        end_radius,
                        tolerance,
                    )?);
                }
                let sweep =
                    Sweep1::try_new(rail, &sections, SweepFrameStyle::Freeform, blend, tolerance)?;
                let surface = sweep.to_rail_basis_surface()?;
                let [u, v] = surface.sampled_kink_parameters(tolerance.angular())?;
                let wall = Brep::try_surface_grid(&surface, &u, &v, tolerance)?;
                let result = if cap == PipeCap::Flat {
                    let capped = wall
                        .try_cap_planar_holes(tolerance)?
                        .ok_or(CommandError::Usage(USAGE))?;
                    if !capped.is_closed() {
                        return Err(CommandError::Usage(USAGE));
                    }
                    capped
                } else {
                    wall
                };
                Geometry::Brep(result)
            }
        };
        let closed = matches!(&result, Geometry::Brep(brep) if brep.is_closed())
            || matches!(&result, Geometry::NurbsSurface(surface) if surface.is_closed_u()? && surface.is_closed_v()?);
        let id = document.add_geometry(result)?;
        Ok(format!(
            "Added {} pipe {id} (start radius {start_radius:.6}, end radius {end_radius:.6})",
            if closed { "closed" } else { "open" }
        ))
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

fn circular_section(
    parameter: Real,
    frame: Frame3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<SweepSection, CommandError> {
    let circle = Circle3::try_from_frame(
        frame.origin(),
        radius,
        frame.x_axis(),
        frame.z_axis(),
        tolerance,
    )?;
    Ok(SweepSection {
        parameter,
        curve: circle.to_nurbs()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{CircularArc3, LineSegment, UnitVector3};

    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn straight_pipe_has_exact_cylinder_and_frustum_outputs() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut document, &format!("Pipe {source} 1"))
            .unwrap();
        let Geometry::Brep(cylinder) = document.objects().last().unwrap().geometry() else {
            panic!("flat pipe should be a B-rep")
        };
        assert!(cylinder.is_closed());
        assert_eq!(cylinder.faces().len(), 3);
        registry
            .execute(&mut document, &format!("Pipe {source} 1 2 Cap=None"))
            .unwrap();
        assert!(matches!(
            document.objects().last().unwrap().geometry(),
            Geometry::NurbsSurface(_)
        ));
        registry
            .execute(&mut document, &format!("Pipe {source} 1 2 Cap=Flat"))
            .unwrap();
        let Geometry::Brep(frustum) = document.objects().last().unwrap().geometry() else {
            panic!("flat tapered pipe should be a B-rep")
        };
        assert!(frustum.is_closed());
        assert_eq!(frustum.faces().len(), 3);
    }

    #[test]
    fn circle_rail_makes_closed_torus_and_preserves_source() {
        let mut document = Document::default();
        let circle = Circle3::try_from_frame(
            p(4., -2., 1.),
            3.,
            UnitVector3::try_new(1., 0., 0., Tolerance::DEFAULT).unwrap(),
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Circle(circle)).unwrap();
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut document, "Pipe 0.5").unwrap();
        let Geometry::NurbsSurface(surface) = document.objects().last().unwrap().geometry() else {
            panic!("circle rail should create a torus surface")
        };
        assert!(surface.is_closed_u().unwrap());
        assert!(surface.is_closed_v().unwrap());
        assert_eq!(document.objects().count(), 2);
    }

    #[test]
    fn smooth_arc_rail_builds_capped_and_open_pipe() {
        let mut document = Document::default();
        let diagonal = 5. * std::f64::consts::FRAC_1_SQRT_2;
        let arc = CircularArc3::try_from_three_points(
            p(5., 0., 0.),
            p(diagonal, diagonal, 0.),
            p(0., 5., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Arc(arc)).unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut document, &format!("Pipe {source} 0.5"))
            .unwrap();
        let Geometry::Brep(capped) = document.objects().last().unwrap().geometry() else {
            panic!("arc pipe should be a B-rep")
        };
        assert!(capped.is_closed());
        let wall = capped.faces()[0].surface();
        let u = (*wall.domain_u().start() + *wall.domain_u().end()) * 0.5;
        let v = (*wall.domain_v().start() + *wall.domain_v().end()) * 0.5;
        let sample = wall.evaluate(u, v).unwrap();
        let rail = CurveRef::Arc(&arc);
        let nearest = rail
            .evaluate(rail.closest_parameter(sample, Tolerance::DEFAULT).unwrap())
            .unwrap();
        assert!((sample.distance_to(nearest).unwrap() - 0.5).abs() < 5e-3);
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.5 0.8 Cap=None ShapeBlending=Global"),
            )
            .unwrap();
        let Geometry::Brep(open) = document.objects().last().unwrap().geometry() else {
            panic!("arc pipe should be a B-rep")
        };
        assert!(!open.is_closed());
        let wall = open.faces()[0].surface();
        let u = (*wall.domain_u().start() + *wall.domain_u().end()) * 0.5;
        let v = (*wall.domain_v().start() + *wall.domain_v().end()) * 0.5;
        let sample = wall.evaluate(u, v).unwrap();
        let nearest = rail
            .evaluate(rail.closest_parameter(sample, Tolerance::DEFAULT).unwrap())
            .unwrap();
        assert!((sample.distance_to(nearest).unwrap() - 0.65).abs() < 5e-3);
    }

    #[test]
    fn invalid_inputs_leave_document_unchanged() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        let originals = document.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        for command in [
            "Pipe",
            "Pipe 0",
            "Pipe -1",
            "Pipe NaN",
            "Pipe 1 Cap=Round",
            "Pipe 1 Cap=Flat Cap=None",
            "Pipe 1 ShapeBlending=Other",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), originals);
        }
    }
}
