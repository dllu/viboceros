//! Divide native surface faces while retaining document identity and history.
use super::*;
use viboceros_document::GeometrySnapshot;
const NAME: &str = "DivideAlongCreases";
const USAGE: &str = "DivideAlongCreases [SplitAtKinks=Yes|No] [SplitAtTangents=Yes|No]";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreaseDivisionOptions {
    pub split_at_kinks: bool,
    pub split_at_tangents: bool,
}
impl Default for CreaseDivisionOptions {
    fn default() -> Self {
        Self {
            split_at_kinks: true,
            split_at_tangents: true,
        }
    }
}
#[derive(Default)]
pub(super) struct DivideAlongCreasesCommand {
    options: remembered::Remembered<CreaseDivisionOptions>,
}
#[derive(Clone, Debug)]
pub struct CreaseDivisionSelection {
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    replacements: Vec<(ObjectId, Geometry)>,
    tolerance: Tolerance,
    added: usize,
}
impl CreaseDivisionSelection {
    pub fn prepare(doc: &Document, options: CreaseDivisionOptions) -> Result<Self, CommandError> {
        let mut result = Self {
            sources: vec![],
            replacements: vec![],
            tolerance: doc.tolerance(),
            added: 0,
        };
        for object in doc.selected_objects() {
            if !doc.is_object_selectable(object.id()) {
                return Err(CommandError::Usage(USAGE));
            }
            if !matches!(
                object.geometry(),
                Geometry::NurbsSurface(_) | Geometry::Brep(_)
            ) {
                return Err(CommandError::Usage(
                    "DivideAlongCreases requires surfaces or polysurfaces",
                ));
            }
            result
                .sources
                .push((object.id(), object.geometry_snapshot().clone()));
            if !options.split_at_kinks && !options.split_at_tangents {
                continue;
            }
            let source = match object.geometry() {
                Geometry::Brep(b) => std::borrow::Cow::Borrowed(b),
                Geometry::NurbsSurface(s) => {
                    std::borrow::Cow::Owned(Brep::try_surface_face(s.clone(), doc.tolerance())?)
                }
                _ => unreachable!(),
            };
            if let Some((brep, added)) = divide(&source, options, doc.tolerance())? {
                result.added += added;
                result
                    .replacements
                    .push((object.id(), Geometry::Brep(brep)));
            }
        }
        if result.sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        Ok(result)
    }
    pub fn commit(&self, doc: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(doc, NAME, |doc| self.apply(doc))
    }
    fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if self.tolerance != doc.tolerance()
            || self.sources.iter().any(|(id, s)| {
                !doc.is_object_selectable(*id)
                    || !doc.object(*id).is_some_and(|o| o.geometry_snapshot() == s)
            })
        {
            return Err(CommandError::Usage(
                "Crease division source or tolerance changed; select objects again",
            ));
        }
        doc.replace_object_geometries(self.replacements.clone())?;
        Ok(format!(
            "Added {} face(s) in {} object(s)",
            self.added,
            self.replacements.len()
        ))
    }
}
fn divide(
    source: &Brep,
    options: CreaseDivisionOptions,
    tolerance: Tolerance,
) -> Result<Option<(Brep, usize)>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        let controls = source.faces().iter().fold(0usize, |n, f| {
            n.saturating_add(f.surface().control_points().len())
        });
        if source.faces().len() > 512 || controls > 2_000_000 {
            return Err(CommandError::Usage(
                "Crease division supports at most 512 faces and 2,000,000 surface controls per B-rep",
            ));
        }
        let origin = source
            .vertices()
            .first()
            .map(|v| v.point())
            .unwrap_or_else(|| source.bounds().min());
        let zero = Point3::try_new(0., 0., 0.)?;
        let local = source.transformed(
            AffineTransform3::from_translation(origin.vector_to(zero)?),
            tolerance,
        )?;
        viboceros_smlib::Shape::divide_brep_along_creases(
            &local,
            options.split_at_kinks,
            options.split_at_tangents,
            tolerance,
        )?
        .map(|(shape, n)| {
            Ok((
                shape.to_brep(tolerance)?.transformed(
                    AffineTransform3::from_translation(zero.vector_to(origin)?),
                    tolerance,
                )?,
                n,
            ))
        })
        .transpose()
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = (source, options, tolerance);
        Err(CommandError::Usage(
            "DivideAlongCreases requires the native-smlib feature",
        ))
    }
}
impl Command for DivideAlongCreasesCommand {
    fn name(&self) -> &'static str {
        NAME
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let o = parse(args, self.options.get())?;
        Ok(Some(ObjectSelectionPrompt {
            command: NAME,
            filter: ObjectSelectionFilter::SurfaceComponents,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![
                BooleanSelectionOption {
                    name: "SplitAtKinks",
                    value: o.split_at_kinks,
                    aliases: &[],
                },
                BooleanSelectionOption {
                    name: "SplitAtTangents",
                    value: o.split_at_tangents,
                    aliases: &[],
                },
            ],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        self.options.set(parse(args, self.options.get())?);
        Ok(())
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let options = parse(args, self.options.get())?;
        let report = CreaseDivisionSelection::prepare(doc, options)?.apply(doc)?;
        self.options.set(options);
        Ok(report)
    }
}
fn parse(
    args: &[&str],
    mut options: CreaseDivisionOptions,
) -> Result<CreaseDivisionOptions, CommandError> {
    let (mut i, mut kinks, mut tangents) = (0, false, false);
    while i < args.len() {
        let (name, value, used) = orient_option(args, i, USAGE)?;
        let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        if option_name_eq(name, "SplitAtKinks") && !kinks {
            options.split_at_kinks = value;
            kinks = true;
        } else if option_name_eq(name, "SplitAtTangents") && !tangents {
            options.split_at_tangents = value;
            tangents = true;
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        i += used;
    }
    Ok(options)
}
#[cfg(test)]
mod tests;
