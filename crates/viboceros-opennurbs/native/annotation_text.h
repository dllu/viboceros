// Model text retains its ON_Text content and native, unrotated ink bounds.
constexpr uint32_t model_text_payload_version = 1;

// Some OpenNURBS builds can lay out lines but cannot resolve glyph metrics.
// Retain a reading area in model units so text remains visible and selectable.
double estimate_model_text_width(const ON_wString& plain, double height) {
  double line = 0.0, widest = 0.0;
  for (int i = 0; i < plain.Length(); ++i) {
    const wchar_t c = plain[i];
    if (c == L'\r' || c == L'\n') {
      widest = std::max(widest, line);
      line = 0.0;
    } else {
      line += c == L'\t' ? 4.0 : c == L' ' ? 0.5 : 1.0;
    }
  }
  return 0.85 * height * std::max(widest, line);
}

bool append_model_text(const ONX_Model& model, const ON_Text& annotation, BridgeObject& output) {
  const ON_Plane& plane = annotation.Plane();
  if (!plane.IsValid() || annotation.Text() == nullptr) return false;
  const auto reference = model.DimensionStyleFromId(annotation.DimensionStyleId());
  const auto* parent = ON_DimStyle::Cast(reference.ModelComponent());
  const auto& style = annotation.DimensionStyle(parent ? *parent : ON_DimStyle::Default);
  const double scale = style.DimScale();
  // Update text metrics for the effective style, including overrides.
  ON_Xform transform;
  annotation.GetTextXform(nullptr, &style, scale, transform);
  const auto* text = annotation.Text();
  const ON_wString plain_text = annotation.PlainText();
  const double width = text->TextIsWrapped() ? text->FormattingRectangleWidth() : 0.0;
  ON_2dPoint corners[4];
  double left = 0.0, right = 0.0, bottom = 0.0, top = 0.0;
  if (text->Get2dCorners(corners) && corners[0].IsValid() && corners[2].IsValid()) {
    left = corners[0].x * scale;
    bottom = corners[0].y * scale;
    right = corners[2].x * scale;
    top = corners[2].y * scale;
  }
  if (right <= left && !plain_text.IsEmpty()) {
    const double height = style.TextHeight() * scale;
    double estimated = estimate_model_text_width(plain_text, height);
    if (width > 0.0) {
      const double wrapped_width = width * scale;
      if (wrapped_width < estimated) {
        bottom -= (std::ceil(estimated / wrapped_width) - 1.0) * 1.2 * height;
        estimated = wrapped_width;
      }
    }
    if (top <= bottom) bottom = top - height;
    switch (style.TextHorizontalAlignment()) {
      case ON::TextHorizontalAlignment::Center: left = -0.5 * estimated; break;
      case ON::TextHorizontalAlignment::Right: left = -estimated; break;
      default: left = 0.0; break;
    }
    right = left + estimated;
  }
  output.coordinates = {
    plane.origin.x, plane.origin.y, plane.origin.z,
    plane.xaxis.x, plane.xaxis.y, plane.xaxis.z,
    plane.yaxis.x, plane.yaxis.y, plane.yaxis.z,
    text->TextRotationRadians(),
    width,
    1.0, left, bottom, right, top
  };
  std::vector<uint8_t> archive;
  if (!vibo_annotation::archive_style(style, archive)) return false;
  ByteWriter writer(output.geometry_data);
  writer.U32(model_text_payload_version);
  writer.U32(static_cast<uint32_t>(archive.size()));
  if (!archive.empty()) writer.Bytes(archive.data(), archive.size());
  for (const auto& value : {utf8(plain_text), utf8(annotation.RichText())}) {
    writer.Bytes(reinterpret_cast<const uint8_t*>(value.c_str()), value.size() + 1);
  }
  output.object_type = VIBO_OBJECT_TEXT;
  return true;
}

ON_Geometry* decode_model_text(const ViboWriteObject& input, ONX_Model* model, std::string& error) {
  if (input.coordinate_count != 16 || input.index_count != 0 || input.knot_u_count != 0 ||
      input.knot_v_count != 0 || input.geometry_data == nullptr) {
    error = "invalid model text coordinates";
    return nullptr;
  }
  ByteReader reader(input.geometry_data, input.geometry_data_count);
  uint32_t version = 0, archive_count = 0;
  std::string plain, rich;
  if (!reader.U32(version) || version != model_text_payload_version || !reader.U32(archive_count)) {
    error = "invalid model text payload";
    return nullptr;
  }
  const auto* archive = reader.Take(archive_count);
  if (archive == nullptr || !reader.CString(plain) || !reader.CString(rich) || !reader.Finished()) {
    error = "invalid model text data";
    return nullptr;
  }
  const double* coordinates = input.coordinates;
  const ON_Plane plane(ON_3dPoint(coordinates), ON_3dVector(coordinates + 3), ON_3dVector(coordinates + 6));
  const double rotation = coordinates[9], width = coordinates[10], factor = coordinates[11];
  if (!plane.IsValid() || !std::isfinite(rotation) || !std::isfinite(width) || width < 0.0 ||
      !std::isfinite(factor) || factor <= 0.0) {
    error = "invalid model text layout";
    return nullptr;
  }
  ON_DimStyle decoded = ON_DimStyle::Default;
  if (archive_count != 0 && !vibo_annotation::read(archive, archive_count, decoded)) {
    error = "invalid model text style";
    return nullptr;
  }
  if (factor != 1.0) {
    const double height = decoded.TextHeight() * factor;
    const double margin = decoded.MaskBorder() * factor;
    if (!std::isfinite(height) || height <= 0.0 || !std::isfinite(margin)) {
      error = "invalid scaled text style";
      return nullptr;
    }
    decoded.SetTextHeight(height);
    decoded.SetMaskBorder(margin);
  }
  const auto* style = archive_count == 0 && factor == 1.0 ? &ON_DimStyle::Default
      : model ? install_object_annotation_style(*model, decoded) : &decoded;
  const ON_wString contents((rich.empty() ? plain : rich).c_str());
  auto text = std::make_unique<ON_Text>();
  if (style == nullptr || !text->Create(contents, style, plane, width > 0.0, width, rotation)) {
    error = "OpenNURBS could not create model text";
    return nullptr;
  }
  return text.release();
}
