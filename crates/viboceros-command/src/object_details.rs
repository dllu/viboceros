//! Read-only object properties and native technical geometry reports.
use super::*;
use std::fmt::Write;
mod summary;
mod text;
use text::Report;

pub(super) struct DetailsCommand {
    pub technical: bool,
}

impl Command for DetailsCommand {
    fn name(&self) -> &'static str {
        if self.technical { "List" } else { "What" }
    }
    fn records_history(&self) -> bool {
        false
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(args, 0, self.name())?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Any,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        require_consumed(args, 0, self.name())?;
        if doc.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut report = Report::new();
        writeln!(
            report,
            "{}: {} object(s)",
            self.name(),
            doc.selected_object_count()
        )
        .unwrap();
        for object in doc.selected_objects() {
            if report.is_full() {
                break;
            }
            attributes(&mut report, doc, object);
            summary::write(&mut report, doc, object.geometry());
            if self.technical && !report.is_full() {
                match native_dump(object.geometry()) {
                    Ok(dump) => {
                        writeln!(report, "{}", dump.text).unwrap();
                        if dump.truncated {
                            writeln!(report, "[Geometry details truncated at 128 KiB]").unwrap();
                        }
                    }
                    Err(error) => {
                        writeln!(report, "Geometry details unavailable: {error}").unwrap();
                    }
                }
            }
        }
        Ok(report.finish())
    }
}

fn attributes(report: &mut Report, doc: &Document, object: &viboceros_document::Object) {
    let a = object.attributes();
    writeln!(report, "\nObject ID: {}", object.id()).unwrap();
    if let Some(name) = a.name() {
        writeln!(report, "Name: {name:?}").unwrap();
    }
    if let Some(layer) = doc.layer(a.layer_id()) {
        writeln!(
            report,
            "Layer: {:?} (number {})",
            layer.name(),
            layer.number()
        )
        .unwrap();
        writeln!(
            report,
            "Layer visible: {}; layer locked: {}",
            layer.is_visible(),
            layer.is_locked()
        )
        .unwrap();
    }
    let color = a.object_color();
    writeln!(
        report,
        "Color source: {:?}; object color: {},{},{}",
        a.color_source(),
        color.red,
        color.green,
        color.blue
    )
    .unwrap();
    writeln!(
        report,
        "Object visible: {}; object locked: {}",
        a.is_visible(),
        a.is_locked()
    )
    .unwrap();
    for id in object.group_ids() {
        if let Some(group) = doc.group(*id) {
            write!(report, "Group: {id}").unwrap();
            if let Some(name) = group.name() {
                write!(report, " {name:?}").unwrap();
            }
            writeln!(report).unwrap();
        }
        if report.is_full() {
            return;
        }
    }
    for (label, values) in [
        ("Attribute user text", a.user_text()),
        ("Geometry user text", object.geometry_user_text()),
    ] {
        for (key, value) in values {
            writeln!(report, "{label}: {key:?} = {value:?}").unwrap();
            if report.is_full() {
                return;
            }
        }
    }
}

fn native_dump(geometry: &Geometry) -> Result<viboceros_io::GeometryDump, CommandError> {
    if let Geometry::BlockInstance(instance) = geometry {
        let reference = instance.reference();
        return Ok(viboceros_io::dump_3dm_instance_reference(
            &reference.definition().to_string(),
            reference.transform(),
        )?);
    }
    Ok(viboceros_io::dump_3dm_geometry(
        &crate::interchange::geometry_to_3dm(geometry)?,
    )?)
}

#[cfg(test)]
mod tests;
