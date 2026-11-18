//! Solid Ellipsoid reuses the planar ellipse getter and adds its normal-axis measurement.

use crate::{
    Command, CommandContext, CommandError, CommandRegistry,
    curve_reference::{CapturedCurve, parse_edge, perpendicular_frame},
    ellipse_input::{Input as EllipseInput, Mode, Slot},
    option_name_eq, parse_ellipsoid_positionals, parse_point,
    remembered::Remembered,
    translation::DestinationConstraint,
};
use std::sync::Arc;
use viboceros_document::{Document, Geometry, ObjectId, SelectionMode};
use viboceros_drafting::PointInput;
use viboceros_geometry::{Ellipse3, Frame3, GeometryError, NurbsSurface, Point3, Real, Tolerance};

const USAGE: &str = "Ellipsoid [Corner | Diameter [Vertical] | FromFoci | AroundCurve] base-input [third-axis-size-or-point]";

pub fn base_option(input: &mut EllipseInput, word: &str) -> Result<bool, CommandError> {
    let key = word
        .split('=')
        .next()
        .unwrap_or(word)
        .trim_start_matches(['_', '-']);
    if ["Deformable", "Degree", "PointCount"]
        .into_iter()
        .any(|name| key.eq_ignore_ascii_case(name))
        || key.eq_ignore_ascii_case("Vertical") && input.mode() != Mode::Diameter
    {
        return Err(CommandError::Usage(USAGE));
    }
    if option_name_eq(word, "Radius") {
        return Ok(input.option("Default")?);
    }
    Ok(input.option(word)?)
}

#[derive(Clone, Copy, Debug)]
pub struct ThirdAxis {
    pub base: Ellipse3,
    frame: Frame3,
    diameter: bool,
    tolerance: Tolerance,
}

impl ThirdAxis {
    pub fn new(base: Ellipse3, mode: Mode, tolerance: Tolerance) -> Result<Self, GeometryError> {
        Ok(Self {
            frame: Frame3::try_from_unit_directions(
                base.center(),
                base.x_axis(),
                base.y_axis(),
                Tolerance::NUMERICAL_VALIDATION,
            )?,
            base,
            diameter: mode == Mode::Diameter,
            tolerance,
        })
    }

    pub fn axis_constraint(self) -> DestinationConstraint {
        DestinationConstraint {
            anchor: self.frame.origin(),
            direction: Some(self.frame.z_axis()),
            distance: None,
        }
    }

    pub fn point(self, point: Point3) -> Result<Definition, GeometryError> {
        let radius = self
            .frame
            .origin()
            .vector_to(point)?
            .dot(self.frame.z_axis().as_vector())?;
        self.definition(radius.abs())
    }

    pub fn number(self, size: Real) -> Result<Definition, GeometryError> {
        self.definition(size.abs() * if self.diameter { 0.5 } else { 1.0 })
    }

    fn definition(self, radius: Real) -> Result<Definition, GeometryError> {
        if !radius.is_finite() || radius <= self.tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "ellipsoid third-axis radius must exceed document tolerance",
            });
        }
        Ok(Definition {
            frame: self.frame,
            radii: [self.base.radius_x(), self.base.radius_y(), radius],
            foci: None,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Definition {
    frame: Frame3,
    radii: [Real; 3],
    pub foci: Option<[Point3; 2]>,
}

impl Definition {
    pub fn focal(
        input: EllipseInput,
        base: Ellipse3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut definition =
            ThirdAxis::new(base, Mode::FromFoci, tolerance)?.number(base.radius_y())?;
        definition.foci = input.picked_foci();
        Ok(definition)
    }

    pub fn surface(self) -> Result<NurbsSurface, GeometryError> {
        let [first, second, third] = self.radii;
        // Rhino puts the surface poles on the first authored axis. The second
        // authored axis is the first equator point and the normal is the next.
        let frame = Frame3::try_from_unit_directions(
            self.frame.origin(),
            self.frame.y_axis(),
            self.frame.z_axis(),
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        NurbsSurface::try_ellipsoid(frame, [second, third, first])
    }

    pub fn sections(self) -> [Result<Ellipse3, GeometryError>; 3] {
        let [x, y, z] = self.frame.axes();
        let [a, b, c] = self.radii;
        [(a, b, x, y), (a, c, x, z), (b, c, y, z)].map(|(rx, ry, x, y)| {
            Ellipse3::try_new(
                self.frame.origin(),
                rx,
                ry,
                x,
                y,
                Tolerance::NUMERICAL_VALIDATION,
            )
        })
    }
}

pub(super) struct EllipsoidCommand(pub(super) Arc<Remembered<bool>>);

impl CommandRegistry {
    pub fn ellipsoid_mark_foci(&self) -> bool {
        self.ellipsoid_mark_foci.get()
    }

    pub fn remember_ellipsoid_mark_foci(&self, mark: bool) {
        self.ellipsoid_mark_foci.set(mark);
    }
}

impl Command for EllipsoidCommand {
    fn name(&self) -> &'static str {
        "Ellipsoid"
    }

    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }

    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        if let Ok(positional) = parse_ellipsoid_positionals(args, doc.tolerance())
            && positional.option_start == args.len()
        {
            return add(
                doc,
                Definition {
                    frame: positional.frame,
                    radii: positional.radii,
                    foci: None,
                },
                None,
            );
        }
        let (definition, release, mark_foci) =
            from_arguments(doc, args, context.construction_plane, self.0.get())?;
        let message = add(doc, definition, release)?;
        self.0.set(mark_foci);
        Ok(message)
    }
}

fn from_arguments(
    doc: &Document,
    mut args: &[&str],
    plane: Frame3,
    mark_foci: bool,
) -> Result<(Definition, Option<ObjectId>, bool), CommandError> {
    let mut input = EllipseInput::new(
        crate::ellipse_input::Options {
            mark_foci,
            ..Default::default()
        },
        plane,
        doc.tolerance(),
    );
    let mut release = None;
    while let Some((&word, rest)) = args.split_first() {
        if base_option(&mut input, word)? {
            args = rest;
        } else if input.requests_curve() {
            let (key, value) = word.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            let (id, edge) = if option_name_eq(key, "Edge") {
                let (id, edge) = parse_edge(value).ok_or(CommandError::Usage(USAGE))?;
                (id, Some(edge))
            } else if option_name_eq(key, "Curve") || option_name_eq(key, "PickedCurve") {
                (value.parse().map_err(|_| CommandError::Usage(USAGE))?, None)
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            let source = CapturedCurve::capture(doc, id, edge)?;
            let (center, count) = parse_point_or_origin(rest, plane)?;
            input.around_curve(perpendicular_frame(source.curve_ref(), center)?);
            if edge.is_none() && option_name_eq(key, "PickedCurve") {
                release = Some(id);
            }
            args = &rest[count..];
        } else if input.slot() == Slot::First || word.contains(',') || coordinate_triple(args) {
            let (point, count) = parse_point_or_origin(args, plane)?;
            args = &args[count..];
            if let Some(base) = input.point(point)? {
                let definition = if input.mode() == Mode::FromFoci {
                    if !args.is_empty() {
                        return Err(CommandError::Usage(USAGE));
                    }
                    Definition::focal(input, base.ellipse, doc.tolerance())?
                } else {
                    let axis = ThirdAxis::new(base.ellipse, input.mode(), doc.tolerance())?;
                    match args {
                        [size] if !size.contains(',') => axis.number(length(doc, size)?)?,
                        _ => {
                            let (point, consumed) = parse_point(args)?;
                            if consumed != args.len() {
                                return Err(CommandError::Usage(USAGE));
                            }
                            axis.point(point)?
                        }
                    }
                };
                return Ok((definition, release, input.options().mark_foci));
            }
        } else {
            input.number(length(doc, word)?.abs())?;
            args = rest;
        }
    }
    Err(CommandError::Usage(USAGE))
}

fn coordinate_triple(args: &[&str]) -> bool {
    // Three bare components can follow a construction mode. A lone length
    // followed by a comma point remains an axis-length lock.
    args.get(..3)
        .is_some_and(|tokens| tokens.iter().all(|word| word.parse::<Real>().is_ok()))
}

fn parse_point_or_origin(args: &[&str], plane: Frame3) -> Result<(Point3, usize), CommandError> {
    if args.first() == Some(&"0") && !coordinate_triple(args) {
        Ok((plane.origin(), 1))
    } else {
        parse_point(args)
    }
}

fn length(doc: &Document, text: &str) -> Result<Real, CommandError> {
    PointInput::parse_length_with_units(text, doc.units())
        .and_then(Result::ok)
        .ok_or_else(|| CommandError::InvalidNumber(text.into()))
}

pub fn commit(
    doc: &mut Document,
    definition: Definition,
    release: Option<ObjectId>,
) -> Result<String, CommandError> {
    doc.begin_transaction("Ellipsoid")?;
    let result = add(doc, definition, release).and_then(|message| {
        doc.commit_transaction()?;
        Ok(message)
    });
    if result.is_err() {
        doc.rollback_transaction()?;
    }
    result
}

fn add(
    doc: &mut Document,
    definition: Definition,
    release: Option<ObjectId>,
) -> Result<String, CommandError> {
    let surface = definition.surface()?;
    if let Some(id) = release {
        doc.release_command_selection_on_history_replay([id])?;
        doc.select_objects_direct([id], SelectionMode::Remove)?;
    }
    let id = doc.add_geometry(Geometry::NurbsSurface(surface))?;
    for point in definition.foci.into_iter().flatten() {
        doc.add_geometry(Geometry::Point(point))?;
    }
    let [x, y, z] = definition.radii;
    Ok(format!(
        "Added NURBS ellipsoid {id} (radii {x:.6} × {y:.6} × {z:.6})"
    ))
}

#[cfg(test)]
mod tests;
