// The Rust and OpenNURBS 3DM bridges share this versioned text dot payload.
constexpr uint32_t text_dot_payload_version = 1;
constexpr uint8_t text_dot_top = 1;
constexpr uint8_t text_dot_transparent = 2;
constexpr uint8_t text_dot_bold = 4;
constexpr uint8_t text_dot_italic = 8;

bool append_text_dot(const ON_TextDot& dot, BridgeObject& output) {
  if (!dot.IsValid()) {
    return false;
  }
  output.object_type = VIBO_OBJECT_TEXT_DOT;
  const auto& point = dot.CenterPoint();
  output.coordinates = {point.x, point.y, point.z};
  ByteWriter writer(output.geometry_data);
  writer.U32(text_dot_payload_version);
  writer.U32(static_cast<uint32_t>(dot.HeightInPoints()));
  writer.U8((dot.AlwaysOnTop() ? text_dot_top : 0) |
            (dot.Transparent() ? text_dot_transparent : 0) |
            (dot.Bold() ? text_dot_bold : 0) |
            (dot.Italic() ? text_dot_italic : 0));
  for (const auto* field : {dot.PrimaryText(), dot.SecondaryText(), dot.FontFace()}) {
    const auto text = utf8(ON_wString(field));
    writer.Bytes(reinterpret_cast<const uint8_t*>(text.c_str()), text.size() + 1);
  }
  return true;
}

ON_Geometry* decode_text_dot(const ViboWriteObject& input, std::string& error) {
  ByteReader reader(input.geometry_data, input.geometry_data_count);
  uint32_t version = 0, height = 0;
  uint8_t flags = 0;
  std::string primary, secondary, font;
  if (input.coordinate_count != 3 || input.index_count != 0 ||
      input.knot_u_count != 0 || input.knot_v_count != 0 ||
      input.geometry_data == nullptr ||
      !reader.U32(version) || version != text_dot_payload_version ||
      !reader.U32(height) || height < ON_TextDot::MinimumHeightInPoints ||
      height > static_cast<uint32_t>(std::numeric_limits<int>::max()) ||
      !reader.U8(flags) || (flags & 0xF0) != 0 ||
      !reader.CString(primary) || !reader.CString(secondary) ||
      !reader.CString(font) || !reader.Finished()) {
    error = "invalid text dot payload";
    return nullptr;
  }
  const ON_3dPoint point(input.coordinates[0], input.coordinates[1], input.coordinates[2]);
  if (!point.IsValid()) {
    error = "invalid text dot point";
    return nullptr;
  }
  const ON_wString text(primary.c_str()), subtitle(secondary.c_str()), face(font.c_str());
  auto dot = std::make_unique<ON_TextDot>(point, text, subtitle);
  dot->SetFontFace(face);
  dot->SetHeightInPoints(static_cast<int>(height));
  dot->SetAlwaysOnTop((flags & text_dot_top) != 0);
  dot->SetTransparent((flags & text_dot_transparent) != 0);
  dot->SetBold((flags & text_dot_bold) != 0);
  dot->SetItalic((flags & text_dot_italic) != 0);
  return dot.release();
}
