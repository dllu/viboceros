//! Registry entry points and compatibility with explicit positional axes.
use super::*;
use crate::{
    Command, CommandContext, axial_primitive_frame, parse_axial_primitive_options,
    remembered::Remembered,
};
use std::sync::Arc;
use viboceros_document::Document;

pub(crate) struct ConeCommand(pub(crate) Arc<Remembered<ConeSettings>>);

impl Command for ConeCommand {
    fn name(&self) -> &'static str {
        Kind::Cone.name()
    }

    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }

    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        ctx: CommandContext,
    ) -> Result<String, CommandError> {
        let (message, preferences) = run(Kind::Cone, doc, args, ctx, self.0.get().0, true)?;
        self.0.set(ConeSettings(preferences));
        Ok(message)
    }
}

fn legacy_cone(d: Definition, tolerance: Tolerance) -> Result<Geometry, CommandError> {
    if d.solid {
        Ok(Geometry::Brep(Brep::try_cone(
            d.frame, d.radius, d.height, tolerance,
        )?))
    } else {
        let apex = d.frame.point_at([0., 0., d.height])?;
        Ok(Geometry::NurbsSurface(NurbsSurface::try_cone(
            d.frame.with_origin(apex),
            d.radius,
            -d.height,
        )?))
    }
}

pub(crate) fn run(
    kind: Kind,
    doc: &mut Document,
    args: &[&str],
    ctx: CommandContext,
    mut preferences: Preferences,
    use_extrusions: bool,
) -> Result<(String, Preferences), CommandError> {
    let tolerance = doc.tolerance();
    let cylinder = kind == Kind::Cylinder;
    let (def, geometry) =
        if let Ok(opts) = parse_axial_primitive_options(args, kind.usage(), cylinder) {
            let (frame, radius) =
                axial_primitive_frame(opts.center, opts.radius, opts.axis, tolerance)?;
            let solid = if cylinder
                && !args.iter().any(|token| {
                    token
                        .split_once('=')
                        .is_some_and(|(name, _)| option_name_eq(name, "Solid"))
                }) {
                true
            } else {
                opts.solid
            };
            let def = Definition {
                kind,
                frame,
                radius,
                height: opts.height,
                solid,
                both_sides: opts.both_sides,
            };
            preferences.radius = radius;
            preferences.height = opts.height;
            let geometry = if cylinder {
                def.geometry(use_extrusions, tolerance)?
            } else {
                legacy_cone(def, tolerance)?
            };
            (def, geometry)
        } else {
            let input = from_arguments(doc, args, kind, ctx.construction_plane, preferences)?;
            preferences = input.preferences;
            let def = input.definition.expect("complete axial definition");
            (def, def.geometry(use_extrusions, tolerance)?)
        };
    let representation = match &geometry {
        Geometry::Brep(brep) if brep.is_extrusion() => "extrusion",
        Geometry::Brep(_) => "B-rep",
        _ => "NURBS",
    };
    let id = doc.add_geometry(geometry)?;
    let label = kind.name().to_ascii_lowercase();
    let solid = if def.solid { "closed" } else { "open" };
    let [start, end] = def.height_range();
    let message = if cylinder {
        format!(
            "Added {solid} {representation} {label} {id} (radius {:.6}, heights {start:.6} to {end:.6})",
            def.radius
        )
    } else {
        format!(
            "Added {solid} {representation} {label} {id} (radius {:.6}, height {end:.6})",
            def.radius
        )
    };
    Ok((message, preferences))
}
