use super::*;
/// Read a selectable single-face surface, clipping a rectangular source face
/// to its active trim bounds while retaining its original parameter units.
pub fn source_surface(doc: &Document, id: ObjectId) -> Result<NurbsSurface, CommandError> {
    let object = doc
        .object(id)
        .filter(|_| doc.is_object_selectable(id))
        .ok_or(failure("select two visible, editable surfaces"))?;
    match object.geometry() {
        Geometry::NurbsSurface(s) => Ok(s.clone()),
        Geometry::Brep(b) if b.faces().len() == 1 => {
            let f = &b.faces()[0];
            let bounds=f.rectangular_trim_bounds(doc.tolerance())?.ok_or(failure("parent faces must have rectangular trims; arbitrary trimmed parents remain in progress"))?;
            #[cfg(feature = "native-smlib")]
            let clipped = viboceros_smlib::trim_surface(
                f.surface(),
                bounds[0][0]..=bounds[0][1],
                bounds[1][0]..=bounds[1][1],
            )?;
            #[cfg(not(feature = "native-smlib"))]
            let clipped = f
                .surface()
                .try_trimmed(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?;
            Ok(clipped)
        }
        _ => Err(failure(
            "select two single-face surfaces; use ExtractSrf for a polysurface face",
        )),
    }
}
pub fn prepare(
    doc: &Document,
    ids: [ObjectId; 2],
    options: &Options,
) -> Result<Prepared, CommandError> {
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = (doc, ids, options);
        Err(failure("requires the native-smlib kernel feature"))
    }
    #[cfg(feature = "native-smlib")]
    {
        use viboceros_smlib::{
            SurfaceFilletOptions, SurfaceFilletParentMode, fillet_surfaces,
            fillet_surfaces_with_parents,
        };
        if ids[0] == ids[1]
            || !options.radius.is_finite()
            || options.radius <= 0.
            || options.join && options.trim != Trim::Yes
        {
            return Err(CommandError::Usage(USAGE));
        }
        let picks = [
            options.picks[0].ok_or(failure(
                "Pick1=u,v and Pick2=u,v are required; pick locations determine the retained sides",
            ))?,
            options.picks[1].ok_or(failure(
                "Pick1=u,v and Pick2=u,v are required; pick locations determine the retained sides",
            ))?,
        ];
        let surfaces = [source_surface(doc, ids[0])?, source_surface(doc, ids[1])?];
        let tolerance = doc.tolerance();
        // The current curved extension producer has a known SDK extent mismatch.
        // Keep the option and its normal default, and decline that unqualified path.
        if options.extend {
            for surface in &surfaces {
                if surface.plane(tolerance)?.is_none() {
                    return Err(failure(
                        "curved Extend=Yes is not yet matched; use Extend=No",
                    ));
                }
            }
        }
        let native = SurfaceFilletOptions::try_from_picks(
            &surfaces[0],
            &surfaces[1],
            picks,
            options.radius,
            options.extend,
            tolerance,
        )?;
        let (fillet, mut parents) = if options.trim == Trim::No {
            (
                fillet_surfaces(&surfaces[0], &surfaces[1], native, tolerance)?
                    .ok_or(failure("no fillet found for these picks and radius"))?,
                [None, None],
            )
        } else {
            let r = fillet_surfaces_with_parents(
                &surfaces[0],
                &surfaces[1],
                native,
                if options.trim == Trim::Yes {
                    SurfaceFilletParentMode::Trim
                } else {
                    SurfaceFilletParentMode::Split
                },
                tolerance,
            )?
            .ok_or(failure("no fillet found for these picks and radius"))?;
            (r.fillet, r.parents)
        };
        if fillet.edge_connected_face_components().len() != 1 {
            return Err(failure(
                "disconnected fillet component selection remains in progress",
            ));
        }
        let sources = ids.map(|id| doc.object(id).unwrap().clone());
        for (i, parent) in parents.iter_mut().enumerate() {
            if let Geometry::Brep(b) = sources[i].geometry() {
                let support = b.faces()[0].surface();
                if support.domain_u() != surfaces[i].domain_u()
                    || support.domain_v() != surfaces[i].domain_v()
                {
                    for piece in parent.iter_mut().flatten() {
                        // Native construction uses the clipped support. Its restored
                        // p-curves already use source units; retain the original full
                        // support so Domain/UV queries do not inherit the clip bounds.
                        let faces = piece
                            .faces()
                            .iter()
                            .map(|face| restore_support(face, support))
                            .collect::<Result<Vec<_>, _>>()?;
                        *piece = Brep::try_new(
                            piece.vertices().to_vec(),
                            piece.edges().to_vec(),
                            faces,
                            tolerance,
                        )?;
                    }
                }
            }
            if let Geometry::Brep(b) = sources[i].geometry()
                && b.faces()[0].is_reversed()
            {
                for piece in parent.iter_mut().flatten() {
                    piece.reverse_orientation();
                }
            }
        }
        let replace = parents.each_ref().map(Option::is_some);
        let mut parts = vec![Output {
            brep: fillet,
            owner: None,
            selected: true,
        }];
        for (i, parent) in parents.into_iter().enumerate() {
            for brep in parent.into_iter().flatten() {
                parts.push(Output {
                    brep,
                    owner: Some(i),
                    selected: false,
                });
            }
        }
        let outputs = if options.join {
            crate::join::join_breps(
                &parts.iter().map(|p| &p.brep).collect::<Vec<_>>(),
                tolerance.absolute(),
                tolerance,
            )?
            .into_iter()
            .map(|c| Output {
                owner: c
                    .source_indices
                    .iter()
                    .filter_map(|&j| parts[j].owner)
                    .min(),
                selected: c.source_indices.iter().any(|&j| parts[j].selected),
                brep: c.brep,
            })
            .collect()
        } else {
            parts
        };
        Ok(Prepared {
            outputs,
            sources,
            replace,
            tolerance,
            layer: doc.current_layer_id(),
        })
    }
}

#[cfg(feature = "native-smlib")]
fn restore_support(
    face: &viboceros_geometry::BrepFace,
    support: &NurbsSurface,
) -> Result<viboceros_geometry::BrepFace, viboceros_geometry::GeometryError> {
    use viboceros_geometry::{BrepFace, BrepLoop, BrepTrim, SurfaceIso};
    let old = [face.surface().domain_u(), face.surface().domain_v()];
    let full = [support.domain_u(), support.domain_v()];
    let loops = face
        .loops()
        .iter()
        .map(|boundary| {
            let trims = boundary
                .trims()
                .iter()
                .map(|trim| {
                    // A boundary of the clipped rectangle may be an interior
                    // isocurve of the original support. Keep its exact p-curve.
                    let iso = match trim.iso() {
                        SurfaceIso::West if old[0].start() != full[0].start() => {
                            SurfaceIso::InteriorUConstant
                        }
                        SurfaceIso::East if old[0].end() != full[0].end() => {
                            SurfaceIso::InteriorUConstant
                        }
                        SurfaceIso::South if old[1].start() != full[1].start() => {
                            SurfaceIso::InteriorVConstant
                        }
                        SurfaceIso::North if old[1].end() != full[1].end() => {
                            SurfaceIso::InteriorVConstant
                        }
                        iso => iso,
                    };
                    BrepTrim::try_new(
                        trim.vertices(),
                        trim.edge(),
                        trim.is_reversed_3d(),
                        trim.curve().clone(),
                        trim.trim_type(),
                        iso,
                        trim.tolerance(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            BrepLoop::try_new(boundary.loop_type(), trims)
        })
        .collect::<Result<Vec<_>, _>>()?;
    BrepFace::try_new(support.clone(), face.is_reversed(), loops)
}
