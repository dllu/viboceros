//! Command options and temporary curve restriction.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn start_radius(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let Some(word) = words.next() else {
            return false;
        };
        let diameter = match word
            .trim_start_matches(['_', '-'])
            .to_ascii_lowercase()
            .as_str()
        {
            "radius" => false,
            "diameter" => true,
            _ => return false,
        };
        let arguments = words.collect::<Vec<_>>();
        if arguments.is_empty()
            && viboceros_command::preselected_circular_radius(&self.document)
                .ok()
                .flatten()
                .is_some()
        {
            return false;
        }
        let mut mark = self.radius_mark[usize::from(diameter)];
        let mut display_units = self.radius_units[usize::from(diameter)];
        let mut units_seen = false;
        let mut getter = None;
        for argument in arguments {
            let argument = argument.trim_start_matches(['_', '-']);
            if argument.eq_ignore_ascii_case(mark_name(diameter)) {
                mark = !mark;
            } else if ["SelectCurve", "SubCrv", "Units"]
                .iter()
                .any(|word| argument.eq_ignore_ascii_case(word))
            {
                if getter.replace(argument).is_some() {
                    return false;
                }
            } else {
                let Some((key, value)) = argument.split_once('=') else {
                    return false;
                };
                if key.eq_ignore_ascii_case(mark_name(diameter)) {
                    let Some(value) = radius_yes_no(value) else {
                        return false;
                    };
                    mark = value;
                } else if key.eq_ignore_ascii_case("Units")
                    && !units_seen
                    && self.document.selected_object_count() == 0
                {
                    let Ok(units) = viboceros_command::radius_display_units(value) else {
                        return false;
                    };
                    display_units = units;
                    units_seen = true;
                } else {
                    return false;
                }
            }
        }
        self.cancel_interactive_command(false);
        self.radius_mark[usize::from(diameter)] = mark;
        self.radius_units[usize::from(diameter)] = display_units;
        self.active_command = Some(InteractiveCommand::Radius {
            diameter,
            mark,
            display_units,
        });
        self.radius_session = Some(Session::default());
        if let Some(option) = getter {
            self.try_continue_radius(option);
        } else {
            self.log_radius_prompt();
        }
        true
    }

    pub(in crate::app) fn log_radius_prompt(&mut self) {
        let Some(InteractiveCommand::Radius { diameter, .. }) = self.active_command else {
            return;
        };
        let custom = self.radius_session.as_ref().is_some_and(|s| s.selecting);
        let text = if custom {
            "Select a curve or press Enter to measure any curve".to_owned()
        } else {
            let units = if self.document.selected_object_count() == 0 {
                " / Units"
            } else {
                ""
            };
            format!(
                "{}: pick a point on a curve or press Enter when done (SelectCurve / {} / SubCrv{units})",
                name(diameter),
                mark_name(diameter)
            )
        };
        self.push_log(text);
    }

    pub(super) fn choose_radius_curve(&mut self, id: ObjectId) {
        match CapturedCurve::capture(&self.document, id, None) {
            Ok(curve) => {
                let session = self.radius_session.as_mut().unwrap();
                session.source = Some(Source {
                    curve,
                    portion: None,
                });
                session.selecting = false;
                self.log_radius_prompt();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(in crate::app) fn try_continue_radius(&mut self, input: &str) -> bool {
        let Some(
            command @ InteractiveCommand::Radius {
                diameter,
                mark,
                mut display_units,
            },
        ) = self.active_command
        else {
            return false;
        };
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        let nested = self.measuring_nested_radius();
        let Some(session) = &mut self.radius_session else {
            return false;
        };
        let token = input.trim().trim_start_matches(['\'', '_', '-']);
        let enter = token.is_empty() || token.eq_ignore_ascii_case("Enter");
        let (key, value) = token
            .split_once('=')
            .map_or((token, None), |(key, value)| (key, Some(value)));
        if session.choosing_units || key.eq_ignore_ascii_case("Units") {
            if self.document.selected_object_count() != 0 {
                self.push_log(
                    "Error: Radius/Diameter Units is unavailable with preselected objects".into(),
                );
            } else if !session.choosing_units && value.is_none() {
                session.choosing_units = true;
                self.push_log(
                    "Choose Model_Units, Micron, Millimeter, Centimeter, Meter, Kilometer, Microinch, Mil, Decimal_Inches, Fractional_Inches, Decimal_Feet, Feet_Inches, or Mile".into(),
                );
            } else if enter {
                session.choosing_units = false;
                self.log_radius_prompt();
            } else if token.eq_ignore_ascii_case("Cancel") {
                return false;
            } else {
                match viboceros_command::radius_display_units(value.unwrap_or(token)) {
                    Ok(units) => {
                        display_units = units;
                        if !nested {
                            self.radius_units[usize::from(diameter)] = units;
                        }
                        session.choosing_units = false;
                        self.active_command = Some(InteractiveCommand::Radius {
                            diameter,
                            mark,
                            display_units,
                        });
                        self.push_log(format!(
                            "Radius/Diameter display units: {}",
                            units.unwrap_or("Model_Units")
                        ));
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
        } else if key.eq_ignore_ascii_case(mark_name(diameter)) {
            if nested {
                self.push_log("Select a curve location to supply the radius".into());
            } else if let Some(mark) = value.map_or(Some(!mark), radius_yes_no) {
                self.radius_mark[usize::from(diameter)] = mark;
                self.active_command = Some(InteractiveCommand::Radius {
                    diameter,
                    mark,
                    display_units,
                });
                self.log_radius_prompt();
            } else {
                self.push_log(format!("{} takes Yes or No", mark_name(diameter)));
            }
        } else if key.eq_ignore_ascii_case("SelectCurve") {
            if let Some(id) = value.and_then(|value| value.parse().ok()) {
                self.choose_radius_curve(id);
            } else if value.is_none() {
                session.selecting = true;
                self.log_radius_prompt();
            } else {
                self.push_log("SelectCurve requires a valid object ID".into());
            }
        } else if session.selecting && enter {
            session.selecting = false;
            session.source = None;
            session.unrestricted = true;
            self.log_radius_prompt();
        } else if enter {
            if !self.cancel_nested_measurement() {
                self.cancel_interactive_command(false);
            }
        } else if key.eq_ignore_ascii_case("SubCrv") && value.is_none() {
            session.subcurve_resume = Some(command);
            let reference = (!session.selecting)
                .then_some(session.source.as_ref())
                .flatten()
                .map(|source| (source.curve.object_id(), source.curve.source().1))
                .or_else(|| {
                    if session.selecting || nested {
                        return None;
                    }
                    let mut objects = self.document.selected_objects();
                    let object = objects.next()?;
                    (object.geometry().curve_ref().is_some() && objects.next().is_none())
                        .then_some((object.id(), None))
                });
            self.begin_radius_subcurve_prompt(reference);
        } else {
            return false;
        }
        if let Some(session) = &mut self.radius_session {
            session.hover = None;
        }
        self.command_input.clear();
        true
    }

    pub(in crate::app) fn return_to_radius(
        &mut self,
        portion: Option<(CapturedCurve, Curve3)>,
    ) -> Result<(), CommandError> {
        let Some(session) = &mut self.radius_session else {
            return Ok(());
        };
        if let Some((source, curve)) = portion {
            let display = curve.as_ref().to_nurbs()?;
            session.source = Some(Source {
                curve: source,
                portion: Some(Portion { curve, display }),
            });
            session.selecting = false;
        }
        self.active_command = session.subcurve_resume.take();
        self.subcurve_prompt = None;
        self.log_radius_prompt();
        Ok(())
    }
}

fn radius_yes_no(value: &str) -> Option<bool> {
    let value = value.trim_start_matches('_');
    if value.eq_ignore_ascii_case("Yes") {
        Some(true)
    } else if value.eq_ignore_ascii_case("No") {
        Some(false)
    } else {
        None
    }
}
