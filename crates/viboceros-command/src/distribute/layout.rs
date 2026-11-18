//! Distribution bounds, interval solver and placement transaction.
use super::*;
pub use crate::placement::Sources;
use crate::placement::{PickPolicy, Placement};
use viboceros_document::ObjectId;
use viboceros_geometry::AffineTransform3;

#[derive(Clone, Debug)]
pub struct Layout {
    sources: Sources,
    placement: Placement,
    spacing: f64,
    mode: DistributionMode,
}

impl Sources {
    pub fn layout(
        &self,
        direction: Direction,
        plane: Frame3,
        settings: DistributionSettings,
    ) -> Result<Layout, CommandError> {
        let units = self.units();
        if units.len() < 3 {
            return Err(CommandError::InsufficientDistributionObjects {
                actual: units.len(),
            });
        }
        let frame = direction_frame(direction, plane, self.origin(&units[0])?, self.tolerance())?;
        let bounds = units
            .iter()
            .map(|unit| self.bounds(unit, frame))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let mut order = (0..units.len()).collect::<Vec<_>>();
        order.sort_by(|a, b| {
            bounds[*a]
                .min()
                .to_array()
                .partial_cmp(&bounds[*b].min().to_array())
                .expect("finite bounds")
        });
        let intervals = order
            .iter()
            .map(|&i| [bounds[i].min().x(), bounds[i].max().x()])
            .collect::<Vec<_>>();
        let (offsets, spacing) = planner::offsets(&intervals, settings.mode, settings.spacing)?;
        let placement =
            self.layout_placement(order.into_iter().zip(offsets).map(|(index, distance)| {
                Ok((&units[index], frame.x_axis().as_vector().scaled(distance)?))
            }))?;
        Ok(Layout {
            sources: self.clone(),
            placement,
            spacing,
            mode: settings.mode,
        })
    }
}

impl Layout {
    pub fn placement(&self) -> &Placement {
        &self.placement
    }

    pub fn translations(&self) -> impl Iterator<Item = (ObjectId, AffineTransform3)> + '_ {
        self.placement.translations()
    }

    pub fn commit(
        self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<String, CommandError> {
        crate::run_command_transaction(document, "Distribute", |doc| self.apply(doc, postselected))
    }

    pub(super) fn apply(
        self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<String, CommandError> {
        if !self.sources.is_current(document) {
            return Err(CommandError::DistributeStale);
        }
        let count = self.sources.unit_count();
        let changed = self.placement.apply(
            document,
            if postselected {
                PickPolicy::ReleaseMoved
            } else {
                PickPolicy::Preserve
            },
        )?;
        Ok(format!(
            "Distributed {count} object/group or grip unit(s) with {:?} spacing {:.6}; moved {changed} object(s)",
            self.mode, self.spacing
        ))
    }
}
