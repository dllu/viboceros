//! Resolve typed handle locations on the original selected B-rep edges.
use super::*;
use viboceros_drafting::PointInput;

impl VibocerosApp {
    pub(in crate::app::edge_fillet) fn continue_edge_placement_point(
        &mut self,
        input: &str,
    ) -> bool {
        if !matches!(
            self.edge_blend_adjustment.operation,
            Some(HandleOperation::Place {
                entering_size: false,
                ..
            })
        ) {
            return false;
        }
        let units = self.document.units();
        if PointInput::parse_length_with_units(input, units).is_some() {
            return false;
        }
        let Some(input) = PointInput::parse_with_units(input, units) else {
            return false;
        };
        let result = input
            .and_then(|input| {
                input.resolve(
                    self.viewports[self.active_viewport].construction_plane(),
                    self.last_point,
                )
            })
            .map_err(|error| error.to_string())
            .and_then(|point| self.resolve_edge_handle_placement(point));

        let error = match result {
            Ok((id, point))
                if self.use_edge_handle_operation(HandleGesture::Place { id, point }) =>
            {
                None
            }
            Ok(_) => Some("The selected edge changed. Pick the edges again.".into()),
            Err(error) => Some(error),
        };
        if let Some(error) = error {
            self.push_log(format!("Error: {error}"));
            self.command_file.fail_dispatch();
        }
        self.command_input.clear();
        true
    }

    fn resolve_edge_handle_placement(
        &self,
        point: Point3,
    ) -> Result<(SizeHandleId, Point3), String> {
        let document = self.edge_blend_source_document();
        let tolerance = document.tolerance();
        let mut nearest = None;
        let mut runner_up = f64::INFINITY;
        for (pick, curve) in self.edge_handle_placement_curves(document)? {
            let parameter = curve
                .closest_parameter(point, tolerance)
                .map_err(|error| error.to_string())?;
            let contact = curve
                .evaluate(parameter)
                .map_err(|error| error.to_string())?;
            let distance = contact
                .distance_to(point)
                .map_err(|error| error.to_string())?;
            match nearest {
                Some((best, _, _)) if best <= distance => runner_up = runner_up.min(distance),
                previous => {
                    if let Some((best, _, _)) = previous {
                        runner_up = best;
                    }
                    nearest = Some((
                        distance,
                        SizeHandleId::at_interior_parameter(pick, curve, parameter),
                        contact,
                    ));
                }
            }
        }
        let (distance, handle, contact) = nearest.ok_or("Select an edge to place a handle.")?;
        if runner_up - distance <= tolerance.absolute() {
            return Err(
                "The location is ambiguous between selected edges. Pick a location in a viewport."
                    .into(),
            );
        }
        let handle =
            handle.ok_or("Enter a location in the interior of the nearest selected edge.")?;
        Ok((handle, contact))
    }
}
