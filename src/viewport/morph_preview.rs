//! Prepared morph cages and source instances shared across deformation commands.
use super::display_cache::{DisplayCache, DisplayGeometry};
use super::object_preview::PreviewObject;
use super::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::rc::Rc;
use viboceros_document::{ControlPointId, GeometrySnapshot, GripMode, GroupId};
use viboceros_geometry::{
    AffineTransform3, BoundingBox3, BrepWireCage, PointMorph, SurfacePreviewCage,
};

pub(crate) trait MorphPreviewKey: Clone + PartialEq {
    type Morph: PointMorph;
    fn morph(&self, tolerance: Tolerance)
    -> Result<Self::Morph, viboceros_geometry::GeometryError>;
    fn rigid(&self) -> bool;
    fn preserve(&self) -> bool;
    fn rigid_groups(&self) -> bool {
        true
    }
    fn rigid_transform(
        &self,
        morph: &Self::Morph,
        center: Point3,
    ) -> Result<AffineTransform3, viboceros_geometry::GeometryError>;
    fn guide(
        &self,
        _morph: &Self::Morph,
    ) -> Result<Vec<Point3>, viboceros_geometry::GeometryError> {
        Ok(Vec::new())
    }
    fn omit_shaded_isocurves(&self) -> bool {
        false
    }
}

pub(super) enum Cage {
    Curve(NurbsCurve),
    Surface(Box<SurfacePreviewCage>),
    Brep(BrepWireCage),
    Direct,
}
pub(super) struct Source {
    id: ObjectId,
    snapshot: GeometrySnapshot,
    density: i32,
    group: Option<GroupId>,
    display: Rc<DisplayGeometry>,
    pub(super) cage: Option<Cage>,
    boundary_cage: Option<BrepWireCage>,
    boundary_display: Option<Rc<DisplayGeometry>>,
    grips: Option<(GripMode, BTreeSet<usize>)>,
}

pub(crate) struct MorphPreviewImage {
    pub(super) objects: BTreeMap<ObjectId, PreviewObject>,
    pub(super) guide: Vec<Point3>,
    pub(super) grips: BTreeMap<ControlPointId, Point3>,
    shaded_objects: Option<BTreeMap<ObjectId, PreviewObject>>,
}

impl MorphPreviewImage {
    pub(super) fn objects_for_mode(&self, mode: DisplayMode) -> &BTreeMap<ObjectId, PreviewObject> {
        if mode != DisplayMode::Wireframe {
            self.shaded_objects.as_ref().unwrap_or(&self.objects)
        } else {
            &self.objects
        }
    }
}

pub(crate) struct MorphPreviewCache<K> {
    pub(super) sources: Vec<Source>,
    object_ids: Vec<ObjectId>,
    grips: Vec<ControlPointId>,
    tolerance: Option<Tolerance>,
    centers: Option<Vec<Point3>>,
    key: Option<K>,
    pub(super) image: Option<Rc<MorphPreviewImage>>,
}

impl<K> Default for MorphPreviewCache<K> {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            object_ids: Vec::new(),
            grips: Vec::new(),
            tolerance: None,
            centers: None,
            key: None,
            image: None,
        }
    }
}

impl<K: MorphPreviewKey> MorphPreviewCache<K> {
    pub(super) fn get(
        &mut self,
        ids: &[ObjectId],
        key: K,
        document: &Document,
        display_cache: &RefCell<DisplayCache>,
    ) -> Result<Rc<MorphPreviewImage>, viboceros_command::CommandError> {
        self.get_with_grips(ids, &[], key, document, display_cache)
    }

    pub(super) fn get_with_grips(
        &mut self,
        ids: &[ObjectId],
        grips: &[ControlPointId],
        key: K,
        document: &Document,
        display_cache: &RefCell<DisplayCache>,
    ) -> Result<Rc<MorphPreviewImage>, viboceros_command::CommandError> {
        let valid = self.tolerance == Some(document.tolerance())
            && self.object_ids == ids
            && self.grips == grips
            && self.sources.iter().all(|s| {
                document.object(s.id).is_some_and(|o| {
                    s.snapshot.shares_storage_with(o.geometry_snapshot())
                        && s.density == o.attributes().wire_density()
                        && s.group == o.top_group()
                        && s.grips
                            .as_ref()
                            .is_none_or(|(mode, _)| document.grip_mode(s.id) == Some(*mode))
                })
            });
        if !valid {
            let mut display = display_cache.borrow_mut();
            let mut picks = BTreeMap::<ObjectId, (GripMode, BTreeSet<usize>)>::new();
            for grip in grips {
                let mode = document
                    .grip_mode(grip.object)
                    .filter(|_| {
                        document
                            .control_point_locations(grip.object)
                            .is_some_and(|p| grip.index < p.len())
                    })
                    .ok_or(
                        viboceros_document::DocumentError::InvalidControlPointSelection {
                            object: grip.object,
                            index: grip.index,
                        },
                    )?;
                picks
                    .entry(grip.object)
                    .or_insert_with(|| (mode, BTreeSet::new()))
                    .1
                    .insert(grip.index);
            }
            let mut owners = ids.to_vec();
            owners.extend(picks.keys().copied().filter(|id| !ids.contains(id)));
            self.object_ids = ids.to_vec();
            self.grips = grips.to_vec();
            self.sources = owners
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
                        boundary_cage: None,
                        boundary_display: None,
                        grips: picks.get(id).cloned(),
                    })
                })
                .collect::<Result<Vec<_>, viboceros_document::DocumentError>>()?;
            self.tolerance = Some(document.tolerance());
            self.centers = None;
            self.key = None;
            self.image = None;
        }
        if self.key.as_ref() == Some(&key) {
            return Ok(self.image.as_ref().unwrap().clone());
        }
        let morph = key.morph(document.tolerance())?;
        if key.rigid() && self.centers.is_none() {
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
        let mut posed_grips = BTreeMap::new();
        let mut shaded_objects = key.omit_shaded_isocurves().then(BTreeMap::new);
        let mut group_maps = BTreeMap::new();
        for (i, source) in self.sources.iter_mut().enumerate() {
            let object = if let Some((mode, picks)) = &source.grips {
                let geometry = source.snapshot.with_morphed_grips_in_mode(
                    picks,
                    &morph,
                    document.tolerance(),
                    *mode,
                )?;
                let points = geometry.grip_locations(*mode)?;
                for &index in picks {
                    if let Some(&point) = points.get(index) {
                        posed_grips.insert(
                            ControlPointId {
                                object: source.id,
                                index,
                            },
                            point,
                        );
                    }
                }
                PreviewObject {
                    geometry: Rc::new(DisplayGeometry::new(
                        geometry.into(),
                        source.density,
                        document.tolerance(),
                    )),
                    transform: None,
                }
            } else if key.rigid() {
                let transform = if let Some(group) = source.group.filter(|_| key.rigid_groups()) {
                    if let Some(map) = group_maps.get(&group) {
                        *map
                    } else {
                        let map = key.rigid_transform(&morph, self.centers.as_ref().unwrap()[i])?;
                        group_maps.insert(group, map);
                        map
                    }
                } else {
                    let center = if key.rigid_groups() {
                        self.centers.as_ref().unwrap()[i]
                    } else {
                        source
                            .snapshot
                            .tight_bounds(document.tolerance())?
                            .center()?
                    };
                    key.rigid_transform(&morph, center)?
                };
                PreviewObject {
                    geometry: source.display.clone(),
                    transform: Some(transform),
                }
            } else {
                if source.cage.is_none() {
                    source.cage = Some(match &*source.snapshot {
                        Geometry::Brep(b) => Cage::Brep(BrepWireCage::try_new_with_operations(
                            b,
                            source.density,
                            document.tolerance(),
                            viboceros_command::morph_preview_operations(),
                        )?),
                        Geometry::NurbsSurface(s) => {
                            Cage::Surface(Box::new(SurfacePreviewCage::try_new_with_operations(
                                s,
                                viboceros_command::morph_preview_operations(),
                            )?))
                        }
                        Geometry::Point(_) | Geometry::PointCloud(_) | Geometry::Mesh(_) => {
                            Cage::Direct
                        }
                        g => {
                            let c = g
                                .nurbs_curve_representation()?
                                .expect("curve geometry has a NURBS representation");
                            Cage::Curve(viboceros_command::prepare_morph_preview_curve(&c)?)
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
                        Geometry::NurbsSurface(s.morphed_surface(&morph, key.preserve())?).into(),
                        source.density,
                        document.tolerance(),
                    )),
                    Cage::Brep(b) => {
                        let mut wires = Vec::new();
                        let preserve = key.preserve()
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
            if let Some(shaded) = &mut shaded_objects {
                let geometry = if source.grips.is_some() {
                    let curves = match &*object.geometry.geometry {
                        Geometry::NurbsSurface(s) => surface_boundaries(s)?,
                        Geometry::Brep(b) => b.edges().iter().map(|e| e.curve().clone()).collect(),
                        _ => Vec::new(),
                    };
                    if curves.is_empty() {
                        object.geometry.clone()
                    } else {
                        display_with_boundaries(
                            &object,
                            source.density,
                            document.tolerance(),
                            curves,
                        )
                    }
                } else if key.rigid() {
                    if source.boundary_display.is_none() {
                        let curves = match &*source.snapshot {
                            Geometry::Brep(b) => {
                                b.edges().iter().map(|e| e.curve().clone()).collect()
                            }
                            Geometry::NurbsSurface(s) => surface_boundaries(s)?,
                            _ => Vec::new(),
                        };
                        source.boundary_display = Some(if curves.is_empty() {
                            source.display.clone()
                        } else {
                            display_with_boundaries(
                                &object,
                                source.density,
                                document.tolerance(),
                                curves,
                            )
                        });
                    }
                    source.boundary_display.as_ref().unwrap().clone()
                } else {
                    let curves = match (&*source.snapshot, &*object.geometry.geometry) {
                        (Geometry::Brep(b), _) => {
                            if source.boundary_cage.is_none() {
                                source.boundary_cage = Some(BrepWireCage::try_new_with_operations(
                                    b,
                                    -1,
                                    document.tolerance(),
                                    viboceros_command::morph_preview_operations(),
                                )?);
                            }
                            source
                                .boundary_cage
                                .as_ref()
                                .unwrap()
                                .morphed_wires(&morph, key.preserve() && b.faces().len() == 1)?
                        }
                        (_, Geometry::NurbsSurface(s)) => surface_boundaries(s)?,
                        _ => Vec::new(),
                    };
                    if curves.is_empty() {
                        object.geometry.clone()
                    } else {
                        display_with_boundaries(
                            &object,
                            source.density,
                            document.tolerance(),
                            curves,
                        )
                    }
                };
                shaded.insert(
                    source.id,
                    PreviewObject {
                        geometry,
                        transform: object.transform,
                    },
                );
            }
            objects.insert(source.id, object);
        }
        let image = Rc::new(MorphPreviewImage {
            objects,
            guide: key.guide(&morph).unwrap_or_default(),
            grips: posed_grips,
            shaded_objects,
        });
        self.key = Some(key);
        self.image = Some(image.clone());
        Ok(image)
    }
}

fn surface_boundaries(
    s: &viboceros_geometry::NurbsSurface,
) -> Result<Vec<NurbsCurve>, viboceros_geometry::GeometryError> {
    if let Some(operations) = viboceros_command::morph_preview_operations() {
        use viboceros_geometry::SurfaceKnotDirection::{U, V};
        [
            (U, *s.domain_v().start()),
            (U, *s.domain_v().end()),
            (V, *s.domain_u().start()),
            (V, *s.domain_u().end()),
        ]
        .into_iter()
        .map(|(direction, t)| (operations.isocurve)(s, direction, t))
        .collect()
    } else {
        Ok(vec![
            s.isocurve_u(*s.domain_v().start())?,
            s.isocurve_u(*s.domain_v().end())?,
            s.isocurve_v(*s.domain_u().start())?,
            s.isocurve_v(*s.domain_u().end())?,
        ])
    }
}

fn display_with_boundaries(
    object: &PreviewObject,
    density: i32,
    tolerance: Tolerance,
    curves: Vec<NurbsCurve>,
) -> Rc<DisplayGeometry> {
    let mut wires = Vec::new();
    for c in curves {
        c.visit_segments(|a, b| wires.push([a, b]));
    }
    Rc::new(DisplayGeometry::with_wires(
        object.geometry.geometry.clone(),
        density,
        tolerance,
        wires,
    ))
}
