//! Collect coplanar boundary ownership without duplicating shared source edges.
use super::*;

const MAX_INPUT_CURVES: usize = 8192;

#[derive(Clone, Copy)]
pub(super) struct Source {
    pub edge: usize,
    pub reversed: bool,
    pub trim_type: BrepTrimType,
    pub sides: [Option<usize>; 2],
}

pub(super) struct Neighbor {
    pub index: usize,
    pub flip: bool,
}

pub(super) struct Input {
    pub curves: Vec<NurbsCurve>,
    pub sources: Vec<Option<Source>>,
    pub neighbors: Vec<Neighbor>,
    pub cap: std::ops::Range<usize>,
}

pub(super) fn collect(
    brep: &Brep,
    face: &BrepFace,
    boundary: &BrepLoop,
    cap: &[NurbsCurve],
    connectors: &[NurbsCurve],
    tolerance: Tolerance,
    work: &mut Work,
) -> Result<Input, GeometryError> {
    let mut input = Input {
        curves: Vec::new(),
        sources: Vec::new(),
        neighbors: Vec::new(),
        cap: 0..0,
    };
    let mut lookup = BTreeMap::new();
    for trim in &boundary.trims {
        add_source(&mut input, &mut lookup, brep, trim, work.face, false, work)?;
    }
    input.cap = input.curves.len()..input.curves.len() + cap.len();
    input.curves.extend_from_slice(cap);
    input.sources.resize(input.curves.len(), None);
    input.curves.extend_from_slice(connectors);
    input.sources.resize(input.curves.len(), None);
    let [origin, east, _, north] = face.surface.try_affine_patch_corners(tolerance)?;
    let normal = origin
        .vector_to(east)?
        .cross(origin.vector_to(north)?)?
        .normalized_nonzero()?;
    let mut queue = VecDeque::new();
    for &edge in lookup.keys() {
        queue.extend(work.source_edges.incidence()[edge].iter().copied());
    }
    let mut visited = BTreeSet::from([work.face]);
    while let Some(index) = queue.pop_front() {
        if !visited.insert(index) {
            continue;
        }
        let neighbor = &brep.faces[index];
        let coplanar = neighbor.surface.control_points().iter().all(|p| {
            let scale = p
                .point()
                .to_array()
                .into_iter()
                .chain(origin.to_array())
                .map(Real::abs)
                .fold(1., Real::max);
            normal
                .as_vector()
                .dot_point_difference(p.point(), origin)
                .abs()
                <= tolerance.absolute().max(tolerance.relative() * scale)
        });
        if !coplanar {
            continue;
        }
        if work.selected.contains(&work.source_edges.owner(index)) {
            return Err(invalid(
                "inset networks between adjacent selected coplanar faces are unsupported",
            ));
        }
        let u = neighbor.surface.domain_u();
        let v = neighbor.surface.domain_v();
        let n = neighbor
            .surface
            .normal_at(u.start().midpoint(*u.end()), v.start().midpoint(*v.end()))?;
        let flip = n.as_vector().dot(normal.as_vector())? < 0.;
        input.neighbors.push(Neighbor { index, flip });
        for trim in neighbor.loops.iter().flat_map(|l| &l.trims) {
            add_source(&mut input, &mut lookup, brep, trim, index, flip, work)?;
            queue.extend(
                work.source_edges.incidence()[trim.edge.unwrap()]
                    .iter()
                    .copied(),
            );
        }
    }
    if input.curves.len() > MAX_INPUT_CURVES {
        return Err(invalid("inset network exceeds its curve-input limit"));
    }
    Ok(input)
}

#[allow(clippy::too_many_arguments)]
fn add_source(
    input: &mut Input,
    lookup: &mut BTreeMap<usize, usize>,
    brep: &Brep,
    trim: &BrepTrim,
    face: usize,
    flip: bool,
    work: &mut Work,
) -> Result<(), GeometryError> {
    let edge = trim
        .edge
        .ok_or_else(|| invalid("inset networks do not support singular boundary trims"))?;
    let index = if let Some(&index) = lookup.get(&edge) {
        index
    } else {
        if input.curves.len() >= MAX_INPUT_CURVES {
            return Err(invalid("inset network exceeds its curve-input limit"));
        }
        work.charge_curve(&brep.edges[edge].curve)?;
        let index = input.curves.len();
        input.curves.push(oriented(brep, trim)?);
        input.sources.push(Some(Source {
            edge,
            reversed: trim.reversed_3d,
            trim_type: trim.trim_type,
            sides: [None; 2],
        }));
        lookup.insert(edge, index);
        index
    };
    let source = input.sources[index].as_mut().unwrap();
    let side = usize::from((source.reversed != trim.reversed_3d) ^ flip);
    if source.sides[side].is_some_and(|previous| previous != face) {
        return Err(invalid(
            "coplanar inset source faces overlap on the same boundary side",
        ));
    }
    source.sides[side] = Some(face);
    Ok(())
}

pub(super) fn padded_surface(
    surface: &NurbsSurface,
    curves: &[NurbsCurve],
    tolerance: Tolerance,
) -> Result<(NurbsSurface, [[Real; 2]; 2]), GeometryError> {
    let corners = surface.try_affine_patch_corners(tolerance)?;
    let [origin, east, _, north] = corners;
    let u = surface.domain_u();
    let v = surface.domain_v();
    let du = u.end() - u.start();
    let dv = v.end() - v.start();
    let mut bounds = [[*u.start(), *u.end()], [*v.start(), *v.end()]];
    let projection =
        NurbsSurface::try_bilinear(corners)?.try_reparameterized(u.clone(), v.clone())?;
    for curve in curves {
        let uv = projection.try_pullback_affine_curve(curve, tolerance)?;
        for p in uv.control_points() {
            for (i, x) in p.point().to_array().into_iter().enumerate() {
                bounds[i][0] = bounds[i][0].min(x);
                bounds[i][1] = bounds[i][1].max(x);
            }
        }
    }
    for axis in &mut bounds {
        let pad = (axis[1] - axis[0]) * 0.25;
        axis[0] -= pad;
        axis[1] += pad;
    }
    let eu = origin.vector_to(east)?;
    let nv = origin.vector_to(north)?;
    let point = |u: Real, v: Real| {
        origin
            .translated(eu.scaled((u - surface.domain_u().start()) / du)?)?
            .translated(nv.scaled((v - surface.domain_v().start()) / dv)?)
    };
    let plane = NurbsSurface::try_bilinear([
        point(bounds[0][0], bounds[1][0])?,
        point(bounds[0][1], bounds[1][0])?,
        point(bounds[0][1], bounds[1][1])?,
        point(bounds[0][0], bounds[1][1])?,
    ])?
    .try_reparameterized(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?;
    Ok((plane, bounds))
}
