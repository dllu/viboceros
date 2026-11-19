//! Trim a closed tapered stock to the selected face.
use super::*;
use crate::extrude_surface::{
    Distance, SurfaceFaces, apply_staged_products, capture_source_face, face_extrusion_direction,
    local_sweep_source, surface_extrusion_targets,
};
use crate::extrude_tapered::{TaperCorners, TaperedExtrusionOptions, produce_curve_shape};
use crate::surface_boundary_cut::{
    BoundaryDifference, axial_extent, axial_point, copied_carrier, hole_crosses_sweep, on_plane,
};
use crate::surface_reference::BoundaryRef;
use viboceros_smlib::Shape;

mod oblique;
mod open_profile;
mod parallel_cut;
mod source_holes;

const REJECTED: CommandError = CommandError::SurfaceExtrusionBoundaryRejected;

pub(super) enum Inputs<'a> {
    Faces(&'a SurfaceFaces),
    Curves,
}

enum Profile {
    Closed {
        face: Brep,
        curve: Option<NurbsCurve>,
    },
    OpenCurve(NurbsCurve),
}

impl Profile {
    fn bounds(&self) -> Result<BoundingBox3, GeometryError> {
        match self {
            Self::Closed { face, .. } => face.padded_trim_bounds(),
            Self::OpenCurve(curve) => Ok(curve.control_point_bounds()),
        }
    }
}

struct Source {
    object: ObjectId,
    face: Option<usize>,
    profile: Profile,
}

pub(super) fn apply(
    doc: &mut Document,
    inputs: Inputs<'_>,
    options: TaperedExtrusionOptions,
    reference: BoundaryRef,
    cplane: Frame3,
) -> Result<usize, CommandError> {
    let cutter = reference.capture(doc)?;
    let tolerance = doc.tolerance();
    let selection = doc
        .selected_object_ids()
        .chain([reference.object])
        .collect::<Vec<_>>();
    let sources = match capture_sources(doc, inputs) {
        Ok(sources) => sources,
        Err(CommandError::SurfaceExtrusionBoundaryRejected) => {
            doc.set_transient_selection(selection)?;
            return Err(REJECTED);
        }
        Err(error) => return Err(error),
    };
    let bounds = sources
        .iter()
        .try_fold(cutter.padded_trim_bounds()?, |bounds, source| {
            bounds.union(source.profile.bounds()?)
        })?;
    let length = bounds.min().distance_to(bounds.max())?;
    let mut products = Vec::with_capacity(sources.len());
    let mut fallback = false;
    let mut remove_faces = Vec::new();
    let mut remove_curves = Vec::new();
    for source in sources {
        let product = match source.profile {
            Profile::Closed { face, curve } => extrude(
                &face,
                curve.as_ref(),
                &cutter,
                length,
                options,
                cplane,
                tolerance,
            ),
            Profile::OpenCurve(curve) => {
                open_profile::extrude(&curve, &cutter, length, options, cplane, tolerance)
            }
        };
        match product {
            Ok(product) => {
                products.push(Geometry::Brep(product.brep));
                fallback |= !product.reached_boundary;
                if options.delete_input {
                    match source.face {
                        Some(face) => remove_faces.push((source.object, face)),
                        None => remove_curves.push(source.object),
                    }
                }
            }
            Err(CommandError::SurfaceExtrusionBoundaryRejected) => {
                doc.set_transient_selection(selection)?;
                return Err(REJECTED);
            }
            Err(error) => return Err(error),
        }
    }
    let count = products.len();
    apply_staged_products(
        doc,
        products,
        remove_faces.into_iter().filter(|_| !fallback),
    )?;
    for curve in remove_curves.into_iter().filter(|_| !fallback) {
        doc.delete_object(curve)?;
    }
    let retained = selection
        .into_iter()
        .filter(|&id| doc.object(id).is_some())
        .collect::<Vec<_>>();
    doc.release_command_selection_on_history_replay(
        retained
            .iter()
            .copied()
            .filter(|&id| !fallback || id == reference.object),
    )?;
    doc.select_command_results(retained)?;
    if fallback { Err(REJECTED) } else { Ok(count) }
}

fn capture_sources(doc: &Document, inputs: Inputs<'_>) -> Result<Vec<Source>, CommandError> {
    match inputs {
        Inputs::Faces(faces) => surface_extrusion_targets(doc, faces)?
            .into_iter()
            .map(|(object, face)| {
                Ok(Source {
                    object,
                    face: Some(face),
                    profile: Profile::Closed {
                        face: capture_source_face(doc, object, face)?,
                        curve: None,
                    },
                })
            })
            .collect(),
        Inputs::Curves => {
            let tolerance = doc.tolerance();
            let mut sources = Vec::new();
            for object in doc.selected_objects() {
                let Some(curve) = object.geometry().nurbs_curve_representation()? else {
                    continue;
                };
                if !curve.is_planar(tolerance)? {
                    return Err(REJECTED);
                }
                let profile = if curve.is_closed()? {
                    Profile::Closed {
                        face: Brep::try_planar_face(&curve, tolerance)?,
                        curve: Some(curve),
                    }
                } else {
                    Profile::OpenCurve(curve)
                };
                sources.push(Source {
                    object: object.id(),
                    face: None,
                    profile,
                });
            }
            if sources.is_empty() {
                return Err(CommandError::NoExtrudableCurves);
            }
            Ok(sources)
        }
    }
}

struct Product {
    brep: Brep,
    reached_boundary: bool,
}

fn extrude(
    source: &Brep,
    curve: Option<&NurbsCurve>,
    cutter: &Brep,
    length: f64,
    options: TaperedExtrusionOptions,
    cplane: Frame3,
    tolerance: Tolerance,
) -> Result<Product, CommandError> {
    let (origin, original) = local_sweep_source(source, tolerance)?;
    let zero = Point3::try_new(0., 0., 0.)?;
    let to_local = AffineTransform3::from_translation(origin.vector_to(zero)?);
    let cutter = cutter.transformed(to_local, tolerance)?;
    let stock = Shape::from_brep(&original, tolerance)?;
    let reference = face_extrusion_direction(&stock, None, cplane, tolerance)?;
    let (axis, draft) =
        crate::extrude_tapered::displacement_and_draft(Distance::Numeric(1.), options, || {
            Ok(reference)
        })?;
    let direction = toward_cutter(original.padded_trim_bounds()?, &cutter, axis, tolerance)?;
    let displacement = direction.scaled(length)?;
    let to_world = AffineTransform3::from_translation(zero.vector_to(origin)?);
    let local_curve = curve.map(|curve| curve.transformed(to_local)).transpose()?;
    let profile = local_curve
        .as_ref()
        .map(viboceros_smlib::KernelCurve::from_nurbs_for_modeling)
        .transpose()?;
    if let (Some(local), Some(profile)) = (&local_curve, &profile)
        && let Ok(Some(brep)) = parallel_cut::produce(
            local,
            profile,
            &cutter,
            displacement,
            draft,
            options,
            tolerance,
        )
    {
        return Ok(Product {
            brep: brep.transformed(to_world, tolerance)?,
            reached_boundary: true,
        });
    }
    let oblique = draft != 0.
        && direction.cross(reference)?.length()? > tolerance.angular()
        && stock.single_face_plane_normal(tolerance)?.is_some();
    if oblique
        && draft > 0.
        && oblique::crosses_circular_focal_radius(
            &original,
            direction,
            reference,
            length * draft.to_radians().tan(),
            tolerance,
        )?
    {
        return Err(REJECTED);
    }
    let filled = if draft < 0. && !oblique {
        source_holes::fill_collapsed(&original, length * (-draft).to_radians().tan(), tolerance)?
    } else {
        None
    };
    let (source, stock) = if let Some((filled, _)) = &filled {
        (filled, Shape::from_brep(filled, tolerance)?)
    } else {
        (&original, stock)
    };
    let swept = if let (Some(local), Some(profile)) = (&local_curve, &profile) {
        produce_curve_shape(
            local,
            profile,
            displacement,
            draft,
            options,
            true,
            tolerance,
        )
    } else {
        stock.tapered_face(
            displacement,
            draft,
            options.corners.kernel(),
            true,
            tolerance,
        )
    };
    let swept = match swept {
        Ok(stock) => stock,
        // A contracting stock can collapse before reaching its temporary end.
        Err(viboceros_smlib::Error::Kernel(_)) => return Err(REJECTED),
        Err(error) => return Err(error.into()),
    };
    let stock = swept.to_brep(tolerance)?;
    if oblique && !stock.is_solid() {
        let output = match (curve.is_some(), options.solid) {
            (false, _) => oblique::Output::Surface,
            (true, true) => oblique::Output::SolidCurve,
            (true, false) => oblique::Output::CurveWalls,
        };
        let mut result = oblique::trim(
            &swept,
            stock,
            &original,
            &cutter,
            displacement,
            output,
            tolerance,
        )?;
        result.brep = result.brep.transformed(to_world, tolerance)?;
        return Ok(result);
    }
    let base_face = source.faces()[0].surface();
    let end = temporary_end(&stock, direction, tolerance)?.clone();
    let coincident = cutter.faces()[0]
        .surface()
        .plane(tolerance)?
        .is_some_and(|plane| on_plane(base_face, plane, tolerance));
    let offset = direction.scaled(2. * length)?;
    let (result, reached_boundary) = if coincident {
        if BoundaryDifference::subtract(&stock, &cutter, offset, tolerance)?.is_some() {
            return Err(REJECTED);
        }
        (stock, true)
    } else {
        clip_stock(stock, source, &end, &cutter, offset, tolerance)?
    };
    let merge_walls = draft < 0.
        && options.corners == TaperCorners::Sharp
        && base_face.plane(tolerance)?.is_some();
    let result = if merge_walls && crate::coplanar_faces::within_native_merge_budget(&result) {
        match Shape::from_brep(&result, tolerance)?.merge_coplanar_faces(tolerance)? {
            Some((merged, _)) => merged
                .to_brep(tolerance)?
                .try_merge_all_edges(tolerance.angular(), tolerance)?,
            None => result,
        }
    } else {
        result
    };
    let result = if curve.is_none() || options.solid {
        result
    } else {
        let cap = if reached_boundary && !coincident {
            cutter.faces()[0].surface()
        } else {
            &end
        };
        let walls = result
            .faces()
            .iter()
            .enumerate()
            .filter_map(|(index, face)| {
                let surface = face.surface();
                (!copied_carrier(surface, base_face, tolerance)
                    && !copied_carrier(surface, cap, tolerance))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        result.sub_brep(&walls, tolerance)?
    };
    let result = if let Some((_, holes)) = filled {
        source_holes::restore_base(result, &original, &holes, tolerance)?
    } else {
        result
    };
    Ok(Product {
        brep: result.transformed(to_world, tolerance)?,
        reached_boundary,
    })
}

fn toward_cutter(
    source: BoundingBox3,
    cutter: &Brep,
    axis: Vector3,
    tolerance: Tolerance,
) -> Result<Vector3, GeometryError> {
    let [base_min, base_max] = axial_extent(source, axis)?;
    let [cutter_min, cutter_max] = axial_extent(cutter.padded_trim_bounds()?, axis)?;
    if cutter_min.midpoint(cutter_max) < base_min.midpoint(base_max) - tolerance.absolute() {
        axis.scaled(-1.)
    } else {
        Ok(axis)
    }
}

fn temporary_end(
    stock: &Brep,
    direction: Vector3,
    tolerance: Tolerance,
) -> Result<&NurbsSurface, CommandError> {
    let far = stock
        .vertices()
        .iter()
        .map(|vertex| axial_point(vertex.point(), direction))
        .try_fold(f64::NEG_INFINITY, |far, point| {
            Ok::<_, GeometryError>(far.max(point?))
        })?;
    Ok(stock
        .faces()
        .iter()
        .find(|face| {
            face.surface().control_points().iter().all(|control| {
                axial_point(control.point(), direction)
                    .is_ok_and(|point| (point - far).abs() <= tolerance.absolute())
            })
        })
        .ok_or(REJECTED)?
        .surface())
}

fn clip_stock(
    stock: Brep,
    source: &Brep,
    end: &NurbsSurface,
    cutter: &Brep,
    offset: Vector3,
    tolerance: Tolerance,
) -> Result<(Brep, bool), CommandError> {
    if cutter.faces()[0].loops().len() > 1 {
        // Ensure the exterior covers the stock independently of the holes.
        let filled = cutter.try_remove_all_holes(tolerance)?.ok_or(REJECTED)?;
        clip(&stock, source, end, &filled, offset, tolerance, true)?;
        if hole_crosses_sweep(cutter, &stock, tolerance)? {
            return Ok((stock, false));
        }
        Ok((
            clip(&stock, source, end, cutter, offset, tolerance, false)?,
            true,
        ))
    } else {
        Ok((
            clip(&stock, source, end, cutter, offset, tolerance, true)?,
            true,
        ))
    }
}

fn clip(
    stock: &Brep,
    source: &Brep,
    end: &NurbsSurface,
    cutter: &Brep,
    offset: Vector3,
    tolerance: Tolerance,
    require_one_section: bool,
) -> Result<Brep, CommandError> {
    let difference =
        BoundaryDifference::subtract(stock, cutter, offset, tolerance)?.ok_or(REJECTED)?;
    if require_one_section && difference.result.edge_connected_face_components().len() != 1 {
        return Err(REJECTED);
    }
    difference.keep_source_section(source, end, cutter, tolerance)
}
