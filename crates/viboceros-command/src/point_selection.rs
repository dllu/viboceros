//! Point selectors also operate inside a grip-capable GetObject prompt.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointSelectionAction {
    Select,
    Invert,
}

impl PointSelectionAction {
    pub fn from_input(input: &str) -> Result<Option<Self>, CommandError> {
        let mut words = input.split_whitespace();
        let Some(name) = words.next() else {
            return Ok(None);
        };
        let action = match name
            .trim_start_matches(['_', '-'])
            .to_ascii_lowercase()
            .as_str()
        {
            "selpt" => Self::Select,
            "invertpt" => Self::Invert,
            _ => return Ok(None),
        };
        require_consumed(&words.collect::<Vec<_>>(), 0, action.name())?;
        Ok(Some(action))
    }

    fn name(self) -> &'static str {
        match self {
            Self::Select => "SelPt",
            Self::Invert => "InvertPt",
        }
    }

    /// Add eligible point objects and visible grips, or invert only the visible
    /// grips. Existing object picks survive both operations. Getter filters can
    /// exclude point objects without changing the meaning of grip selection.
    pub fn apply(
        self,
        document: &mut Document,
        mut accepts_object: impl FnMut(&viboceros_document::Object) -> bool,
    ) -> Result<String, CommandError> {
        let objects = if self == Self::Select {
            document
                .selectable_objects()
                .filter(|o| matches!(o.geometry(), Geometry::Point(_)) && accepts_object(o))
                .map(|o| o.id())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let grips = document
            .control_points()
            .map(|(id, _, _)| id)
            .collect::<Vec<_>>();
        // Both sets come from this live selectable state. Adding object picks
        // cannot invalidate the frozen grip identities or their display.
        if !objects.is_empty() {
            document.select_objects_direct(objects, SelectionMode::Add)?;
        }
        let count = document.select_control_points(
            grips,
            match self {
                Self::Select => SelectionMode::Add,
                Self::Invert => SelectionMode::Toggle,
            },
        )?;
        if count == 0 && self == Self::Select {
            Ok(format!(
                "Selected {} object(s)",
                document.selected_object_count()
            ))
        } else {
            Ok(format!(
                "Selected {} object(s), {count} grip(s)",
                document.selected_object_count()
            ))
        }
    }
}

pub(super) struct PointSelectionCommand(pub(super) PointSelectionAction);
impl Command for PointSelectionCommand {
    fn name(&self) -> &'static str {
        self.0.name()
    }
    fn records_history(&self) -> bool {
        false
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, self.name())?;
        self.0.apply(document, |_| true)
    }
}

#[cfg(test)]
mod tests;
