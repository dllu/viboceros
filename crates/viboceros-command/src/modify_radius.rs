//! Radius edits retain each curve's representation and independent domain.
use super::*;
use crate::circle_input::CircleSizeMode;
use viboceros_document::ReplacementHistory;
use viboceros_geometry::CurveSegment3;

pub const USAGE: &str = "ModifyRadius [Radius|Diameter|Area|Circumference] value [Copy=Yes|No]";
// Rhino's geometric zero tolerance, independent of the model fitting tolerance.
const RECOGNITION: Real = 2.328_306_436_538_696_3e-10;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub mode: CircleSizeMode,
    pub value: Option<Real>,
    pub copy: Option<bool>,
}
impl Input {
    pub fn radius(self) -> Result<Option<Real>, CommandError> {
        self.value
            .map(|v| {
                let r = self.mode.radius(v);
                if v.is_finite() && v > 0. && r.is_finite() && r > 0. {
                    Ok(r)
                } else {
                    Err(CommandError::Usage(USAGE))
                }
            })
            .transpose()
    }
}

/// Parse a complete script or an unfinished numeric getter without editing it.
pub fn parse(arguments: &[&str], mut input: Input) -> Result<Input, CommandError> {
    let mut seen_size = false;
    let mut seen_copy = false;
    let mut i = 0;
    while i < arguments.len() {
        let word = arguments[i];
        if let Some((name, value)) = word.split_once('=') {
            if option_name_eq(name, "Copy") && !seen_copy {
                input.copy = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
                seen_copy = true;
            } else if let Some(mode) = CircleSizeMode::parse(name) {
                if seen_size {
                    return Err(CommandError::Usage(USAGE));
                }
                input.mode = mode;
                input.value = Some(parse_finite_real(value)?);
                seen_size = true;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        } else if option_name_eq(word, "Copy") && !seen_copy {
            i += 1;
            input.copy = Some(
                parse_yes_no(arguments.get(i).ok_or(CommandError::Usage(USAGE))?)
                    .ok_or(CommandError::Usage(USAGE))?,
            );
            seen_copy = true;
        } else if let Some(mode) = CircleSizeMode::parse(word) {
            if seen_size {
                return Err(CommandError::Usage(USAGE));
            }
            input.mode = mode;
        } else {
            if seen_size {
                return Err(CommandError::Usage(USAGE));
            }
            input.value = Some(parse_finite_real(word)?);
            seen_size = true;
        }
        i += 1;
    }
    input.radius()?;
    Ok(input)
}

/// Public native recognition is shared with SimplifyCrv. Composite curves are
/// deliberately ineligible: a circular polycurve is not an arc object.
pub fn circular_support(curve: CurveRef<'_>) -> Result<Option<(Point3, Real)>, GeometryError> {
    Ok(match curve {
        CurveRef::Circle(c) => Some((c.center(), c.radius())),
        CurveRef::Arc(c) => Some((c.center(), c.radius())),
        CurveRef::NurbsCurve(c) => {
            let tolerance = Tolerance::try_new(RECOGNITION, 1e-12, 1e-10)?;
            match c.analytic_form(tolerance)? {
                Some(CurveSegment3::Arc(arc)) => Some((arc.center(), arc.radius())),
                _ => None,
            }
        }
        _ => None,
    })
}

/// The native selector admits circular composite inputs although the editor
/// only changes standalone arc and NURBS curve objects.
pub fn accepts_curve(curve: CurveRef<'_>) -> bool {
    if matches!(curve, CurveRef::PolyCurve(_) | CurveRef::Ellipse(_)) {
        return curve
            .to_nurbs()
            .and_then(|c| c.analytic_form(Tolerance::try_new(RECOGNITION, 1e-12, 1e-10)?))
            .is_ok_and(|form| matches!(form, Some(CurveSegment3::Arc(_))));
    }
    circular_support(curve).is_ok_and(|support| support.is_some())
}

pub(super) struct ModifyRadiusCommand;
impl Command for ModifyRadiusCommand {
    fn name(&self) -> &'static str {
        "ModifyRadius"
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let input = parse(args, Input::default())?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::CircularCurves,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![BooleanSelectionOption {
                name: "Copy",
                value: input.copy.unwrap_or(false),
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn object_selection_confirmation(
        &self,
        _doc: &Document,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        self.object_selection_prompt(args)
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        run(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        run(doc, args, true)
    }
}
fn run(doc: &mut Document, args: &[&str], postselected: bool) -> Result<String, CommandError> {
    let input = parse(args, Input::default())?;
    let radius = input.radius()?.ok_or(CommandError::Usage(USAGE))?;
    let mut maps = Vec::new();
    for object in doc.selected_objects() {
        if !doc.is_object_selectable(object.id()) {
            continue;
        }
        let Some(curve) = object.geometry().curve_ref() else {
            continue;
        };
        let Some((center, old_radius)) = circular_support(curve)? else {
            continue;
        };
        if radius == old_radius {
            continue;
        }
        maps.push((
            object.id(),
            AffineTransform3::try_uniform_scale(center, radius / old_radius)?,
        ));
    }
    let count = maps.len();
    // Finish all geometry and metadata preparation before touching selection.
    let plans = maps
        .into_iter()
        .map(|(id, map)| {
            let object = doc.object(id).unwrap();
            Ok((
                id,
                object
                    .geometry()
                    .transformed(map, Tolerance::NUMERICAL_VALIDATION)?,
                object.geometry_user_text().clone(),
            ))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    if postselected && count != 0 {
        doc.release_command_selection_on_history_replay(selected.iter().copied())?;
        doc.clear_selection();
    }
    if input.copy.unwrap_or(false) {
        for (_, geometry, text) in plans {
            doc.add_geometry_with_metadata(
                geometry,
                viboceros_document::ObjectAttributes::on_layer(doc.current_layer_id()),
                text,
            )?;
        }
    } else {
        doc.replace_object_geometries_with_history(
            plans
                .iter()
                .map(|(id, geometry, _)| (*id, geometry.clone())),
            ReplacementHistory::EveryReplacement,
        )?;
        for (id, _, text) in plans {
            for (key, value) in text {
                doc.set_object_geometry_user_text([id], &key, Some(&value))?;
            }
        }
    }
    if postselected && count != 0 && input.copy.unwrap_or(false) {
        doc.select_command_results(selected)?;
    }
    Ok(format!("Modified radius of {count} curve(s)"))
}

#[cfg(test)]
mod tests;
