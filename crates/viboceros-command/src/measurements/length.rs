//! Selected curve and edge lengths without temporary document objects.
use super::*;
mod components;
pub(crate) struct LengthCommand {
    selection: crate::component_selection::ComponentSelection,
}
impl LengthCommand {
    pub(crate) fn new(selection: crate::component_selection::ComponentSelection) -> Self {
        Self { selection }
    }
}
const LENGTH_USAGE: &str =
    "Length [SubCrv Parameter=start,end|SubCrv start_point end_point] [Units=name]";

fn length_report(count: usize, total: Real, target: Option<&LengthUnitSystem>) -> String {
    format!(
        "Measured {count} curve(s): total length {}{}",
        format_measurement(total),
        target
            .map(|unit| format!(" {}", unit.name()))
            .unwrap_or_default()
    )
}

impl Command for LengthCommand {
    fn name(&self) -> &'static str {
        "Length"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Len"]
    }

    fn records_history(&self) -> bool {
        false
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let subcurve = arguments
            .first()
            .is_some_and(|option| option_name_eq(option, "SubCrv"));
        let eligible = match arguments {
            [] => true,
            [option] if subcurve => true,
            [option] => {
                measurement_display_option(option, LENGTH_USAGE)?;
                true
            }
            [_, units] if subcurve => {
                measurement_display_option(units, LENGTH_USAGE)?;
                true
            }
            _ => false,
        };
        Ok(eligible.then_some(ObjectSelectionPrompt {
            command: "Length",
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: crate::CommandContext,
    ) -> Result<String, CommandError> {
        let report = self.run(document, arguments)?;
        document.clear_selection();
        self.selection.clear();
        Ok(report)
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (arguments, target) = match arguments.last() {
            Some(last)
                if last
                    .split_once('=')
                    .is_some_and(|(name, _)| option_name_eq(name, "Units")) =>
            {
                (
                    &arguments[..arguments.len() - 1],
                    measurement_display_option(last, LENGTH_USAGE)?,
                )
            }
            _ => (arguments, None),
        };
        let scale = measurement_display_scale(
            document,
            target.as_ref(),
            "Length display conversion requires physical source units",
        )?;
        if arguments
            .first()
            .is_some_and(|argument| option_name_eq(argument, "SubCrv"))
        {
            let mut selected = document.selected_objects();
            let object = selected.next().ok_or(CommandError::NoObjectsSelected)?;
            if selected.next().is_some() {
                return Err(CommandError::Usage(
                    "Length SubCrv requires one selected curve",
                ));
            }
            let curve = geometry_curve_ref(object.geometry())
                .ok_or(CommandError::UnsupportedLengthGeometry)?;
            let part = subcurve::parse_subcurve(
                curve,
                document.tolerance(),
                &arguments[1..],
                LENGTH_USAGE,
            )?;
            let total = part.as_ref().length(document.tolerance())?;
            let total = distance::display_value(total, scale)?;
            return Ok(length_report(1, total, target.as_ref()));
        }
        require_consumed(arguments, 0, "Length")?;
        let (count, sum) = accumulate_length(document, &self.selection)?;
        let total = if scale == 1.0 {
            sum.total()?
        } else {
            sum.scaled_total(scale).map_err(|_| {
                viboceros_document::DocumentError::from(
                    viboceros_geometry::UnitError::UnrepresentableScale,
                )
            })?
        };
        if total == 0.0 && sum.total()? != 0.0 {
            return Err(viboceros_document::DocumentError::from(
                viboceros_geometry::UnitError::UnrepresentableScale,
            )
            .into());
        }
        self.selection.clear();
        Ok(length_report(count, total, target.as_ref()))
    }
}

fn accumulate_length(
    document: &Document,
    selection: &crate::component_selection::ComponentSelection,
) -> Result<(usize, FiniteSum), CommandError> {
    let picks = selection
        .checked_picks(document)
        .map_err(CommandError::Usage)?;
    let mut count = 0;
    let mut sum = FiniteSum::default();
    let mut add = |value: f64| -> Result<(), CommandError> {
        if !value.is_finite() || value < 0. {
            return Err(GeometryError::NonFinite {
                context: "curve length",
            }
            .into());
        }
        sum.add(value)?;
        count += 1;
        Ok(())
    };
    for object in document.selected_objects() {
        let curve =
            geometry_curve_ref(object.geometry()).ok_or(CommandError::UnsupportedLengthGeometry)?;
        add(curve.length(document.tolerance())?)?;
    }
    components::accumulate(document, picks, &mut add)?;
    if count == 0 {
        return Err(CommandError::NoObjectsSelected);
    }
    Ok((count, sum))
}
