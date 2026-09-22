//! In-place direction reversal with command-specific closed-B-rep policy.
use super::*;

#[cfg(test)]
mod tests;

pub(super) struct FlipCommand;

impl Command for FlipCommand {
    fn name(&self) -> &'static str {
        "Flip"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Reverse", "Rev"]
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(arguments, 0, "Flip")?;
        Ok(Some(ObjectSelectionPrompt {
            command: "Flip",
            // Rhino's actual command accepts mixed selections, including
            // points and closed solids, then leaves the skipped peers selected.
            filter: ObjectSelectionFilter::Any,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        run(document, arguments, false)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        run(document, arguments, true)
    }
}

fn run(
    document: &mut Document,
    arguments: &[&str],
    postselected: bool,
) -> Result<String, CommandError> {
    require_consumed(arguments, 0, "Flip")?;
    if document.selected_object_count() == 0 {
        return Err(CommandError::NoObjectsSelected);
    }
    let mut replacements = Vec::new();
    let mut retained = Vec::new();
    for object in document.selected_objects() {
        if let Some(reversed) = flipped_geometry(object.geometry(), document.tolerance())? {
            replacements.push((object.id(), reversed));
        } else {
            retained.push(object.id());
        }
    }
    let count = document.replace_object_geometries(replacements)?;
    if postselected {
        // Do not expand groups or clear skipped objects. In particular, a
        // flipped member must not drag its unchanged group peers out of selection.
        document.select_command_results(retained)?;
    }
    Ok(format!("Flipped {count} object(s)"))
}

fn flipped_geometry(
    geometry: &Geometry,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, GeometryError> {
    let converted;
    let brep = match geometry {
        Geometry::Brep(brep) => brep,
        Geometry::NurbsSurface(surface) => {
            // Face sense is independent of UV parameterization. Promoting a
            // natural face preserves knots, coefficients, domains, and UVs.
            converted = Brep::try_surface_face(surface.clone(), tolerance)?;
            &converted
        }
        _ => return flipped_curve_or_mesh(geometry, tolerance),
    };
    // Closed topology alone does not make a solid: an inconsistently oriented
    // closed B-rep is flippable. This is command policy, not a restriction on
    // the kernel: inward solids remain representable and closed meshes can flip.
    Ok((!brep.is_solid()).then(|| Geometry::Brep(brep.reversed())))
}

/// Shared with Dir's existing curve/mesh mode; Dir's surface menu has a separate
/// workflow and has not been audited by the standalone Flip command probes.
pub(super) fn flipped_curve_or_mesh(
    geometry: &Geometry,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, GeometryError> {
    Ok(match geometry {
        Geometry::Line(line) => Some(Geometry::Line(line.reversed())),
        Geometry::Circle(circle) => Some(Geometry::Circle(circle.reversed())),
        Geometry::Arc(arc) => Some(Geometry::Arc(arc.reversed(tolerance)?)),
        Geometry::Ellipse(ellipse) => Some(Geometry::Ellipse(ellipse.reversed())),
        Geometry::Polyline(polyline) => Some(Geometry::Polyline(polyline.reversed())),
        Geometry::NurbsCurve(curve) => Some(Geometry::NurbsCurve(curve.reversed()?)),
        Geometry::PolyCurve(curve) => Some(Geometry::PolyCurve(curve.reversed()?)),
        Geometry::Mesh(mesh) => Some(Geometry::Mesh(mesh.reversed())),
        _ => None,
    })
}
