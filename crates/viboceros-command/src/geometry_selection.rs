//! Geometry-filtered selection commands, independent of interactive picking.

use super::*;

pub(super) struct SelectGeometryCommand {
    pub(super) name: &'static str,
    pub(super) aliases: &'static [&'static str],
    pub(super) filter: GeometrySelectionFilter,
}

impl Command for SelectGeometryCommand {
    fn name(&self) -> &'static str {
        self.name
    }

    fn aliases(&self) -> &'static [&'static str] {
        self.aliases
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, self.name)?;
        let tolerance = document.tolerance();
        let mode = if matches!(self.filter, GeometrySelectionFilter::MirroredBlock) {
            SelectionMode::Replace
        } else {
            SelectionMode::Add
        };
        select_matching_geometry(
            document,
            |geometry| self.filter.matches(geometry, tolerance),
            mode,
        )
    }
}

/// Collect matches before changing selection so a fallible predicate cannot
/// leave a partial result. Hidden/locked objects never reach the predicate.
fn select_matching_geometry(
    document: &mut Document,
    mut predicate: impl FnMut(&Geometry) -> Result<bool, GeometryError>,
    mode: SelectionMode,
) -> Result<String, CommandError> {
    let mut matches = Vec::new();
    for object in document.selectable_objects() {
        if predicate(object.geometry())? {
            matches.push(object.id());
        }
    }
    document.select_objects_direct(matches, mode)?;
    Ok(format!(
        "Selected {} object(s)",
        document.selected_object_count()
    ))
}

mod filter;
pub(super) use filter::GeometrySelectionFilter;

pub(super) struct SelShortCurveCommand;

pub(super) struct SelSmallCommand;

impl Command for SelSmallCommand {
    fn name(&self) -> &'static str {
        "SelSmall"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let [size] = arguments else {
            return Err(CommandError::Usage("SelSmall maximum-size"));
        };
        let maximum = parse_finite_real(size)?;
        if maximum < 0.0 {
            return Err(CommandError::Usage("SelSmall maximum-size"));
        }
        let tolerance = document.tolerance();
        select_matching_geometry(
            document,
            |geometry| {
                let bounds = geometry.tight_bounds(tolerance)?;
                let min = bounds.min().to_array();
                let max = bounds.max().to_array();
                // hypot keeps a diagonal larger than f64::MAX as infinity, so such
                // an object simply fails a finite limit instead of failing the
                // entire selection on an intermediate subtraction overflow.
                let diagonal = (max[0] - min[0])
                    .hypot(max[1] - min[1])
                    .hypot(max[2] - min[2]);
                Ok(diagonal < maximum)
            },
            SelectionMode::Add,
        )
    }
}

impl Command for SelShortCurveCommand {
    fn name(&self) -> &'static str {
        "SelShortCrv"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let [maximum_length] = arguments else {
            return Err(CommandError::Usage("SelShortCrv maximum-length"));
        };
        let maximum_length = parse_positive_curve_length(maximum_length)?;
        // Rhino's command includes a relative 1e-6 allowance, including its
        // boundary but not the next float (see the retained line probes).
        // Above MAX every representable length is eligible; keep a finite cap.
        let comparison_limit = (maximum_length * 1.000001).min(Real::MAX);
        select_matching_geometry(
            document,
            |geometry| match geometry_curve_ref(geometry) {
                Some(curve) => curve.is_short_for_selection(comparison_limit),
                None => Ok(false),
            },
            SelectionMode::Add,
        )
    }
}

#[cfg(test)]
mod tests;
