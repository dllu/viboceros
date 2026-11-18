//! Three-point alignment uses the same edge and width input as Rectangle.
use super::*;
use viboceros_command::rectangle_input::RectangleInput;
use viboceros_drafting::PointInput;
use viboceros_geometry::GeometryError;

#[derive(Default)]
pub(super) struct RectangleGetter {
    pub(super) edge_midpoint: bool,
    edge_length: Option<f64>,
    width: Option<f64>,
}

impl RectangleGetter {
    pub(super) fn input(
        &self,
        options: AlignmentOptions,
        plane: Frame3,
    ) -> Result<RectangleInput, GeometryError> {
        let mut rectangle = RectangleInput::new(plane);
        rectangle.option(if self.edge_midpoint {
            "EdgeMidpoint"
        } else {
            "3Point"
        });
        if let Some(first) = options.references[0] {
            rectangle.point(first)?;
            if let Some(length) = self.edge_length {
                rectangle.number(length)?;
            }
        }
        if let Some(second) = options.references[1] {
            rectangle.point(second)?;
            if let Some(width) = self.width {
                rectangle.number(width)?;
            }
        }
        Ok(rectangle)
    }
}

impl VibocerosApp {
    pub(super) fn continue_align_rectangle(
        &mut self,
        text: &str,
        enter: bool,
        options: AlignmentOptions,
    ) -> bool {
        let Some(session) = &mut self.align_session else {
            return false;
        };
        if !options.three_point || options.references[0].is_none() {
            return false;
        }
        let width_stage = options.references[1].is_some();
        let value = if enter && width_stage {
            let plane = self.viewports[self.active_viewport].construction_plane();
            session
                .rectangle
                .input(options, plane)
                .and_then(|mut input| {
                    input.enter()?;
                    let start = options.references[0].unwrap();
                    start.distance_to(input.anchor().unwrap())
                })
                .map_err(|error| error.to_string())
        } else if let Some(value) = PointInput::parse_length_with_units(text, self.document.units())
        {
            value.map_err(|error| error.to_string())
        } else {
            return false;
        };
        match value {
            Ok(value) if value.is_finite() && value > 0.0 => {
                if width_stage {
                    session.rectangle.width = Some(if enter && session.rectangle.edge_midpoint {
                        value * 2.0
                    } else {
                        value
                    });
                } else {
                    session.rectangle.edge_length = Some(value);
                }
                session.preview = None;
                self.push_log(
                    if width_stage {
                        "Choose alignment rectangle"
                    } else {
                        "Pick the end of the alignment plane edge"
                    }
                    .into(),
                );
            }
            Ok(_) => self.push_log("Enter a positive alignment rectangle dimension".into()),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }
}
