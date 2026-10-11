//! Geometry assembly delegates to the native stitcher; ownership stays in Rust.
use super::*;
use viboceros_geometry::{BrepJoinComponent, BrepJoinReport, BrepSolidOrientation};
use viboceros_smlib::Shape;

fn assemble(
    sources: &[&Brep],
    distance: Real,
    tolerance: Tolerance,
) -> Result<BrepJoinReport, CommandError> {
    if !distance.is_finite() || distance <= 0. {
        return Err(GeometryError::InvalidBrepTopology {
            context: "B-rep join distance",
        }
        .into());
    }
    if sources.len() > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit.into());
    }
    let face_sources = sources
        .iter()
        .enumerate()
        .flat_map(|(i, b)| std::iter::repeat_n(i, b.faces().len()))
        .collect::<Vec<_>>();
    let combined = Brep::try_combine(sources.iter().map(|b| (*b).clone()).collect(), tolerance)?;
    let bounds = combined.bounds();
    let low = bounds.min();
    let high = bounds.max();
    let extent = low
        .to_array()
        .into_iter()
        .zip(high.to_array())
        .map(|(a, b)| b - a)
        .fold(0_f64, f64::max);
    let normalize = extent > 0. && extent.is_finite() && !(1e-3..=1e6).contains(&extent);
    let (prepared, local_tolerance, local_distance, inverse) = if normalize {
        let origin = Point3::try_new(0., 0., 0.)?;
        let move_to_origin = AffineTransform3::from_translation(low.vector_to(origin)?);
        let forward =
            move_to_origin.then(AffineTransform3::try_uniform_scale(origin, 1. / extent)?)?;
        let inverse = AffineTransform3::try_uniform_scale(origin, extent)?
            .then(AffineTransform3::from_translation(origin.vector_to(low)?))?;
        let local = Tolerance::try_new(
            tolerance.absolute() / extent,
            tolerance.relative(),
            tolerance.angular(),
        )?;
        (
            combined.transformed(forward, local)?,
            local,
            distance / extent,
            Some(inverse),
        )
    } else {
        (combined, tolerance, distance, None)
    };
    let shape = if normalize {
        Shape::from_sheet_with_normalized_domains(&prepared, local_tolerance)?
    } else {
        Shape::from_brep(&prepared, local_tolerance)?
    };
    let stitched = shape.stitch(local_distance)?;
    let output = stitched.shape.to_brep(local_tolerance)?;
    let output = if let Some(inverse) = inverse {
        output.transformed(inverse, tolerance)?
    } else {
        output
    };
    if output.faces().len() != stitched.face_origins.len() {
        return Err(viboceros_smlib::Error::InvalidOutput.into());
    }
    let owners = stitched
        .face_origins
        .iter()
        .map(|&i| {
            face_sources
                .get(i)
                .copied()
                .ok_or(viboceros_smlib::Error::InvalidOutput)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut edges = vec![BTreeSet::new(); output.edges().len()];
    for (f, face) in output.faces().iter().enumerate() {
        for boundary in face.loops() {
            for trim in boundary.trims() {
                if let Some(edge) = trim.edge() {
                    edges[edge].insert(owners[f]);
                }
            }
        }
    }
    let mut pairs = BTreeSet::new();
    for owners in &edges {
        if owners.len() > 2 {
            return Err(viboceros_smlib::Error::Kernel(
                "Native join produced nonmanifold cross-source edges".into(),
            )
            .into());
        }
        if owners.len() == 2 {
            pairs.insert([*owners.first().unwrap(), *owners.last().unwrap()]);
        }
    }
    let mut components = Vec::new();
    for faces in output.edge_connected_face_components() {
        let source_indices = faces
            .iter()
            .map(|&f| owners[f])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut used = BTreeSet::new();
        for &f in &faces {
            for boundary in output.faces()[f].loops() {
                for trim in boundary.trims() {
                    if let Some(edge) = trim.edge() {
                        used.insert(edge);
                    }
                }
            }
        }
        let joined_edge_count = used.iter().filter(|&&edge| edges[edge].len() > 1).count();
        let mut brep = output.sub_brep(&faces, tolerance)?;
        // Native normal unification applies to the entire assembly. If an
        // unrelated open sheet remains, a newly closed component may retain
        // its inward source sense. Reuse the existing exact spatial witness
        // rather than relying on a mass integral at extreme scales.
        if joined_edge_count > 0 && brep.solid_orientation()? == BrepSolidOrientation::Inward {
            brep.reverse_orientation();
        }
        components.push(BrepJoinComponent {
            brep,
            source_indices,
            joined_edge_count,
        });
    }
    Ok(BrepJoinReport {
        components,
        candidate_source_pairs: pairs.into_iter().collect(),
    })
}

pub(super) fn join_breps(
    sources: &[&Brep],
    distance: Real,
    tolerance: Tolerance,
) -> Result<Vec<BrepJoinComponent>, CommandError> {
    Ok(assemble(sources, distance, tolerance)?.components)
}

pub(super) fn join_breps_with_report(
    sources: &[&Brep],
    distance: Real,
    tolerance: Tolerance,
) -> Result<BrepJoinReport, CommandError> {
    let mut result = assemble(sources, distance, tolerance)?;
    if sources.len() == 2 {
        return Ok(result);
    }
    // Two-source kernel probes retain boundary contacts before batch ambiguity
    // resolution, for the command-first policy's original-pick participation.
    let mut pairs = BTreeSet::new();
    for a in 0..sources.len() {
        for b in a + 1..sources.len() {
            if assemble(&[sources[a], sources[b]], distance, tolerance)?
                .components
                .iter()
                .any(|c| c.joined_edge_count > 0)
            {
                pairs.insert([a, b]);
            }
        }
    }
    result.candidate_source_pairs = pairs.into_iter().collect();
    Ok(result)
}
