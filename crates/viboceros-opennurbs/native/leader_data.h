// Native text leader payload shared with the Rust 3DM adapter.
constexpr uint32_t leader_payload_version = 1;
constexpr size_t max_leader_points = 100000;

bool append_leader(const ONX_Model& model, const ON_Leader& leader, BridgeObject& output) {
  const auto& plane = leader.Plane();
  if (!plane.IsValid() || leader.PointCount() < 2 || leader.PointCount() > max_leader_points)
    return false;
  output.coordinates = {
    plane.origin.x, plane.origin.y, plane.origin.z,
    plane.xaxis.x, plane.xaxis.y, plane.xaxis.z,
    plane.yaxis.x, plane.yaxis.y, plane.yaxis.z
  };
  ON_3dPoint previous = ON_3dPoint::UnsetPoint;
  for (unsigned i = 0; i < leader.PointCount(); ++i) {
    ON_3dPoint point;
    if (!leader.Point3d(static_cast<int>(i), point) || !point.IsValid()) return false;
    if (point == previous) continue;
    output.coordinates.insert(output.coordinates.end(), {point.x, point.y, point.z});
    previous = point;
  }
  if (output.coordinates.size() < 15) return false;
  const auto reference = model.DimensionStyleFromId(leader.DimensionStyleId());
  const auto* parent = ON_DimStyle::Cast(reference.ModelComponent());
  const auto& style = leader.DimensionStyle(parent ? *parent : ON_DimStyle::Default);
  std::vector<uint8_t> archive;
  if (!vibo_annotation::archive_style(style, archive)) return false;
  ByteWriter writer(output.geometry_data);
  writer.U32(leader_payload_version);
  writer.U32(static_cast<uint32_t>(archive.size()));
  if (!archive.empty()) writer.Bytes(archive.data(), archive.size());
  for (const auto& text : {utf8(leader.PlainText()), utf8(leader.RichText())}) {
    writer.Bytes(reinterpret_cast<const uint8_t*>(text.c_str()), text.size() + 1);
  }
  output.object_type = VIBO_OBJECT_LEADER;
  return true;
}

const ON_DimStyle* install_object_annotation_style(ONX_Model& model, ON_DimStyle& style) {
  const auto id = style.Id();
  if (id != ON_nil_uuid) {
    const auto& system_style = ON_DimStyle::SystemDimstyleFromId(id);
    if (system_style.Id() == id) {
      if (style.CompareFields(system_style)) return &system_style;
      // Installing a modified style with a system ID changes every annotation
      // that uses the corresponding built-in style.
      style.ClearId();
      style.ClearIndex();
    } else {
      const auto existing = model.ComponentFromId(ON_ModelComponent::Type::DimStyle, id);
      const auto* component = ON_DimStyle::Cast(existing.ModelComponent());
      if (component != nullptr && style.CompareFields(*component)) return component;
    }
  }
  // Effective overrides are stored as standalone styles so detached annotations do
  // not reference a dimension style absent from the destination model.
  style.SetParentId(ON_nil_uuid);
  style.ClearAllFieldOverrides();
  if (style.Name().IsEmpty()) style.SetName(L"Annotation style");
  const auto added = model.AddModelComponent(style, true);
  return ON_DimStyle::Cast(added.ModelComponent());
}

ON_Geometry* decode_leader(const ViboWriteObject& input, ONX_Model* model, std::string& error) {
  const size_t count = input.coordinate_count;
  if (count < 15 || count % 3 != 0 || (count - 9) / 3 > max_leader_points ||
      input.index_count != 0 || input.knot_u_count != 0 || input.knot_v_count != 0 ||
      input.geometry_data == nullptr) {
    error = "invalid leader path";
    return nullptr;
  }
  ByteReader reader(input.geometry_data, input.geometry_data_count);
  uint32_t version = 0, archive_count = 0;
  std::string text, rich_text;
  if (!reader.U32(version) || version != leader_payload_version || !reader.U32(archive_count)) {
    error = "invalid leader payload";
    return nullptr;
  }
  const auto* archive = reader.Take(archive_count);
  if (archive == nullptr || !reader.CString(text) || !reader.CString(rich_text) || !reader.Finished()) {
    error = "invalid leader annotation data";
    return nullptr;
  }
  const double* coords = input.coordinates;
  const ON_Plane plane(ON_3dPoint(coords), ON_3dVector(coords + 3), ON_3dVector(coords + 6));
  if (!plane.IsValid()) {
    error = "invalid leader plane";
    return nullptr;
  }
  std::vector<ON_3dPoint> points;
  points.reserve((count - 9) / 3);
  for (size_t i = 9; i < count; i += 3) points.emplace_back(coords + i);

  ON_DimStyle archived_style;
  const ON_DimStyle* style = &ON_DimStyle::Default;
  if (archive_count != 0) {
    if (!vibo_annotation::read(archive, archive_count, archived_style)) {
      error = "invalid leader annotation style";
      return nullptr;
    }
    style = model ? install_object_annotation_style(*model, archived_style) : &archived_style;
  }
  const ON_wString contents((rich_text.empty() ? text : rich_text).c_str());
  auto leader = std::make_unique<ON_Leader>();
  if (style == nullptr || !leader->Create(contents, style, static_cast<int>(points.size()),
                                         points.data(), plane, false, 0.0) ||
      (!contents.IsEmpty() && leader->Text() == nullptr)) {
    error = "OpenNURBS could not create a text leader";
    return nullptr;
  }
  return leader.release();
}
