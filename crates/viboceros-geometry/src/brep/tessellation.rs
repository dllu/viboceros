//! Per-face meshing, boundary provenance audits, and conforming reconstruction.
mod planar_mesh;

use super::*;

mod audit;
mod conforming;
mod creases;
mod curvature;
mod independent;
mod normals;
mod sampling;
use sampling::{
    SPARSE_MESH_VERTEX_LIMIT, SPARSE_POLYGON_VERTEX_LIMIT, Sampling,
    append_trimmed_surface_grid_parameters, trim_chart_samples,
};

#[cfg(test)]
mod tests;
