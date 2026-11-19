//! Classify closed-region nesting and generate the requested offset steps.
use super::*;

pub(super) fn solve(
    sources: &[Curve3],
    options: MultipleOptions,
    normal: UnitVector3,
    tolerance: Tolerance,
    by_step: bool,
) -> Result<Vec<(usize, Curve3)>, CommandError> {
    let selected_count = sources.len();
    if selected_count == 0 {
        return Err(CommandError::NoObjectsSelected);
    }
    if selected_count
        .checked_mul(options.count)
        .is_none_or(|total| total > MAX_OFFSET_OUTPUTS)
    {
        return Err(CommandError::Usage(MULTIPLE_USAGE));
    }
    let inward = sources
        .iter()
        .map(|curve| {
            curve
                .offset_region_inward_sign_with_operations(normal, tolerance, offset_operations())
                .map_err(map_offset_error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let closed = inward
        .iter()
        .enumerate()
        .filter_map(|(i, sign)| sign.map(|_| i))
        .collect::<Vec<_>>();
    // Region nesting has an unambiguous depth only when selected boundaries
    // do not cross or touch. Check this before creating any document objects.
    let mut nurbs: Vec<Option<NurbsCurve>> = vec![None; closed.len()];
    for i in 0..closed.len() {
        for j in i + 1..closed.len() {
            let first = &sources[closed[i]];
            let second = &sources[closed[j]];
            let intersects = if let Some(intersects) =
                first.offset_region_boundary_relation(second, tolerance)?
            {
                intersects
            } else {
                if nurbs[i].is_none() {
                    nurbs[i] = Some(first.as_ref().to_nurbs().map_err(map_offset_error)?);
                }
                if nurbs[j].is_none() {
                    nurbs[j] = Some(second.as_ref().to_nurbs().map_err(map_offset_error)?);
                }
                !crate::curve_intersections::events(
                    nurbs[i].as_ref().expect("cached first curve"),
                    nurbs[j].as_ref().expect("cached second curve"),
                    tolerance,
                )?
                .is_empty()
            };
            if intersects {
                return Err(GeometryError::IntersectingOffsetRegions.into());
            }
        }
    }
    let mut containing = 0;
    for &i in &closed {
        containing += usize::from(
            sources[i]
                .offset_region_contains_with_operations(
                    options.side,
                    normal,
                    tolerance,
                    offset_operations(),
                )?
                .unwrap_or(false),
        );
    }
    let inside = containing % 2 != 0;
    let mut depths = vec![0_usize; sources.len()];
    for &i in &closed {
        let witness = sources[i]
            .offset_region_boundary_point()?
            .expect("closed region witness");
        for &j in &closed {
            if i != j
                && sources[j]
                    .offset_region_contains_with_operations(
                        witness,
                        normal,
                        tolerance,
                        offset_operations(),
                    )?
                    .unwrap_or(false)
            {
                depths[i] += 1;
            }
        }
    }
    let signs = sources
        .iter()
        .enumerate()
        .map(|(i, curve)| {
            if let Some(inward_sign) = inward[i] {
                Ok(inward_sign
                    * if inside { 1.0 } else { -1.0 }
                    * if depths[i] % 2 == 0 { 1.0 } else { -1.0 })
            } else {
                curve
                    .offset_side(options.side, normal, tolerance)
                    .map_err(map_offset_error)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut outputs = Vec::with_capacity(selected_count * options.count);
    let mut emit = |index, step| -> Result<(), CommandError> {
        let curve: &Curve3 = &sources[index];
        let distance = signs[index] * options.distance * step as Real;
        let parts = if options.loose {
            staged::loose_offset(curve, distance, normal, tolerance, false)?
        } else {
            curve
                .try_offset_with_corner_and_trim(
                    distance,
                    normal,
                    tolerance,
                    options.corner,
                    true,
                    offset_operations(),
                )
                .map_err(map_offset_error)?
        };
        if outputs.len().saturating_add(parts.len()) > MAX_OFFSET_OUTPUTS {
            return Err(CommandError::Usage(MULTIPLE_USAGE));
        }
        outputs.extend(parts.into_iter().map(|curve| (index, curve)));
        Ok(())
    };
    if by_step {
        for step in 1..=options.count {
            for index in 0..sources.len() {
                emit(index, step)?;
            }
        }
    } else {
        for index in 0..sources.len() {
            for step in 1..=options.count {
                emit(index, step)?;
            }
        }
    }
    Ok(outputs)
}

#[derive(Clone, Copy)]
pub(super) struct MultipleOptions {
    pub(super) distance: Real,
    pub(super) side: Point3,
    pub(super) count: usize,
    pub(super) loose: bool,
    pub(super) corner: RibbonCornerStyle,
    pub(super) input_layer: bool,
}
