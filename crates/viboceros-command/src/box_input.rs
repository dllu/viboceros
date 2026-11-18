//! Shared solid and mesh box construction from rectangle and diagonal getters.
use super::*;
use crate::rectangle_input::{RectangleDefinition, RectangleInput, RectangleMode, RectangleScript};

pub use crate::subd_box::{Definition, base_plane, default_height, height_at};

pub const BOX_USAGE: &str = "Box [Center|3Point [EdgeMidpoint]|Vertical|Diagonal [Cube]] points-or-dimensions [height-or-point|Enter]";
pub const MESH_BOX_USAGE: &str = "MeshBox [XCount=n YCount=n ZCount=n] [Center|3Point [EdgeMidpoint]|Vertical|Diagonal [Cube]|AroundCurve] points-or-dimensions [height-or-point|Enter]";

pub fn rectangle_option(input: &mut RectangleInput, text: &str, mesh: bool) -> bool {
    (["Center", "3Point", "Vertical"]
        .iter()
        .any(|name| option_name_eq(text, name))
        || (input.mode() == RectangleMode::ThreePoint && option_name_eq(text, "EdgeMidpoint"))
        || (mesh && option_name_eq(text, "AroundCurve")))
        && input.option(text)
}

#[derive(Clone, Copy, Debug)]
pub struct Diagonal {
    pub frame: Frame3,
    pub cube: bool,
}

impl Diagonal {
    pub fn definition(self, point: Point3) -> Result<Definition, GeometryError> {
        let [mut x, mut y, mut z] = self.frame.coordinates_of(point)?;
        if self.cube {
            // Orthogonal projection onto the space diagonal of the picked
            // octant. In-plane picks take the lower half of that octant.
            let side = (x.abs() + y.abs() + z.abs()) / 3.;
            x = if x < 0. { -side } else { side };
            y = if y < 0. { -side } else { side };
            z = if z <= 0. { -side } else { side };
        }
        let origin = self.frame.origin();
        let x_corner = self.frame.point_at([x, 0., 0.])?;
        let y_corner = self.frame.point_at([0., y, 0.])?;
        let base = RectangleDefinition::from_three_points(origin, x_corner, y_corner)?;
        Ok(Definition {
            base,
            height: z * x.signum() * y.signum(),
        })
    }
}

pub fn square_corner(frame: Frame3, cursor: Point3) -> Result<Point3, GeometryError> {
    let [x, y, _] = frame.coordinates_of(cursor)?;
    let size = x.abs().max(y.abs());
    frame.point_at([size.copysign(x), size.copysign(y), 0.])
}

pub fn solid(
    definition: Definition,
    extrusion: bool,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let frame = base_plane(definition.base)?;
    let [width, depth] = base_dimensions(definition.base)?;
    let output = Brep::try_box_extrusion(
        frame,
        [[0., width], [0., depth]],
        definition.height,
        tolerance,
    )?;
    if extrusion {
        Ok(output)
    } else {
        output.try_extrusion_brep_form(tolerance)
    }
}

pub fn mesh(
    definition: Definition,
    [x, y, z]: [usize; 3],
    ordered_height: bool,
    tolerance: Tolerance,
) -> Result<TriangleMesh, GeometryError> {
    let frame = base_plane(definition.base)?;
    let [width, depth] = base_dimensions(definition.base)?;
    if ordered_height {
        return TriangleMesh::try_box_grid(
            frame,
            [[0., width], [0., depth], [0., definition.height]],
            x,
            y,
            z,
            tolerance,
        );
    }
    // Rhino orders the base grid before the top and flips Y/Z for positive
    // heights. Retain that vertex and face ordering for the box mesh.
    let (frame, side_y) = if definition.height > 0. {
        (
            Frame3::try_from_unit_directions(
                frame.origin(),
                frame.x_axis(),
                frame.y_axis().opposite(),
                tolerance,
            )?,
            [-depth, 0.],
        )
    } else {
        (frame, [0., depth])
    };
    TriangleMesh::try_box_grid(
        frame,
        [[0., width], side_y, [0., -definition.height.abs()]],
        x,
        y,
        z,
        tolerance,
    )
}

fn base_dimensions(base: RectangleDefinition) -> Result<[Real; 2], GeometryError> {
    Ok([
        base.corners[0].distance_to(base.corners[1])?,
        default_height(base)?,
    ])
}

pub fn check_mesh_counts([x, y, z]: [usize; 3]) -> Result<(), CommandError> {
    if x == 0 || y == 0 || z == 0 {
        return Err(CommandError::Usage(MESH_BOX_USAGE));
    }
    if x.checked_mul(y)
        .and_then(|xy| x.checked_mul(z).and_then(|xz| xy.checked_add(xz)))
        .and_then(|sum| y.checked_mul(z).and_then(|yz| sum.checked_add(yz)))
        .and_then(|sum| sum.checked_mul(2))
        .is_none_or(|faces| faces > MAX_MESH_BOX_FACES)
    {
        return Err(GeometryError::TooManyMeshFaces.into());
    }
    Ok(())
}

pub fn mesh_count(
    counts: [usize; 3],
    args: &[&str],
) -> Result<Option<([usize; 3], usize)>, CommandError> {
    let Some(token) = args.first().map(|word| word.trim_start_matches('_')) else {
        return Ok(None);
    };
    if let Ok(count) = token.parse::<usize>() {
        if count == 0 {
            return Ok(None);
        }
        let counts = [count; 3];
        return Ok(Some((counts, 1)));
    }
    let (key, value) = token
        .split_once('=')
        .map_or((token, None), |(key, value)| (key, Some(value)));
    let Some(axis) = ["XCount", "YCount", "ZCount"]
        .iter()
        .position(|name| option_name_eq(key, name))
    else {
        return Ok(None);
    };
    let (value, consumed) = if let Some(value) = value {
        (value, 1)
    } else {
        (*args.get(1).ok_or(CommandError::Usage(MESH_BOX_USAGE))?, 2)
    };
    let size = value
        .trim_start_matches('_')
        .parse::<usize>()
        .ok()
        .filter(|&n| n > 0);
    let mut updated = counts;
    updated[axis] = size.ok_or_else(|| CommandError::InvalidMeshBoxFaceCount(value.to_owned()))?;
    Ok(Some((updated, consumed)))
}

pub(super) fn is_extended(argument: &str) -> bool {
    [
        "Diagonal",
        "Cube",
        "Center",
        "3Point",
        "Vertical",
        "AroundCurve",
        "Enter",
    ]
    .iter()
    .any(|name| option_name_eq(argument, name))
}

pub(super) struct ScriptedBox {
    pub definition: Definition,
    pub counts: Option<[usize; 3]>,
    pub ordered_height: bool,
}

pub(super) fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    mut counts: Option<[usize; 3]>,
) -> Result<ScriptedBox, CommandError> {
    let usage = if counts.is_some() {
        MESH_BOX_USAGE
    } else {
        BOX_USAGE
    };
    let mut index = 0;
    let mut rectangle = RectangleScript::new(RectangleInput::new(plane));
    let mut diagonal = None;
    let mut base = None;
    while index < args.len() && base.is_none() {
        let word = args[index];
        if rectangle.input.anchor().is_none()
            && !rectangle.has_curve()
            && let Some(current) = counts
            && let Some((update, consumed)) = mesh_count(current, &args[index..])?
        {
            counts = Some(update);
            index += consumed;
            continue;
        }
        if rectangle.input.anchor().is_none() && option_name_eq(word, "Diagonal") {
            diagonal = Some(false);
            index += 1;
            continue;
        }
        if diagonal.is_some() && option_name_eq(word, "Cube") {
            diagonal = Some(true);
            index += 1;
            continue;
        }
        if let Some(cube) = diagonal {
            let (first, consumed) = parse_point(&args[index..])?;
            index += consumed;
            let (second, consumed) = parse_point(&args[index..])?;
            index += consumed;
            let definition = Diagonal {
                frame: plane.with_origin(first),
                cube,
            }
            .definition(second)?;
            if definition.height.abs() > doc.tolerance().absolute() {
                require_consumed(args, index, usage)?;
                return Ok(ScriptedBox {
                    definition,
                    counts,
                    ordered_height: true,
                });
            }
            base = Some(definition.base);
        } else if !rectangle.has_curve()
            && rectangle_option(&mut rectangle.input, word, counts.is_some())
        {
            index += 1;
        } else {
            let (consumed, definition) = rectangle.value(doc, &args[index..], false)?;
            index += consumed;
            base = definition;
        }
    }
    let base = base.ok_or(CommandError::Usage(usage))?;
    let height = match args.get(index..) {
        Some([]) if diagonal.is_none() => default_height(base)?,
        Some([word]) if option_name_eq(word, "Enter") && diagonal.is_none() => {
            default_height(base)?
        }
        Some([word]) if !word.contains(',') && diagonal.is_none() => parse_finite_real(word)?,
        Some(tokens) if !tokens.is_empty() => {
            let (point, consumed) = parse_point(tokens)?;
            require_consumed(tokens, consumed, usage)?;
            height_at(base, point)?
        }
        _ => return Err(CommandError::Usage(usage)),
    };
    Ok(ScriptedBox {
        definition: Definition { base, height },
        counts,
        ordered_height: diagonal.is_some() || rectangle.input.mode() == RectangleMode::Vertical,
    })
}
