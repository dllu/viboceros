//! Rail-revolution roles, options and atomic surface construction.
use super::*;
const NAME: &str = "RailRevolve";
const USAGE: &str = "RailRevolve axis-start axis-end [RailName=name|RailId=id] [ScaleHeight=Yes|No] [SplitAtTangents=Yes|No]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RailRevolveOptions {
    pub scale_height: bool,
    pub split_at_tangents: bool,
}
impl Default for RailRevolveOptions {
    fn default() -> Self {
        Self {
            scale_height: false,
            split_at_tangents: true,
        }
    }
}
#[derive(Default)]
pub(super) struct RailRevolveCommand {
    options: remembered::Remembered<RailRevolveOptions>,
}

struct Parsed {
    points: Vec<Point3>,
    rail_name: Option<String>,
    rail_id: Option<ObjectId>,
    options: RailRevolveOptions,
}
fn parse(args: &[&str], mut options: RailRevolveOptions) -> Result<Parsed, CommandError> {
    let mut positions = vec![];
    let (mut rail_name, mut rail_id) = (None, None);
    let (mut height, mut split, mut i) = (false, false, 0);
    while i < args.len() {
        let key = args[i].split('=').next().unwrap();
        if args[i].contains('=')
            || ["ScaleHeight", "SplitAtTangents", "RailName", "RailId"]
                .iter()
                .any(|k| option_name_eq(key, k))
        {
            let (key, value, used) = orient_option(args, i, USAGE)?;
            if option_name_eq(key, "ScaleHeight") && !height {
                options.scale_height = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                height = true;
            } else if option_name_eq(key, "SplitAtTangents") && !split {
                options.split_at_tangents =
                    parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                split = true;
            } else if option_name_eq(key, "RailName")
                && rail_name.is_none()
                && rail_id.is_none()
                && !value.is_empty()
            {
                rail_name = Some(value.to_owned());
            } else if option_name_eq(key, "RailId") && rail_id.is_none() && rail_name.is_none() {
                rail_id = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?);
            } else {
                return Err(CommandError::Usage(USAGE));
            }
            i += used;
        } else {
            positions.push(args[i]);
            i += 1;
        }
    }
    let mut points = vec![];
    let mut i = 0;
    while i < positions.len() {
        let (point, used) = parse_point(&positions[i..])?;
        points.push(point);
        i += used;
        if points.len() > 2 {
            return Err(CommandError::Usage(USAGE));
        }
    }
    Ok(Parsed {
        points,
        rail_name,
        rail_id,
        options,
    })
}

fn roles(doc: &Document, parsed: &Parsed) -> Result<(ObjectId, ObjectId), CommandError> {
    let selected = selected_ids(doc)?;
    let rail = if let Some(id) = parsed.rail_id {
        id
    } else if parsed.rail_name.is_some() {
        extrude_along_curve::resolve_curve_along_curve_path(
            doc,
            &selected,
            parsed.rail_name.as_deref(),
        )?
    } else {
        if selected.len() != 2 {
            return Err(CommandError::Usage(USAGE));
        }
        selected[1]
    };
    let profiles = selected
        .into_iter()
        .filter(|id| *id != rail)
        .collect::<Vec<_>>();
    let [profile] = profiles.as_slice() else {
        return Err(CommandError::Usage(USAGE));
    };
    for id in [*profile, rail] {
        if !doc.is_object_selectable(id)
            || doc
                .object(id)
                .is_none_or(|o| o.geometry().curve_ref().is_none())
        {
            return Err(CommandError::Usage(USAGE));
        }
    }
    Ok((*profile, rail))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailRevolveStart {
    pub axis_start: Option<Point3>,
    pub profile: ObjectId,
    pub rail: ObjectId,
    pub options: RailRevolveOptions,
}
pub fn start(
    doc: &Document,
    args: &[&str],
    options: RailRevolveOptions,
) -> Result<Option<RailRevolveStart>, CommandError> {
    let parsed = parse(args, options)?;
    if parsed.points.len() == 2 {
        return Ok(None);
    }
    let (profile, rail) = roles(doc, &parsed)?;
    Ok(Some(RailRevolveStart {
        axis_start: parsed.points.first().copied(),
        profile,
        rail,
        options: parsed.options,
    }))
}
pub fn selection_input(input: &str, options: RailRevolveOptions) -> Result<String, CommandError> {
    let args = input.split_whitespace().skip(1).collect::<Vec<_>>();
    let p = parse(&args, options)?;
    let mut result = String::from(NAME);
    for q in p.points {
        result.push_str(&format!(" {},{},{}", q.x(), q.y(), q.z()));
    }
    if let Some(name) = p.rail_name {
        result.push_str(&format!(" RailName={name}"));
    }
    if let Some(id) = p.rail_id {
        result.push_str(&format!(" RailId={id}"));
    }
    result.push_str(&options.arguments());
    Ok(result)
}
impl RailRevolveOptions {
    pub fn from_prompt(prompt: &ObjectSelectionPrompt) -> Self {
        Self {
            scale_height: prompt
                .options
                .iter()
                .any(|o| o.name == "ScaleHeight" && o.value),
            split_at_tangents: prompt
                .options
                .iter()
                .any(|o| o.name == "SplitAtTangents" && o.value),
        }
    }
    pub fn update(self, input: &str) -> Result<Self, CommandError> {
        let args = input.split_whitespace().collect::<Vec<_>>();
        let p = parse(&args, self)?;
        if !p.points.is_empty() || p.rail_name.is_some() || p.rail_id.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(p.options)
    }
    pub fn arguments(self) -> String {
        format!(
            " ScaleHeight={} SplitAtTangents={}",
            if self.scale_height { "Yes" } else { "No" },
            if self.split_at_tangents { "Yes" } else { "No" }
        )
    }
}
impl Command for RailRevolveCommand {
    fn name(&self) -> &'static str {
        NAME
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let p = parse(args, self.options.get())?;
        Ok(Some(ObjectSelectionPrompt {
            command: NAME,
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![
                BooleanSelectionOption {
                    name: "ScaleHeight",
                    value: p.options.scale_height,
                    aliases: &[],
                },
                BooleanSelectionOption {
                    name: "SplitAtTangents",
                    value: p.options.split_at_tangents,
                    aliases: &[],
                },
            ],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        self.options.set(parse(args, self.options.get())?.options);
        Ok(())
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let parsed = parse(args, self.options.get())?;
        let [origin, end] = parsed.points.as_slice() else {
            return Err(CommandError::Usage(USAGE));
        };
        let axis = origin.vector_to(*end)?.normalized(doc.tolerance())?;
        let (profile, rail) = roles(doc, &parsed)?;
        let curve = |id| {
            doc.object(id)
                .unwrap()
                .geometry()
                .nurbs_curve_representation()?
                .ok_or(CommandError::Usage(USAGE))
        };
        let geometry = construct(
            &curve(profile)?,
            &curve(rail)?,
            *origin,
            axis,
            parsed.options,
            doc.tolerance(),
        )?;
        doc.add_geometry(geometry)?;
        self.options.set(parsed.options);
        Ok("Created a rail-revolved surface".into())
    }
}
fn construct(
    profile: &NurbsCurve,
    rail: &NurbsCurve,
    origin: Point3,
    axis: UnitVector3,
    options: RailRevolveOptions,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        if options.split_at_tangents {
            Ok(Geometry::Brep(viboceros_smlib::rail_revolved_brep(
                profile,
                rail,
                origin,
                axis,
                options.scale_height,
                tolerance,
            )?))
        } else {
            Ok(Geometry::NurbsSurface(
                viboceros_smlib::rail_revolved_surface(
                    profile,
                    rail,
                    origin,
                    axis,
                    options.scale_height,
                    tolerance,
                )?,
            ))
        }
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = (profile, rail, origin, axis, options, tolerance);
        Err(CommandError::Usage(
            "RailRevolve requires the native-smlib feature",
        ))
    }
}

#[cfg(test)]
mod tests;
