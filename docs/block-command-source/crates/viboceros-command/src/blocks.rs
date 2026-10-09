//! Shared-definition commands and borrowed quoted-name parsing.
use super::*;
use viboceros_document::BlockReference;

pub const BLOCK_USAGE: &str = "Block base-point block-name";
pub const INSERT_USAGE: &str =
    "Insert block-name point [Scale=n|x,y,z] [Rotation=degrees] [Axis=x,y,z]";

pub fn base_point(arguments: &[&str]) -> Result<(Point3, usize), CommandError> {
    parse_point(arguments)
}

/// Quoted names retain internal whitespace; no escapes or trailing quote text.
pub fn tokenize(input: &str) -> Result<Vec<&str>, CommandError> {
    let mut rest = input.trim();
    let mut result = Vec::new();
    while !rest.is_empty() {
        if let Some(quoted) = rest.strip_prefix('"') {
            let end = quoted
                .find('"')
                .ok_or(CommandError::Usage("unterminated quoted block name"))?;
            let token = &quoted[..end];
            rest = &quoted[end + 1..];
            if token.trim().is_empty() || rest.chars().next().is_some_and(|ch| !ch.is_whitespace())
            {
                return Err(CommandError::Usage("invalid quoted block name"));
            }
            result.push(token);
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let token = &rest[..end];
            if token.contains('"') {
                return Err(CommandError::Usage("invalid quoted block name"));
            }
            result.push(token);
            rest = &rest[end..];
        }
        rest = rest.trim_start();
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InsertOptions {
    pub scale: [f64; 3],
    pub rotation_degrees: f64,
    pub axis: [f64; 3],
}

impl Default for InsertOptions {
    fn default() -> Self {
        Self {
            scale: [1.; 3],
            rotation_degrees: 0.,
            axis: [0., 0., 1.],
        }
    }
}

impl InsertOptions {
    pub fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
        let mut options = Self::default();
        let mut seen = BTreeSet::new();
        for word in arguments {
            let (name, value) = word
                .split_once('=')
                .ok_or(CommandError::Usage(INSERT_USAGE))?;
            let name = name.trim_start_matches('_').to_ascii_lowercase();
            let name = if name == "rotate" {
                "rotation".to_owned()
            } else {
                name
            };
            if !seen.insert(name.clone()) {
                return Err(CommandError::Usage(INSERT_USAGE));
            }
            match name.as_str() {
                "scale" => {
                    options.scale = if value.contains(',') {
                        vector(value)?
                    } else {
                        [parse_finite_real(value)?; 3]
                    }
                }
                "rotation" => options.rotation_degrees = parse_finite_real(value)?,
                "axis" => options.axis = vector(value)?,
                _ => return Err(CommandError::Usage(INSERT_USAGE)),
            }
        }
        options.placement(Point3::try_new(0., 0., 0.)?)?;
        Ok(options)
    }

    /// Scale in definition/world XYZ axes, rotate around the specified world
    /// axis (World Z by default), then translate to the insertion point.
    pub fn placement(self, point: Point3) -> Result<AffineTransform3, GeometryError> {
        let origin = Point3::try_new(0., 0., 0.)?;
        let scale = AffineTransform3::try_nonuniform_scale(origin, self.scale)?;
        scale.orientation_reversing()?;
        let axis = Vector3::try_from(self.axis)?.normalized_nonzero()?;
        let rotation = super::rotation_policy::command_rotation(
            origin,
            axis,
            self.rotation_degrees.to_radians(),
        )?;
        scale
            .then(rotation)?
            .then(AffineTransform3::from_translation(Vector3::try_from(
                point.to_array(),
            )?))
    }

    pub fn command_options(self) -> String {
        format!(
            "Scale={},{},{} Rotation={} Axis={},{},{}",
            self.scale[0],
            self.scale[1],
            self.scale[2],
            self.rotation_degrees,
            self.axis[0],
            self.axis[1],
            self.axis[2]
        )
    }
}

fn vector(value: &str) -> Result<[f64; 3], CommandError> {
    let values = value.split(',').collect::<Vec<_>>();
    let [x, y, z] = values.as_slice() else {
        return Err(CommandError::Usage(INSERT_USAGE));
    };
    Ok([
        parse_finite_real(x)?,
        parse_finite_real(y)?,
        parse_finite_real(z)?,
    ])
}

pub(super) struct BlockCommand;
impl Command for BlockCommand {
    fn name(&self) -> &'static str {
        "Block"
    }
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        tokenize(input)
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(arguments.is_empty().then(|| ObjectSelectionPrompt {
            command: "Block",
            filter: ObjectSelectionFilter::Any,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::PointInputAfterSelection,
        }))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (point, consumed) = parse_point(arguments)?;
        let name = arguments
            .get(consumed)
            .ok_or(CommandError::Usage(BLOCK_USAGE))?;
        require_consumed(arguments, consumed + 1, BLOCK_USAGE)?;
        let sources = document.selected_object_ids().collect::<Vec<_>>();
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let (definition, instance) = document.create_block_from_objects(*name, point, sources)?;
        Ok(format!(
            "Created block '{}' ({definition}), instance {instance}",
            document.block_definition(definition).unwrap().name()
        ))
    }
}

pub(super) struct InsertCommand;
impl Command for InsertCommand {
    fn name(&self) -> &'static str {
        "Insert"
    }
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        tokenize(input)
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let name = arguments.first().ok_or(CommandError::Usage(INSERT_USAGE))?;
        let (point, consumed) = parse_point(&arguments[1..])?;
        let options = InsertOptions::parse(&arguments[1 + consumed..])?;
        let definition = document
            .block_definition_by_name(name)
            .ok_or(CommandError::BlockNameNotFound((*name).into()))?
            .id();
        let reference = BlockReference::try_new(definition, options.placement(point)?)?;
        let instance = document.add_block_instance(reference)?;
        Ok(format!("Inserted block '{name}', instance {instance}"))
    }
}

#[cfg(test)]
mod tests;
