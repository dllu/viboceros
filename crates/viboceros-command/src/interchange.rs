//! File import/export commands and document-to-format adapters.
use super::{Command, CommandError};
mod names;
mod paths;
use names::ImportNames;
#[cfg(test)]
mod tests;
use std::collections::{BTreeMap, BTreeSet};
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectColorSource};
use viboceros_geometry::{GeometryError, Tolerance, TriangleMesh};
use viboceros_io::{
    StlFormat, ThreeDmColorSource, ThreeDmGeometry, ThreeDmGroup, ThreeDmLayer, ThreeDmModel,
    ThreeDmObject, read_stl_file, write_3dm_file, write_stl_file,
};

pub(super) const SURFACE_EXPORT_SAMPLES_PER_SPAN: usize = 16;

pub(super) struct ImportStlCommand;

impl Command for ImportStlCommand {
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        paths::parse(input, false)
    }

    fn name(&self) -> &'static str {
        "ImportStl"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("ImportStl path"));
        }
        let path = arguments.join(" ");
        let mesh = read_stl_file(&path)?;
        let triangle_count = mesh.triangles().len();
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        Ok(format!(
            "Imported STL mesh {id} ({triangle_count} triangles) from '{path}'"
        ))
    }
}

pub(super) struct ExportStlCommand;

impl Command for ExportStlCommand {
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        paths::parse(input, true)
    }

    fn name(&self) -> &'static str {
        "ExportStl"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("ExportStl [Ascii|Binary] path"));
        }
        let (format, path_arguments) = if arguments[0].eq_ignore_ascii_case("ascii") {
            (StlFormat::Ascii, &arguments[1..])
        } else if arguments[0].eq_ignore_ascii_case("binary") {
            (StlFormat::Binary, &arguments[1..])
        } else {
            (StlFormat::Binary, arguments)
        };
        if path_arguments.is_empty() {
            return Err(CommandError::Usage("ExportStl [Ascii|Binary] path"));
        }
        let path = path_arguments.join(" ");
        let mesh = combined_document_mesh(document)?;
        let triangle_count = mesh.triangles().len();
        write_stl_file(&path, &mesh, format)?;
        Ok(format!(
            "Exported {triangle_count} triangles as {format:?} STL to '{path}'"
        ))
    }
}

pub(super) struct ImportThreeDmCommand;

pub(super) struct ImportStepCommand;

impl Command for ImportStepCommand {
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        let input = input.trim();
        let end = input.find(char::is_whitespace).unwrap_or(input.len());
        if input[..end].eq_ignore_ascii_case("Native=Yes") {
            let mut arguments = paths::parse(&input[end..], false)?;
            if arguments.is_empty() {
                return Err(CommandError::Usage("ImportStep Native=Yes path"));
            }
            arguments.insert(0, "Native=Yes");
            return Ok(arguments);
        }
        paths::parse(input, false)
    }

    fn name(&self) -> &'static str {
        "ImportStep"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ImportStp"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("ImportStep path"));
        }
        if arguments.len() == 2 && arguments[0] == "Native=Yes" {
            return import_native_step(document, arguments[1]);
        }
        let path = arguments.join(" ");
        let import =
            viboceros_io::read_step_file_in_units(&path, document.units(), document.tolerance())?;
        let object_count = import.objects.len();
        let triangle_count = import
            .objects
            .iter()
            .map(|object| object.mesh.triangles().len())
            .sum::<usize>();
        let warning_count = import.report.warning_count();
        let layer_id = document.current_layer_id();
        for object in import.objects {
            let mut attributes = ObjectAttributes::on_layer(layer_id);
            if let Some(name) = object.name {
                attributes = attributes.with_name(name);
            }
            document.add_geometry_with_attributes(Geometry::Mesh(object.mesh), attributes)?;
        }
        Ok(format!(
            "Imported {object_count} STEP mesh object(s) ({triangle_count} triangles) from '{path}' ({warning_count} conversion warning(s))"
        ))
    }
}

fn import_native_step(document: &mut Document, path: &str) -> Result<String, CommandError> {
    let reader = std::fs::File::open(path).map_err(viboceros_io::StepError::from)?;
    let imported = viboceros_io::read_step_planar_instances_in_units(
        reader,
        document.units(),
        document.tolerance(),
    )?;
    let warnings = imported.report.warning_count();
    let mut occurrences = BTreeMap::<usize, (Option<String>, Vec<viboceros_geometry::Brep>)>::new();
    for instance in imported.instances {
        occurrences
            .entry(instance.placement_index)
            .or_insert_with(|| (instance.name, Vec::new()))
            .1
            .push(instance.brep);
    }
    // Preflight all combined occurrences before inserting the first object.
    let objects = occurrences
        .into_values()
        .map(|(name, shells)| {
            Ok((
                name,
                viboceros_geometry::Brep::try_combine(shells, document.tolerance())?,
            ))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let count = objects.len();
    let layer = document.current_layer_id();
    for (name, brep) in objects {
        let mut attributes = ObjectAttributes::on_layer(layer);
        if let Some(name) = name {
            attributes = attributes.with_name(name);
        }
        document.add_geometry_with_attributes(Geometry::Brep(brep), attributes)?;
    }
    Ok(format!(
        "Imported {count} native planar STEP object(s) from '{path}' ({warnings} conversion warning(s))"
    ))
}

pub(super) struct ExportStepCommand;

impl Command for ExportStepCommand {
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        paths::parse(input, false)
    }

    fn name(&self) -> &'static str {
        "ExportStep"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ExportStp"]
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("ExportStep path"));
        }
        let path = arguments.join(" ");
        let mesh = combined_document_mesh(document)?;
        let triangle_count = mesh.triangles().len();
        viboceros_io::write_step_file_in_units(
            &path,
            std::slice::from_ref(&mesh),
            document.units(),
            document.tolerance(),
        )?;
        Ok(format!(
            "Exported {triangle_count} triangles as a STEP faceted shell to '{path}'"
        ))
    }
}

impl Command for ImportThreeDmCommand {
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        paths::parse(input, false)
    }

    fn name(&self) -> &'static str {
        "Import3dm"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("Import3dm path"));
        }
        let path = arguments.join(" ");
        let model =
            viboceros_io::read_3dm_file_in_units(&path, document.units(), document.tolerance())?;
        let unsupported = model.unsupported_object_count();
        let layer_count = model.layers.len();
        let object_count = model.objects.len();

        let mut imported_layers = Vec::with_capacity(layer_count);
        let mut layer_names = ImportNames::new(
            document.layers().map(|layer| layer.name()),
            true,
            "Imported Layer",
        );
        for layer in &model.layers {
            let name = layer_names.allocate(&layer.name);
            let id = document.add_layer(
                name,
                ColorRgb::new(layer.color[0], layer.color[1], layer.color[2]),
            )?;
            imported_layers.push(id);
        }

        let mut imported_objects = Vec::with_capacity(object_count);
        for object in model.objects {
            let layer_id = imported_layers[object.layer_index];
            let mut attributes = ObjectAttributes::on_layer(layer_id)
                .with_object_color(ColorRgb::new(
                    object.object_color[0],
                    object.object_color[1],
                    object.object_color[2],
                ))
                .with_color_source(document_color_source_from_3dm(object.color_source))
                .with_visibility(object.visible)
                .with_locked(object.locked)
                .try_with_wire_density(object.wire_density)?;
            if let Some(name) = object.name {
                attributes = attributes.with_name(name);
            }
            let id = document.add_geometry_with_attributes(
                document_geometry_from_3dm(object.geometry),
                attributes,
            )?;
            imported_objects.push((id, object.group_indices));
        }

        let mut imported_groups = Vec::with_capacity(model.groups.len());
        let mut group_names = ImportNames::new(
            document.groups().filter_map(|group| group.name()),
            false,
            "Imported Group",
        );
        for group in &model.groups {
            let name = group_names.allocate(&group.name);
            imported_groups.push(document.add_empty_group(Some(name))?);
        }
        for (id, memberships) in imported_objects {
            document.set_object_group_memberships(
                id,
                memberships.iter().map(|index| imported_groups[*index]),
            )?;
        }
        let imported_group_count = imported_groups.len();

        for (source, id) in model.layers.iter().zip(imported_layers) {
            document.set_layer_visibility(id, source.visible)?;
            document.set_layer_locked(id, source.locked)?;
        }

        Ok(format!(
            "Imported {object_count} objects in {imported_group_count} groups on {layer_count} layers from '{path}' ({unsupported} unsupported objects skipped)"
        ))
    }
}

pub(super) struct ExportThreeDmCommand;

impl Command for ExportThreeDmCommand {
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        paths::parse(input, false)
    }

    fn name(&self) -> &'static str {
        "Export3dm"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("Export3dm path"));
        }
        let path = arguments.join(" ");
        let model = document_3dm_model(document)?;
        let group_count = model.groups.len();
        let layer_count = model.layers.len();
        let report = write_3dm_file(&path, &model)?;
        let object_count = report.written_object_count;
        let mut message = format!(
            "Exported {object_count} objects in {group_count} groups on {layer_count} layers to '{path}'"
        );
        if report.adapted_curve_count != 0 {
            message.push_str(&format!(
                "; adapted {} curve objects from {} source objects",
                report.adapted_curve_count, report.source_object_count
            ));
        }
        Ok(message)
    }
}

pub(super) fn document_3dm_model(document: &Document) -> Result<ThreeDmModel, CommandError> {
    let layers = document
        .layers()
        .map(|layer| {
            let color = layer.color();
            ThreeDmLayer {
                name: layer.name().to_owned(),
                color: [color.red, color.green, color.blue],
                visible: layer.is_visible(),
                locked: layer.is_locked(),
            }
        })
        .collect();
    let layer_indices: BTreeMap<_, _> = document
        .layers()
        .enumerate()
        .map(|(index, layer)| (layer.id(), index))
        .collect();
    let used_group_names = document
        .groups()
        .filter_map(|group| group.name().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let document_groups = document.groups().collect::<Vec<_>>();
    let mut group_name_numbers = 1_u64..=u64::MAX;
    let groups = document_groups
        .iter()
        .map(|group| ThreeDmGroup {
            name: group.name().map_or_else(
                || next_serialized_group_name(&used_group_names, &mut group_name_numbers),
                str::to_owned,
            ),
        })
        .collect::<Vec<_>>();
    let group_indices = document_groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.id(), index))
        .collect::<BTreeMap<_, _>>();
    let objects = document
        .objects()
        .map(|object| {
            Ok(ThreeDmObject {
                geometry: geometry_to_3dm(object.geometry())?,
                layer_index: layer_indices[&object.attributes().layer_id()],
                name: object.attributes().name().map(str::to_owned),
                visible: object.attributes().is_visible(),
                locked: object.attributes().is_locked(),
                object_color: {
                    let color = object.attributes().object_color();
                    [color.red, color.green, color.blue]
                },
                color_source: three_dm_color_source_from_document(
                    object.attributes().color_source(),
                ),
                wire_density: object.attributes().wire_density(),
                group_indices: object
                    .group_ids()
                    .iter()
                    .map(|id| group_indices[id])
                    .collect(),
            })
        })
        .collect::<Result<_, CommandError>>()?;
    let mut model = ThreeDmModel::new(layers, groups, objects);
    model.units = document.units().clone();
    model.tolerance = document.tolerance();
    Ok(model)
}

fn next_serialized_group_name(
    used: &BTreeSet<String>,
    numbers: &mut impl Iterator<Item = u64>,
) -> String {
    // One monotonic sequence per export: earlier candidates never need to be
    // revisited or added to the set of reserved document names.
    for number in numbers.by_ref() {
        let candidate = format!("Group{number:02}");
        if !used.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!("the finite document cannot contain every numbered group name")
}

const fn document_color_source_from_3dm(source: ThreeDmColorSource) -> ObjectColorSource {
    match source {
        ThreeDmColorSource::Layer => ObjectColorSource::Layer,
        ThreeDmColorSource::Object => ObjectColorSource::Object,
        ThreeDmColorSource::Material => ObjectColorSource::Material,
        ThreeDmColorSource::Parent => ObjectColorSource::Parent,
    }
}

const fn three_dm_color_source_from_document(source: ObjectColorSource) -> ThreeDmColorSource {
    match source {
        ObjectColorSource::Layer => ThreeDmColorSource::Layer,
        ObjectColorSource::Object => ThreeDmColorSource::Object,
        ObjectColorSource::Material => ThreeDmColorSource::Material,
        ObjectColorSource::Parent => ThreeDmColorSource::Parent,
    }
}

pub(super) fn geometry_to_3dm(geometry: &Geometry) -> Result<ThreeDmGeometry, CommandError> {
    Ok(match geometry {
        Geometry::Point(point) => ThreeDmGeometry::Point(*point),
        Geometry::PointCloud(cloud) => ThreeDmGeometry::PointCloud(cloud.clone()),
        Geometry::Line(line) => ThreeDmGeometry::Line(*line),
        Geometry::Circle(circle) => ThreeDmGeometry::NurbsCurve(circle.to_nurbs()?),
        Geometry::Arc(arc) => ThreeDmGeometry::Arc(*arc),
        Geometry::Ellipse(ellipse) => ThreeDmGeometry::NurbsCurve(ellipse.to_nurbs()?),
        Geometry::Polyline(polyline) => ThreeDmGeometry::Polyline(polyline.clone()),
        Geometry::NurbsCurve(curve) => ThreeDmGeometry::NurbsCurve(curve.clone()),
        Geometry::PolyCurve(curve) => ThreeDmGeometry::PolyCurve(curve.clone()),
        Geometry::NurbsSurface(surface) => ThreeDmGeometry::NurbsSurface(surface.clone()),
        Geometry::Brep(brep) => ThreeDmGeometry::Brep(brep.clone()),
        Geometry::Mesh(mesh) => ThreeDmGeometry::Mesh(mesh.clone()),
    })
}

pub(super) fn document_geometry_from_3dm(geometry: ThreeDmGeometry) -> Geometry {
    match geometry {
        ThreeDmGeometry::Point(point) => Geometry::Point(point),
        ThreeDmGeometry::PointCloud(cloud) => Geometry::PointCloud(cloud),
        ThreeDmGeometry::Line(line) => Geometry::Line(line),
        ThreeDmGeometry::Arc(arc) => Geometry::Arc(arc),
        ThreeDmGeometry::NurbsCurve(curve) => Geometry::NurbsCurve(curve),
        ThreeDmGeometry::Polyline(curve) => Geometry::Polyline(curve),
        ThreeDmGeometry::PolyCurve(curve) => Geometry::PolyCurve(curve),
        ThreeDmGeometry::NurbsSurface(surface) => Geometry::NurbsSurface(surface),
        ThreeDmGeometry::Brep(brep) => Geometry::Brep(brep),
        ThreeDmGeometry::Mesh(mesh) => Geometry::Mesh(mesh),
    }
}

pub(super) fn combined_document_mesh(document: &Document) -> Result<TriangleMesh, CommandError> {
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for object in document.objects() {
        if !object.attributes().is_visible()
            || !document
                .layer(object.attributes().layer_id())
                .is_some_and(|layer| layer.is_visible())
        {
            continue;
        }
        let tessellation;
        let mesh = match object.geometry() {
            Geometry::Mesh(mesh) => mesh,
            Geometry::NurbsSurface(surface) => {
                tessellation =
                    surface.tessellate(SURFACE_EXPORT_SAMPLES_PER_SPAN, document.tolerance())?;
                &tessellation
            }
            Geometry::Brep(brep) => {
                tessellation =
                    brep.tessellate(SURFACE_EXPORT_SAMPLES_PER_SPAN, document.tolerance())?;
                &tessellation
            }
            _ => continue,
        };
        let offset =
            u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
        let combined_vertex_count = vertices
            .len()
            .checked_add(mesh.vertices().len())
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if combined_vertex_count > u32::MAX as usize {
            return Err(GeometryError::TooManyMeshVertices.into());
        }
        vertices.extend_from_slice(mesh.vertices());
        for triangle in mesh.triangles() {
            triangles.push([
                triangle[0]
                    .checked_add(offset)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
                triangle[1]
                    .checked_add(offset)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
                triangle[2]
                    .checked_add(offset)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
            ]);
        }
    }
    if triangles.is_empty() {
        return Err(CommandError::NoMeshToExport);
    }
    Ok(TriangleMesh::try_new(
        vertices,
        triangles,
        Tolerance::MESH_VALIDATION,
    )?)
}
