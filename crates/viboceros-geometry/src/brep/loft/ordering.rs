//! Plane ordering with a midpoint-chain fallback for open sections.
use super::*;
use std::collections::VecDeque;

pub(super) fn sort_profiles(
    curves: &mut [NurbsCurve],
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if curves.len() < 2 {
        return Ok(false);
    }
    let closed = curves
        .iter()
        .map(NurbsCurve::is_closed)
        .collect::<Result<Vec<_>, _>>()?;
    let all_closed = closed.iter().all(|c| *c);
    if !all_closed && closed.iter().any(|c| *c) {
        return Ok(false);
    }
    if let Some(oriented) = sort_parallel_planes(curves, all_closed, tolerance)? {
        return Ok(oriented);
    }
    if !all_closed {
        sort_midpoint_chain(curves)?;
    }
    Ok(false)
}

fn sort_parallel_planes(
    curves: &mut [NurbsCurve],
    all_closed: bool,
    tolerance: Tolerance,
) -> Result<Option<bool>, GeometryError> {
    let mut normals = Vec::with_capacity(curves.len());
    for curve in curves.iter() {
        if !curve.is_planar(tolerance)? || (!all_closed && curve.is_linear(tolerance)?) {
            return Ok(None);
        }
        let normal = if all_closed {
            profiles::area_normal(curve)?
        } else {
            crate::CurveRef::NurbsCurve(curve)
                .supporting_plane_normal(tolerance)?
                .map(|n| n.as_vector())
        };
        let Some(normal) = normal else {
            return Ok(None);
        };
        normals.push(normal);
    }
    let axis = normals[0];
    for &normal in &normals {
        if axis.cross(normal)?.length()? > tolerance.angular().max(1e-12) {
            return Ok(None);
        }
    }
    let origin = curves[0].control_points()[0].point();
    let mut order = curves
        .iter()
        .enumerate()
        .map(|(i, c)| {
            origin
                .vector_to(c.control_points()[0].point())?
                .dot(axis)
                .map(|offset| (offset, i))
        })
        .collect::<Result<Vec<_>, _>>()?;
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Coincident open-profile planes do not define a section order. Native
    // uses the spatial chain for the whole set, including the other planes.
    if !all_closed
        && order
            .windows(2)
            .any(|pair| (pair[1].0 - pair[0].0).abs() <= tolerance.absolute())
    {
        return Ok(None);
    }
    let reference = normals[order[0].1];
    let sorted = order
        .into_iter()
        .map(|(_, i)| curves[i].clone())
        .collect::<Vec<_>>();
    curves.clone_from_slice(&sorted);
    // Open section ordering retains V; closed sections also orient the chart.
    Ok(Some(all_closed && reference.dot(axis)? > 0.))
}

fn sort_midpoint_chain(curves: &mut [NurbsCurve]) -> Result<(), GeometryError> {
    let centers = curves
        .iter()
        .map(|c| {
            c.evaluate(*c.domain().start())?
                .midpoint(c.evaluate(*c.domain().end())?)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut order = VecDeque::from([0]);
    let mut remaining = (1..curves.len()).collect::<Vec<_>>();
    while !remaining.is_empty() {
        let ends = [order[0], order[order.len() - 1]];
        let mut best = (f64::INFINITY, 0, 0);
        for (slot, &candidate) in remaining.iter().enumerate() {
            for (side, &end) in ends.iter().enumerate() {
                let distance = centers[end].distance_to(centers[candidate])?;
                if distance < best.0 {
                    best = (distance, slot, side);
                }
            }
        }
        let next = remaining.remove(best.1);
        if order.len() == 1 || best.2 == 1 {
            order.push_back(next);
        } else {
            order.push_front(next);
        }
    }
    let sorted = order
        .into_iter()
        .map(|i| curves[i].clone())
        .collect::<Vec<_>>();
    curves.clone_from_slice(&sorted);
    Ok(())
}
