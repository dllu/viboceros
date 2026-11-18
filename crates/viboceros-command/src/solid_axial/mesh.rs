//! Mesh output for the staged getters and the established explicit-axis scripts.
use super::*;
use crate::{
    AxialPrimitiveRadius, Command, CommandContext, axial_primitive_frame, parse_mesh_cone_options,
    parse_mesh_cylinder_options, parse_mesh_truncated_cone_options, remembered::Remembered,
};
use std::sync::Arc;
use viboceros_document::Document;
use viboceros_geometry::{
    GeometryError, MeshConeOptions, MeshCylinderOptions, MeshTruncatedConeOptions, Point3,
    TriangleMesh, UnitVector3,
};

impl Definition {
    pub fn mesh_face_count(self) -> Option<usize> {
        let shape = self.kind.shape();
        self.topology
            .face_count(shape.cap_count(self.solid, self.end_radius))
    }

    pub fn mesh(self, tolerance: Tolerance) -> Result<TriangleMesh, GeometryError> {
        match self.kind.shape() {
            Shape::Cone => TriangleMesh::try_cone_grid(
                self.apex_frame()?,
                self.radius,
                self.height.abs(),
                MeshConeOptions {
                    vertical_count: self.topology.vertical_count,
                    around_count: self.topology.around_count,
                    solid: self.solid,
                    cap_style: self.topology.cap_style,
                },
                tolerance,
            ),
            Shape::Cylinder => {
                let (frame, height) = self.cylinder_path()?;
                self.cylinder_mesh(frame, [0., height], tolerance)
            }
            Shape::TruncatedCone => TriangleMesh::try_truncated_cone_with_apex(
                self.frustum_frame()?,
                [self.radius, self.end_radius],
                self.height.abs(),
                self.frustum_options(),
                tolerance,
            ),
        }
    }

    fn cylinder_mesh(
        self,
        frame: Frame3,
        heights: [f64; 2],
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        let mesh = TriangleMesh::try_cylinder_grid(
            frame,
            self.radius,
            heights,
            MeshCylinderOptions {
                vertical_count: self.topology.vertical_count,
                around_count: self.topology.around_count,
                cap_bottom: self.solid,
                cap_top: self.solid,
                circumscribe: false,
                cap_style: self.topology.cap_style,
            },
            tolerance,
        )?;
        if self.solid {
            Ok(mesh.unified_face_orientations()?.0)
        } else {
            Ok(mesh)
        }
    }

    fn frustum_options(self) -> MeshTruncatedConeOptions {
        MeshTruncatedConeOptions {
            vertical_count: self.topology.vertical_count,
            around_count: self.topology.around_count,
            solid: self.solid,
            cap_style: self.topology.cap_style,
        }
    }

    fn legacy_mesh(self, tolerance: Tolerance) -> Result<TriangleMesh, GeometryError> {
        match self.kind.shape() {
            Shape::Cylinder => {
                let height = self.height;
                let heights = if self.both_sides {
                    [-height.abs(), height.abs()]
                } else {
                    [height.min(0.), height.max(0.)]
                };
                self.cylinder_mesh(self.frame, heights, tolerance)
            }
            Shape::TruncatedCone => TriangleMesh::try_truncated_cone_grid(
                self.frustum_frame()?,
                [self.radius, self.end_radius],
                self.height.abs(),
                self.frustum_options(),
                tolerance,
            ),
            Shape::Cone => self.mesh(tolerance),
        }
    }
}

pub(crate) struct MeshCommand {
    pub(crate) kind: Kind,
    pub(crate) preferences: Arc<Remembered<MeshSettings>>,
}

impl Command for MeshCommand {
    fn name(&self) -> &'static str {
        self.kind.name()
    }

    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }

    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let tolerance = doc.tolerance();
        let mut preferences = self.preferences.get().0;
        let (definition, mesh) = match legacy_arguments(self.kind, args) {
            Ok(options) => {
                let (frame, radius) =
                    axial_primitive_frame(options.center, options.radius, options.axis, tolerance)?;
                let definition = Definition {
                    kind: self.kind,
                    frame,
                    radius,
                    height: options.height,
                    solid: options.solid,
                    both_sides: options.both_sides,
                    end_radius: options.end_radius,
                    topology: options.topology,
                };
                (definition, definition.legacy_mesh(tolerance)?)
            }
            Err(error) => {
                let input = from_arguments(
                    doc,
                    args,
                    self.kind,
                    context.construction_plane,
                    preferences.for_mesh_start(),
                )
                .map_err(|staged_error| {
                    if matches!(
                        &error,
                        CommandError::Usage(_) | CommandError::InvalidNumber(_)
                    ) {
                        staged_error
                    } else {
                        error
                    }
                })?;
                preferences = input.preferences;
                let definition = input.definition.expect("complete mesh axial primitive");
                (definition, definition.mesh(tolerance)?)
            }
        };
        let id = doc.add_geometry(Geometry::Mesh(mesh))?;
        self.preferences.set(MeshSettings(preferences));
        Ok(format!(
            "Added {} mesh {} {id} ({} around × {} vertical faces)",
            if definition.solid { "closed" } else { "open" },
            self.kind.noun(),
            definition.topology.around_count,
            definition.topology.vertical_count
        ))
    }
}

struct LegacyArguments {
    center: Point3,
    axis: UnitVector3,
    radius: AxialPrimitiveRadius,
    height: f64,
    end_radius: f64,
    solid: bool,
    both_sides: bool,
    topology: MeshTopology,
}

fn legacy_arguments(kind: Kind, args: &[&str]) -> Result<LegacyArguments, CommandError> {
    match kind.shape() {
        Shape::Cone => {
            let opts = parse_mesh_cone_options(args)?;
            Ok(LegacyArguments {
                center: opts.center,
                axis: opts.axis,
                radius: opts.radius,
                height: opts.height,
                end_radius: 0.,
                solid: opts.solid,
                both_sides: false,
                topology: MeshTopology {
                    vertical_count: opts.vertical_count,
                    around_count: opts.around_count,
                    cap_style: opts.cap_style,
                },
            })
        }
        Shape::Cylinder => {
            let opts = parse_mesh_cylinder_options(args)?;
            Ok(LegacyArguments {
                center: opts.center,
                axis: opts.axis,
                radius: opts.radius,
                height: opts.height,
                end_radius: 0.,
                solid: opts.solid,
                both_sides: opts.both_sides,
                topology: MeshTopology {
                    vertical_count: opts.vertical_count,
                    around_count: opts.around_count,
                    cap_style: opts.cap_style,
                },
            })
        }
        Shape::TruncatedCone => {
            let opts = parse_mesh_truncated_cone_options(args)?;
            Ok(LegacyArguments {
                center: opts.center,
                axis: opts.axis,
                radius: opts.base_radius,
                height: opts.height,
                end_radius: opts.end_radius,
                solid: opts.solid,
                both_sides: false,
                topology: MeshTopology {
                    vertical_count: opts.vertical_count,
                    around_count: opts.around_count,
                    cap_style: opts.cap_style,
                },
            })
        }
    }
}
