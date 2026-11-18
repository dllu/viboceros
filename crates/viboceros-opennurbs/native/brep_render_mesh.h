// Saved per-face render polygons keep their source-face identity after combining.
struct BridgeRenderMesh {
  std::vector<double> vertices;
  std::vector<double> normals;
  std::vector<uint32_t> faces;
  std::vector<uint32_t> face_sources;
};

std::unique_ptr<BridgeRenderMesh> read_brep_render_mesh(const ON_Brep& brep) {
  auto result = std::make_unique<BridgeRenderMesh>();
  bool has_normals = true;
  for (int i = 0; i < brep.m_F.Count(); ++i) {
    const auto* mesh = brep.m_F[i].Mesh(ON::render_mesh);
    if (!mesh || mesh->VertexCount() <= 0 || mesh->FaceCount() <= 0)
      return nullptr;
    const size_t offset = result->vertices.size() / 3;
    if (offset + mesh->VertexCount() > std::numeric_limits<uint32_t>::max())
      return nullptr;
    has_normals = has_normals && mesh->HasVertexNormals();
    for (int v = 0; v < mesh->VertexCount(); ++v) {
      const auto point = mesh->Vertex(v);
      if (!point.IsValid()) return nullptr;
      result->vertices.insert(result->vertices.end(), {point.x, point.y, point.z});
      if (has_normals) {
        const auto normal = mesh->m_N[v];
        if (normal.IsValid()) {
          result->normals.insert(result->normals.end(), {normal.x, normal.y, normal.z});
        } else {
          has_normals = false;
        }
      }
    }
    for (int f = 0; f < mesh->FaceCount(); ++f) {
      for (const auto v : mesh->m_F[f].vi) {
        if (v < 0 || v >= mesh->VertexCount()) return nullptr;
        result->faces.push_back(static_cast<uint32_t>(offset + v));
      }
      result->face_sources.push_back(static_cast<uint32_t>(i));
    }
  }
  if (!has_normals) result->normals.clear();
  return result;
}
