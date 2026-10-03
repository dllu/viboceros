//! Prepared wire cages shared by all viewports; no model edits or morph fitting.
use super::display_cache::{DisplayCache, DisplayGeometry};
use super::object_preview::PreviewObject;
use super::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use viboceros_command::twist::{TwistOptions, reference_angle};
use viboceros_document::{GeometrySnapshot, GroupId};
use viboceros_geometry::{
    BoundingBox3, BrepWireCage, PointMorph, SurfacePreviewCage, TwistPointMorph,
};

enum Cage {
    Curve(NurbsCurve),
    Surface(Box<SurfacePreviewCage>),
    Brep(BrepWireCage),
    Direct,
}
struct Source {
    id: ObjectId,
    snapshot: GeometrySnapshot,
    density: i32,
    group: Option<GroupId>,
    display: Rc<DisplayGeometry>,
    cage: Option<Cage>,
}

#[derive(Clone, Copy, PartialEq)]
struct Key {
    start: Point3,
    end: Point3,
    angle: Real,
    rigid: bool,
    infinite: bool,
    preserve: bool,
}

pub(crate) struct TwistPreviewImage {
    pub(super) objects: BTreeMap<ObjectId, PreviewObject>,
}

#[derive(Default)]
pub(crate) struct TwistPreviewCache {
    sources: Vec<Source>,
    tolerance: Option<Tolerance>,
    centers: Option<Vec<Point3>>,
    key: Option<Key>,
    image: Option<Rc<TwistPreviewImage>>,
}

#[derive(Clone, Copy)]
pub(crate) struct TwistPreview<'a> {
    pub sources: &'a [ObjectId],
    pub start: Point3,
    pub end: Point3,
    pub reference: Point3,
    pub options: TwistOptions,
    pub last_angle: Option<Real>,
    pub cache: &'a RefCell<TwistPreviewCache>,
}

impl std::fmt::Debug for TwistPreview<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TwistPreview")
            .field("sources", &self.sources)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("reference", &self.reference)
            .field("options", &self.options)
            .field("last_angle", &self.last_angle)
            .finish_non_exhaustive()
    }
}

/// Choose the continuous turn nearest the last mouse angle. Typed scalar input
/// keeps its explicit angle; only the reference-point mouse path accumulates turns.
pub(crate) fn continuous_angle(principal: Real, previous: Real) -> Real {
    principal + ((previous - principal) / 360.).round() * 360.
}

#[cfg(test)]
mod tests;

impl TwistPreview<'_> {
    pub(super) fn resolve(
        self,
        cursor: Option<Point3>,
        document: &Document,
        display_cache: &RefCell<DisplayCache>,
    ) -> (Option<Rc<TwistPreviewImage>>, Option<Option<Real>>) {
        let update = cursor.and_then(|p| {
            reference_angle(
                self.start,
                self.end,
                self.reference,
                p,
                document.tolerance(),
            )
            .ok()
            .map(|a| continuous_angle(a, self.last_angle.unwrap_or(0.)))
        });
        let angle = update.or(self.last_angle);
        let mut cache = self.cache.borrow_mut();
        let Some(angle) = angle else {
            return (None, None);
        };
        let key = Key {
            start: self.start,
            end: self.end,
            angle,
            rigid: self.options.rigid,
            infinite: self.options.infinite,
            preserve: !self.options.rigid
                && self.options.preserve_structure
                && self.sources.iter().any(|id| {
                    document.object(*id).is_some_and(|o| {
                        matches!(o.geometry(), Geometry::NurbsSurface(_))
                            || matches!(o.geometry(), Geometry::Brep(b) if b.faces().len()==1)
                    })
                }),
        };
        let image = cache.get(self.sources, key, document, display_cache).ok();
        // Native GetAngle retains the last valid deformation on an axial pick.
        // Failed preparation also retains the previous display without changing the model.
        let image = image.or_else(|| cache.image.clone());
        (image, update.map(Some))
    }
}

impl TwistPreviewCache {
    fn get(
        &mut self,
        ids: &[ObjectId],
        key: Key,
        document: &Document,
        display_cache: &RefCell<DisplayCache>,
    ) -> Result<Rc<TwistPreviewImage>, viboceros_command::CommandError> {
        let valid = self.tolerance == Some(document.tolerance())
            && self.sources.len() == ids.len()
            && self.sources.iter().zip(ids).all(|(s, id)| {
                s.id == *id
                    && document.object(*id).is_some_and(|o| {
                        s.snapshot.shares_storage_with(o.geometry_snapshot())
                            && s.density == o.attributes().wire_density()
                            && s.group == o.top_group()
                    })
            });
        if !valid {
            let mut display = display_cache.borrow_mut();
            self.sources = ids
                .iter()
                .map(|id| {
                    let o = document
                        .object(*id)
                        .ok_or(viboceros_document::DocumentError::ObjectNotFound(*id))?;
                    Ok(Source {
                        id: *id,
                        snapshot: o.geometry_snapshot().clone(),
                        density: o.attributes().wire_density(),
                        group: o.top_group(),
                        display: display.get(o, document.tolerance()),
                        cage: None,
                    })
                })
                .collect::<Result<Vec<_>, viboceros_document::DocumentError>>()?;
            self.tolerance = Some(document.tolerance());
            self.centers = None;
            self.key = None;
            self.image = None;
        }
        if self.key == Some(key) {
            return Ok(self.image.as_ref().unwrap().clone());
        }
        let morph = TwistPointMorph::try_new(
            key.start,
            key.end,
            key.angle.to_radians(),
            key.infinite,
            document.tolerance(),
        )?;
        if key.rigid && self.centers.is_none() {
            let bounds = self
                .sources
                .iter()
                .map(|s| s.snapshot.tight_bounds(document.tolerance()))
                .collect::<Result<Vec<_>, _>>()?;
            let mut groups = BTreeMap::<GroupId, BoundingBox3>::new();
            for (source, bounds) in self.sources.iter().zip(&bounds) {
                if let Some(group) = source.group {
                    let merged = groups
                        .get(&group)
                        .map_or(Ok(*bounds), |b| b.union(*bounds))?;
                    groups.insert(group, merged);
                }
            }
            self.centers = Some(
                self.sources
                    .iter()
                    .zip(bounds)
                    .map(|(s, b)| {
                        s.group
                            .and_then(|g| groups.get(&g))
                            .copied()
                            .unwrap_or(b)
                            .center()
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        let mut objects = BTreeMap::new();
        let mut group_maps = BTreeMap::new();
        for (i, source) in self.sources.iter_mut().enumerate() {
            let object = if key.rigid {
                let transform = if let Some(group) = source.group {
                    if let Some(map) = group_maps.get(&group) {
                        *map
                    } else {
                        let map = morph.rigid_transform(self.centers.as_ref().unwrap()[i])?;
                        group_maps.insert(group, map);
                        map
                    }
                } else {
                    morph.rigid_transform(self.centers.as_ref().unwrap()[i])?
                };
                PreviewObject {
                    geometry: source.display.clone(),
                    transform: Some(transform),
                }
            } else {
                if source.cage.is_none() {
                    source.cage = Some(match &*source.snapshot {
                        Geometry::Brep(b) => Cage::Brep(BrepWireCage::try_new(
                            b,
                            source.density,
                            document.tolerance(),
                        )?),
                        Geometry::NurbsSurface(s) => {
                            Cage::Surface(Box::new(SurfacePreviewCage::try_new(s)?))
                        }
                        Geometry::Point(_) | Geometry::PointCloud(_) | Geometry::Mesh(_) => {
                            Cage::Direct
                        }
                        g => {
                            let c = g
                                .nurbs_curve_representation()?
                                .expect("curve geometry has a NURBS representation");
                            Cage::Curve(c.try_change_degree(c.degree().max(3), false)?)
                        }
                    });
                }
                let geometry = match source.cage.as_ref().unwrap() {
                    Cage::Curve(c) => Rc::new(DisplayGeometry::new(
                        Geometry::NurbsCurve(morph.morph_nurbs_curve_controls(c)?).into(),
                        source.density,
                        document.tolerance(),
                    )),
                    Cage::Surface(s) => Rc::new(DisplayGeometry::new(
                        Geometry::NurbsSurface(s.morphed_surface(&morph, key.preserve)?).into(),
                        source.density,
                        document.tolerance(),
                    )),
                    Cage::Brep(b) => {
                        let mut wires = Vec::new();
                        let preserve = key.preserve
                            && matches!(&*source.snapshot, Geometry::Brep(b) if b.faces().len() == 1);
                        for c in b.morphed_wires(&morph, preserve)? {
                            c.visit_segments(|a, b| wires.push([a, b]));
                        }
                        Rc::new(DisplayGeometry::with_wires(
                            source.snapshot.clone(),
                            source.density,
                            document.tolerance(),
                            wires,
                        ))
                    }
                    Cage::Direct => {
                        if matches!(&*source.snapshot, Geometry::Mesh(_)) {
                            let wires = source
                                .display
                                .wires()
                                .iter()
                                .map(|line| {
                                    Ok([morph.morph_point(line[0])?, morph.morph_point(line[1])?])
                                })
                                .collect::<Result<Vec<_>, viboceros_geometry::GeometryError>>()?;
                            Rc::new(DisplayGeometry::with_wires(
                                source.snapshot.clone(),
                                source.density,
                                document.tolerance(),
                                wires,
                            ))
                        } else {
                            Rc::new(DisplayGeometry::new(
                                source
                                    .snapshot
                                    .morphed(&morph, document.tolerance())?
                                    .into(),
                                source.density,
                                document.tolerance(),
                            ))
                        }
                    }
                };
                PreviewObject {
                    geometry,
                    transform: None,
                }
            };
            objects.insert(source.id, object);
        }
        let image = Rc::new(TwistPreviewImage { objects });
        self.key = Some(key);
        self.image = Some(image.clone());
        Ok(image)
    }
}
