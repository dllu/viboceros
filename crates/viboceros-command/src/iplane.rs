//! Standalone infinite-plane intersections reuse the section producer.
use super::*;
use viboceros_geometry::Frame3;

#[cfg(test)]
mod tests;

pub(super) struct IPlaneCommand;
const USAGE: &str = "IPlane [Objects=id,...] [Plane=WorldXY|WorldYZ|WorldZX|CPlaneXY|CPlaneYZ|CPlaneZX|x,y,z,nx,ny,nz|Object:id[:face]|AroundCurve:id:parameter]";

impl Command for IPlaneCommand {
    fn name(&self) -> &'static str {
        "IPlane"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["InfinitePlane", "IP"]
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
        let mut objects = None;
        let mut definition = "CPlaneXY";
        let mut seen = BTreeSet::new();
        for argument in args {
            let (key, value) = argument
                .trim_start_matches('_')
                .split_once('=')
                .ok_or(CommandError::Usage(USAGE))?;
            let key = key.to_ascii_lowercase();
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.as_str() {
                "objects" => {
                    let ids = value
                        .split(',')
                        .map(|id| {
                            id.parse::<ObjectId>()
                                .map_err(|_| CommandError::Usage(USAGE))
                        })
                        .collect::<Result<BTreeSet<_>, _>>()?;
                    objects = Some(ids.into_iter().collect::<Vec<_>>());
                }
                "plane" => definition = value,
                _ => return Err(CommandError::Usage(USAGE)),
            }
        }
        let plane = infinite_plane::resolve(doc, definition, context.construction_plane)?.plane;
        let frame = Frame3::try_from_unit_normal(plane.origin(), plane.normal())?;
        let ids = objects.unwrap_or_else(|| {
            doc.selected_objects()
                .filter(|o| ObjectSelectionFilter::Section.accepts_object(o))
                .map(|o| o.id())
                .collect()
        });
        if ids.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        if ids.len() > MAX_INTERSECT_PAIRS {
            return Err(CommandError::TooManyIntersectPairs {
                maximum: MAX_INTERSECT_PAIRS,
            });
        }
        let mut staged = Vec::new();
        for id in ids {
            let object = doc.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
            if !doc.is_object_selectable(id)
                || !ObjectSelectionFilter::Section.accepts_object(object)
            {
                return Err(CommandError::Usage(USAGE));
            }
            staged.extend(section::section_geometry(
                object.geometry(),
                frame,
                0.,
                true,
                doc.tolerance(),
            )?);
            if staged.len() > MAX_INTERSECT_OUTPUTS {
                return Err(CommandError::TooManyIntersectOutputs {
                    maximum: MAX_INTERSECT_OUTPUTS,
                });
            }
        }
        let count = staged.len();
        let mut output = Vec::with_capacity(count);
        for geometry in staged {
            output.push(doc.add_geometry(geometry)?);
        }
        doc.select_command_results(output)?;
        Ok(format!(
            "Created {count} infinite-plane intersection object(s)"
        ))
    }
}
