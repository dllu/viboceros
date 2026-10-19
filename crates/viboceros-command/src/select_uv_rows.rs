//! Repeated, non-geometric surface control-row selection.
use super::*;
use viboceros_document::{ControlPointId, GeometrySnapshot, ObjectId};

const USAGE: &str = "SelUV Points [Direction=U|V|Both] [HideOthers=Yes|No] [Toggle] x,y,z ...";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub direction: SurfaceKnotDirection,
    pub hide_others: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            direction: SurfaceKnotDirection::U,
            hide_others: false,
        }
    }
}
impl Options {
    pub fn arguments(self) -> String {
        format!(
            "Direction={} HideOthers={}",
            match self.direction {
                SurfaceKnotDirection::U => "U",
                SurfaceKnotDirection::V => "V",
                SurfaceKnotDirection::Both => "Both",
            },
            if self.hide_others { "Yes" } else { "No" }
        )
    }
    pub fn accept(&mut self, input: &str) -> Result<bool, CommandError> {
        let input = input.trim_start_matches(['_', '-']);
        if input.eq_ignore_ascii_case("Toggle") {
            self.direction = match self.direction {
                SurfaceKnotDirection::U => SurfaceKnotDirection::V,
                SurfaceKnotDirection::V => SurfaceKnotDirection::U,
                SurfaceKnotDirection::Both => SurfaceKnotDirection::Both,
            };
            return Ok(true);
        }
        let Some((key, value)) = input.split_once('=') else {
            return Ok(false);
        };
        if option_name_eq(key, "Direction") {
            self.direction = match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                "u" => SurfaceKnotDirection::U,
                "v" => SurfaceKnotDirection::V,
                "both" => SurfaceKnotDirection::Both,
                _ => return Err(CommandError::Usage(USAGE)),
            };
        } else if option_name_eq(key, "HideOthers") {
            self.hide_others = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}
pub fn accepts(geometry: &Geometry) -> bool {
    matches!(geometry, Geometry::NurbsSurface(_))
        || matches!(geometry, Geometry::Brep(b) if b.faces().len() == 1)
}

#[derive(Debug)]
pub struct Session {
    object: ObjectId,
    source: GeometrySnapshot,
}
impl Session {
    fn resolve(doc: &Document, object: ObjectId) -> Result<Self, CommandError> {
        let source = doc
            .object(object)
            .filter(|o| doc.is_object_selectable(object) && accepts(o.geometry()))
            .ok_or(CommandError::Usage("SelUV requires one surface"))?
            .geometry_snapshot()
            .clone();
        Ok(Self { object, source })
    }
    /// Preselection retains ordinary picks; postselection releases all picks.
    /// Display and selection changes deliberately leave model history alone.
    pub fn start(
        doc: &mut Document,
        object: ObjectId,
        preselected: bool,
    ) -> Result<Self, CommandError> {
        let session = Self::resolve(doc, object)?;
        if !preselected {
            doc.clear_selection();
        }
        doc.clear_control_point_selection();
        let selected = doc.is_selected(object);
        doc.enable_control_points([object])?;
        if selected {
            doc.select_objects_direct([object], SelectionMode::Add)?;
        }
        Ok(session)
    }
    pub fn object(&self) -> ObjectId {
        self.object
    }
    /// Rhino releases the owner selection after EndCommand, keeping the grips
    /// and unrelated preselected objects. Finishing and cancelling agree.
    pub fn finish(&self, doc: &mut Document) -> Result<(), CommandError> {
        if doc.is_object_selectable(self.object) {
            doc.select_objects_direct([self.object], SelectionMode::Remove)?;
        }
        Ok(())
    }
    pub fn current(&self, doc: &Document) -> bool {
        doc.is_object_selectable(self.object)
            && doc.grip_mode(self.object) == Some(viboceros_document::GripMode::Control)
            && doc
                .object(self.object)
                .is_some_and(|o| self.source.shares_storage_with(o.geometry_snapshot()))
    }
    pub fn row(
        &self,
        doc: &Document,
        point: Point3,
        options: Options,
    ) -> Result<Vec<ControlPointId>, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage(
                "SelUV surface or control display changed; start again",
            ));
        }
        self.rows_at(point, options, doc.tolerance())
    }
    fn rows_at(
        &self,
        point: Point3,
        options: Options,
        tolerance: Tolerance,
    ) -> Result<Vec<ControlPointId>, CommandError> {
        let surface = surface_reference::surface(&self.source, None).unwrap().0;
        let [u, v] = surface.grip_dimensions();
        let (a, b) = surface.closest_parameters(point, tolerance)?;
        let [basis_u, basis_v] = surface.basis_values(a, b)?;
        let dominant = |basis: Vec<Real>, count: usize| {
            let mut values = vec![0.; count];
            for (i, value) in basis.into_iter().enumerate() {
                values[i % count] += value;
            }
            let (mut index, mut largest) = (0, values[0]);
            for (i, &value) in values.iter().enumerate().skip(1) {
                if value > largest {
                    index = i;
                    largest = value;
                }
            }
            index
        };
        let fixed_v = (options.direction != SurfaceKnotDirection::V).then(|| dominant(basis_v, v));
        let fixed_u = (options.direction != SurfaceKnotDirection::U).then(|| dominant(basis_u, u));
        Ok((0..u * v)
            .filter(|i| fixed_v == Some(i % v) || fixed_u == Some(i / v))
            .map(|index| ControlPointId {
                object: self.object,
                index,
            })
            .collect())
    }
    pub fn pick(
        &self,
        doc: &mut Document,
        point: Point3,
        options: Options,
    ) -> Result<usize, CommandError> {
        let row = self.row(doc, point, options)?;
        Ok(doc.select_control_points(row, SelectionMode::Add)?)
    }
}

#[derive(Default)]
pub(super) struct SelectUVCommand(remembered::Remembered<Options>);
impl SelectUVCommand {
    fn execute(
        &self,
        doc: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        if args.first().is_none_or(|a| !option_name_eq(a, "Points")) {
            return Err(CommandError::Usage(USAGE));
        }
        let mut options = self.0.get();
        let mut picks = Vec::new();
        for word in &args[1..] {
            if !options.accept(word)? {
                picks.push((parse_point(&[word])?.0, options));
            }
        }
        let ids = doc
            .selected_objects()
            .filter(|o| accepts(o.geometry()))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if ids.len() != 1 {
            return Err(CommandError::Usage("SelUV requires one selected surface"));
        }
        let source = Session::resolve(doc, ids[0])?;
        // Stage geometry queries before changing display or selection.
        let rows = picks
            .into_iter()
            .map(|(p, o)| source.rows_at(p, o, doc.tolerance()))
            .collect::<Result<Vec<_>, _>>()?;
        let session = Session::start(doc, ids[0], !postselected)?;
        for row in rows {
            doc.select_control_points(row, SelectionMode::Add)?;
        }
        session.finish(doc)?;
        self.0.set(options);
        Ok(format!(
            "SelUV: {} grip(s) selected",
            doc.selected_control_points().count()
        ))
    }
}
impl Command for SelectUVCommand {
    fn select_uv_options(&self) -> Option<Options> {
        Some(self.0.get())
    }
    fn remember_select_uv_options(&self, value: Options) -> bool {
        self.0.set(value);
        true
    }
    fn name(&self) -> &'static str {
        "SelUV"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.execute(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, true)
    }
}

#[cfg(test)]
mod tests;
