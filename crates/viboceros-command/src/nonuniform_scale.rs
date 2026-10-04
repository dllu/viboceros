//! Construction-plane scaling with independent signed axis factors.
use super::*;

pub const USAGE: &str =
    "ScaleNU origin x-factor y-factor z-factor [WorldCoordinates] [Copy=Yes|No]";

pub(super) struct ScaleNonUniformCommand([remembered::Remembered<Real>; 3]);

impl Default for ScaleNonUniformCommand {
    fn default() -> Self {
        Self(std::array::from_fn(|_| remembered::Remembered::new(1.)))
    }
}

impl Command for ScaleNonUniformCommand {
    fn name(&self) -> &'static str {
        "ScaleNU"
    }

    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }

    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }

    fn axis_scale_defaults(&self) -> Option<[Real; 3]> {
        Some(std::array::from_fn(|axis| self.0[axis].get()))
    }

    fn remember_axis_scale(&self, axis: usize, value: Real) -> bool {
        if let Some(preference) = self.0.get(axis) {
            preference.set(value);
            true
        } else {
            false
        }
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (arguments, sources) = affine_transform_arguments(document, arguments, USAGE)?;
        let (positional, world, copy) = options(&arguments, false)?;
        let (origin, mut consumed) = parse_point(&positional)?;
        let plane = if world {
            CommandContext::default().construction_plane
        } else {
            context.construction_plane
        };
        let mut factors = [1.; 3];
        for (axis, factor) in factors.iter_mut().enumerate() {
            let token = positional.get(consumed).ok_or(CommandError::Usage(USAGE))?;
            if token.contains(',') {
                let (reference, first) = parse_point(&positional[consumed..])?;
                let mut target_tokens = &positional[consumed + first..];
                let distance = if target_tokens
                    .first()
                    .is_some_and(|token| !token.contains(','))
                {
                    let distance = parse_finite_real(target_tokens[0])?.abs();
                    target_tokens = &target_tokens[1..];
                    Some(distance)
                } else {
                    None
                };
                if !target_tokens
                    .first()
                    .is_some_and(|token| token.contains(','))
                {
                    return Err(CommandError::Usage(USAGE));
                }
                let (target, second) = parse_point(target_tokens)?;
                *factor = constrained_reference_factor(
                    plane,
                    origin,
                    axis,
                    reference,
                    target,
                    distance,
                    document.tolerance(),
                )?;
                consumed += first + second + usize::from(distance.is_some());
            } else {
                *factor = parse_finite_real(token)?;
                consumed += 1;
            }
        }
        require_consumed(&positional, consumed, USAGE)?;
        let transform = scale_map(plane, origin, factors)?;
        for (axis, value) in factors.into_iter().enumerate() {
            self.remember_axis_scale(axis, value);
        }
        let (changed, copied) = apply_transform_with_renewal(document, &sources, transform, copy)?;
        Ok(format!(
            "Scaled {changed} object(s) by {:.6},{:.6},{:.6}, creating {copied} copy object(s)",
            factors[0], factors[1], factors[2]
        ))
    }
}

fn options<'a>(
    arguments: &[&'a str],
    default_copy: bool,
) -> Result<(Vec<&'a str>, bool, bool), CommandError> {
    let mut world = false;
    let mut seen_world = false;
    let mut remaining = Vec::with_capacity(arguments.len());
    for &token in arguments {
        if option_name_eq(token, "WorldCoordinates") {
            if seen_world {
                return Err(CommandError::Usage(USAGE));
            }
            seen_world = true;
            world = true;
        } else {
            remaining.push(token);
        }
    }
    let has_copy = remaining.iter().any(|token| {
        token
            .split_once('=')
            .is_some_and(|(name, _)| option_name_eq(name, "Copy"))
    });
    let (positional, copy) = parse_transform_copy_arguments(&remaining, USAGE)?;
    Ok((
        positional,
        world,
        if has_copy { copy } else { default_copy },
    ))
}

/// Starting options contain no origin or factors. The command registry expands
/// bare Copy name/value pairs for full invocations; prompts accept those too.
pub fn start_options(arguments: &[&str], default_copy: bool) -> Result<(bool, bool), CommandError> {
    let copy_pair;
    let arguments = if let [name, value] = arguments
        && option_name_eq(name, "Copy")
    {
        copy_pair = format!("Copy={value}");
        vec![copy_pair.as_str()]
    } else {
        arguments.to_vec()
    };
    let (positional, world, copy) = options(&arguments, default_copy)?;
    if !positional.is_empty() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok((world, copy))
}

pub fn scale_map(
    plane: Frame3,
    origin: Point3,
    factors: [Real; 3],
) -> Result<AffineTransform3, GeometryError> {
    if factors == [1.; 3] {
        Ok(AffineTransform3::identity())
    } else {
        let frame = plane.with_origin(origin);
        AffineTransform3::try_frame_mapping(frame, frame, factors)
    }
}

/// Reference picks measure unsigned distances along the active frame axis.
/// Negative factors are explicit numeric input. Viewport input can constrain
/// free mouse picks to this line before evaluating the map.
pub fn reference_factor(
    plane: Frame3,
    origin: Point3,
    axis: usize,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    constrained_reference_factor(plane, origin, axis, reference, target, None, tolerance)
}

pub fn constrained_reference_factor(
    plane: Frame3,
    origin: Point3,
    axis: usize,
    reference: Point3,
    target: Point3,
    distance: Option<Real>,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    let frame = plane.with_origin(origin);
    let length = frame.coordinate_of(axis, reference)?.abs();
    if !length.is_finite() || length <= tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "ScaleNU axis reference",
        }
        .into());
    }
    let numerator = match distance {
        Some(distance) => distance,
        None => frame.coordinate_of(axis, target)?.abs(),
    };
    let factor = numerator.abs() / length;
    if factor.is_finite() {
        Ok(factor)
    } else {
        Err(CommandError::InvalidScaleFactor(factor.to_string()))
    }
}

/// State is shared by typed factors, reference picks and viewport previews.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleNuPrompt {
    pub origin: Option<Point3>,
    pub factors: [Option<Real>; 3],
    pub reference: Option<Point3>,
    pub distance: Option<Real>,
    pub world: bool,
}

impl ScaleNuPrompt {
    pub const fn new(world: bool) -> Self {
        Self {
            origin: None,
            factors: [None; 3],
            reference: None,
            distance: None,
            world,
        }
    }

    pub fn axis(self) -> Option<usize> {
        self.factors.iter().position(Option::is_none)
    }

    pub const fn prompt(self) -> &'static str {
        if self.origin.is_none() {
            return "ScaleNU: pick the origin (WorldCoordinates; Copy=Yes|No; Esc cancels)";
        }
        match (self.factors, self.reference) {
            ([None, _, _], None) => {
                "ScaleNU: X factor or first reference point (Enter accepts default)"
            }
            ([None, _, _], Some(_)) => "ScaleNU: second X reference point",
            ([Some(_), None, _], None) => {
                "ScaleNU: Y factor or first reference point (Enter accepts default)"
            }
            ([Some(_), None, _], Some(_)) => "ScaleNU: second Y reference point",
            (_, None) => "ScaleNU: Z factor or first reference point (Enter accepts default)",
            (_, Some(_)) => "ScaleNU: second Z reference point",
        }
    }
}

#[cfg(test)]
mod tests;
