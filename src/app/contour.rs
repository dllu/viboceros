//! Contour uses shared CPlane, snap, coordinate, and unit-aware input.
use super::*;
impl VibocerosApp {
    pub(super) fn try_start_contour_input(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty()
            || !words[0]
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Contour")
        {
            return false;
        }
        let Ok(options) = self
            .commands
            .contour_preferences()
            .options
            .with_arguments(&words[1..])
        else {
            return false;
        };
        if !self
            .document
            .selected_objects()
            .any(|o| viboceros_command::ObjectSelectionFilter::Section.accepts_object(o))
        {
            return false;
        }
        self.cancel_interactive_command(false);
        self.active_command = Some(InteractiveCommand::Contour {
            base: None,
            direction: None,
            spacing_start: None,
            options,
        });
        self.contour_prompt();
        true
    }
    pub(super) fn accept_contour_point(&mut self, point: Point3) -> Option<bool> {
        let Some(InteractiveCommand::Contour {
            base,
            direction,
            spacing_start,
            options,
        }) = self.active_command
        else {
            return None;
        };
        if let (Some(base), Some(direction)) = (base, direction) {
            if let Some(start) = spacing_start {
                return Some(match start.distance_to(point) {
                    Ok(spacing) => self.finish_contour(base, direction, spacing, options),
                    Err(error) => {
                        self.push_log(format!("Error: {error}"));
                        false
                    }
                });
            }
            self.active_command = Some(InteractiveCommand::Contour {
                base: Some(base),
                direction: Some(direction),
                spacing_start: Some(point),
                options,
            });
            self.last_point = Some(point);
            self.contour_prompt();
            return Some(true);
        }
        if let Some(base) = base {
            if let Err(error) = base
                .vector_to(point)
                .and_then(|v| v.normalized(self.document.tolerance()))
            {
                self.push_log(format!("Error: {error}"));
                return Some(false);
            }
            self.active_command = Some(InteractiveCommand::Contour {
                base: Some(base),
                direction: Some(point),
                spacing_start: None,
                options,
            });
        } else {
            self.active_command = Some(InteractiveCommand::Contour {
                base: Some(point),
                direction: None,
                spacing_start: None,
                options,
            });
        }
        self.last_point = Some(point);
        self.contour_prompt();
        Some(true)
    }
    fn finish_contour(
        &mut self,
        base: Point3,
        direction: Point3,
        spacing: f64,
        options: viboceros_command::contour::ContourOptions,
    ) -> bool {
        let command = format!(
            "Contour {} {} {spacing} {}",
            format_model_point(base),
            format_model_point(direction),
            options.arguments(),
        );
        match self.commands.execute(&mut self.document, &command) {
            Ok(message) => {
                self.cancel_interactive_command(false);
                self.push_log(message);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
    pub(super) fn continue_contour_input(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Contour {
            base,
            direction,
            spacing_start,
            mut options,
        }) = self.active_command
        else {
            return false;
        };
        let allow_range = base.is_none() && !options.range;
        match options.update(input, allow_range) {
            Ok(true) => {
                self.active_command = Some(InteractiveCommand::Contour {
                    base,
                    direction,
                    spacing_start,
                    options,
                });
                self.contour_prompt();
                return true;
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
            Ok(false) => {}
        }
        if let (Some(base), Some(direction)) = (base, direction) {
            if input.is_empty()
                || input
                    .trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Enter")
            {
                self.finish_contour(
                    base,
                    direction,
                    self.commands.contour_preferences().spacing,
                    options,
                );
                return true;
            }
            if let Some(quantity) = viboceros_drafting::PointInput::parse_length_with_units(
                input,
                self.document.units(),
            ) {
                match quantity {
                    Ok(spacing) => {
                        self.finish_contour(base, direction, spacing, options);
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                };
            } else {
                return false;
            }
            self.command_input.clear();
            return true;
        }
        false
    }

    fn contour_prompt(&mut self) {
        let Some(InteractiveCommand::Contour {
            base,
            direction,
            spacing_start,
            options,
        }) = self.active_command
        else {
            return;
        };
        let spacing = self.commands.contour_preferences().spacing;
        let point = match (base, direction, spacing_start, options.range) {
            (None, _, _, true) => "Contour range start point".to_owned(),
            (None, _, _, false) => "Contour plane base point (Range)".to_owned(),
            (_, None, _, true) => "End of contour range perpendicular to contour planes".to_owned(),
            (_, None, _, false) => "Direction perpendicular to contour planes".to_owned(),
            (_, _, Some(_), _) => format!("Second distance point <{spacing}>"),
            _ => format!("Distance between contours <{spacing}>"),
        };
        self.push_log(format!("{point}. {}", options.arguments()));
        self.command_input.clear();
    }
}
