use super::*;
pub const USAGE: &str = "ExtrudeSubD [distance|Distance=value] [Faces=0,1] [Edges=0,1] [Basis=WCS|UVN] [Direction=X|Y|Z|N|V|x,y,z]";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    Wcs,
    Uvn,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Direction {
    X,
    Y,
    Z,
    N,
    V,
    Vector(Vector3),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub distance: f64,
    pub basis: Basis,
    pub direction: Direction,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            distance: 1.,
            basis: Basis::Uvn,
            direction: Direction::N,
        }
    }
}
impl Options {
    pub fn validate(self) -> Result<(), CommandError> {
        if !self.distance.is_finite()
            || self.distance == 0.
            || !matches!(
                (self.basis, self.direction),
                (
                    Basis::Wcs,
                    Direction::X | Direction::Y | Direction::Z | Direction::Vector(_)
                ) | (Basis::Uvn, Direction::N | Direction::V)
            )
        {
            return Err(CommandError::Usage(USAGE));
        }
        self.geometry_direction()?;
        Ok(())
    }
    pub(super) fn geometry_direction(self) -> Result<SubDExtrudeDirection, CommandError> {
        Ok(match self.direction {
            Direction::N => SubDExtrudeDirection::VertexNormals,
            Direction::V => SubDExtrudeDirection::EdgeOutward,
            other => SubDExtrudeDirection::World(match other {
                Direction::X => Vector3::try_new(1., 0., 0.)?,
                Direction::Y => Vector3::try_new(0., 1., 0.)?,
                Direction::Z => Vector3::try_new(0., 0., 1.)?,
                Direction::Vector(v) => v.normalized_nonzero()?.as_vector(),
                _ => unreachable!(),
            }),
        })
    }
    pub fn command(self) -> String {
        let direction = match self.direction {
            Direction::X => "X".into(),
            Direction::Y => "Y".into(),
            Direction::Z => "Z".into(),
            Direction::N => "N".into(),
            Direction::V => "V".into(),
            Direction::Vector(v) => {
                let [x, y, z] = v.to_array();
                format!("{x},{y},{z}")
            }
        };
        format!(
            "ExtrudeSubD Distance={} Basis={} Direction={direction}",
            self.distance,
            if self.basis == Basis::Wcs {
                "WCS"
            } else {
                "UVN"
            }
        )
    }
}
#[derive(Clone, Debug)]
pub struct Input {
    pub options: Options,
    pub distance_supplied: bool,
    pub selection: Option<SubDExtrudeSelection>,
}
pub fn parse(args: &[&str], mut options: Options) -> Result<Input, CommandError> {
    let mut seen = [false; 5];
    let mut direction = None;
    let mut faces = vec![];
    let mut edges = vec![];
    let mut i = 0;
    while i < args.len() {
        let (name, value, consumed) = if let Some((n, v)) = args[i].split_once('=') {
            (n, v, 1)
        } else if parse_finite_real(args[i]).is_ok() {
            ("Distance", args[i], 1)
        } else {
            (
                args[i],
                *args.get(i + 1).ok_or(CommandError::Usage(USAGE))?,
                2,
            )
        };
        let index = if option_name_eq(name, "Distance") {
            options.distance = parse_finite_real(value)?;
            0
        } else if option_name_eq(name, "Basis") {
            let basis = if option_name_eq(value, "WCS") {
                Basis::Wcs
            } else if option_name_eq(value, "UVN") {
                Basis::Uvn
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if basis != options.basis {
                options.direction = if basis == Basis::Wcs {
                    Direction::Z
                } else {
                    Direction::N
                };
            }
            options.basis = basis;
            1
        } else if option_name_eq(name, "Direction") {
            direction = Some(value);
            2
        } else if option_name_eq(name, "Faces") {
            faces = indices(value)?;
            3
        } else if option_name_eq(name, "Edges") {
            edges = indices(value)?;
            4
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if std::mem::replace(&mut seen[index], true) {
            return Err(CommandError::Usage(USAGE));
        }
        i += consumed;
    }
    if let Some(value) = direction {
        options.direction = if option_name_eq(value, "X") {
            Direction::X
        } else if option_name_eq(value, "Y") {
            Direction::Y
        } else if option_name_eq(value, "Z") {
            Direction::Z
        } else if option_name_eq(value, "N") {
            Direction::N
        } else if option_name_eq(value, "V") {
            Direction::V
        } else {
            let v = value
                .split(',')
                .map(parse_finite_real)
                .collect::<Result<Vec<_>, _>>()?;
            let [x, y, z] = v.as_slice() else {
                return Err(CommandError::Usage(USAGE));
            };
            Direction::Vector(
                Vector3::try_new(*x, *y, *z)?
                    .normalized_nonzero()?
                    .as_vector(),
            )
        };
    }
    options.validate()?;
    Ok(Input {
        options,
        distance_supplied: seen[0],
        selection: (seen[3] || seen[4])
            .then_some(SubDExtrudeSelection::Components { faces, edges }),
    })
}
fn indices(value: &str) -> Result<Vec<usize>, CommandError> {
    if value.len() > 4_000_000 {
        return Err(CommandError::Usage(USAGE));
    }
    value
        .split(',')
        .enumerate()
        .map(|(i, v)| {
            if i >= 400_000 {
                return Err(CommandError::Usage(USAGE));
            }
            v.parse().map_err(|_| CommandError::Usage(USAGE))
        })
        .collect()
}
