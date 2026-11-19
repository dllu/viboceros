// The active annotation style is archived separately from the geometry table.
namespace vibo_annotation {

constexpr size_t max_style_size = 8 * 1024 * 1024;
constexpr int archive_version = 80;

bool archive_style(const ON_DimStyle& style, std::vector<uint8_t>& bytes) {
  if (style.Id() == ON_DimStyle::Default.Id() && style.CompareFields(ON_DimStyle::Default)) {
    return true;
  }
  ON_Write3dmBufferArchive archive(1024, max_style_size, archive_version, ON::Version());
  if (!style.Write(archive) || archive.SizeOfArchive() == 0) {
    return false;
  }
  const auto* data = static_cast<const uint8_t*>(archive.Buffer());
  bytes.assign(data, data + archive.SizeOfArchive());
  return true;
}

bool retain(const ONX_Model& model, std::vector<uint8_t>& bytes) {
  const auto reference = model.CurrentDimensionStyle();
  const auto* style = ON_DimStyle::Cast(reference.ModelComponent());
  return style == nullptr || archive_style(*style, bytes);
}

bool read(const uint8_t* data, size_t count, ON_DimStyle& style) {
  if (data == nullptr || count == 0 || count > max_style_size) {
    return false;
  }
  ON_Read3dmBufferArchive archive(count, data, false, archive_version, ON::Version());
  return style.Read(archive) && archive.AtEnd();
}

bool install(ONX_Model& model, const uint8_t* data, size_t count) {
  ON_DimStyle style;
  if (!read(data, count, style)) {
    return false;
  }
  const ON_UUID id = style.Id();
  if (id != ON_nil_uuid && id == ON_DimStyle::SystemDimstyleFromId(id).Id()) {
    return model.SetCurrentDimensionStyleId(id);
  }
  const auto added = model.AddModelComponent(style, true);
  return !added.IsEmpty() && model.SetCurrentDimensionStyleId(added.ModelComponent()->Id());
}

ViboAnnotationLengthFormat length_format(const ON_DimStyle& style) {
  const ON_UnitSystem display_units(style.DimensionLengthDisplayUnit(0));
  const double meters = display_units.MetersPerUnit(0.0);
  return {
    display_units.UnitSystem() == ON::LengthUnitSystem::None ? 0.0 : meters,
    style.LengthFactor(), style.RoundOff(), style.LengthResolution(),
    static_cast<uint32_t>(style.DimensionLengthDisplay()),
    static_cast<uint32_t>(style.ZeroSuppress()),
    static_cast<uint32_t>(style.DecimalSeparator()),
    style.TextHeight() * style.DimScale(),
    style.LeaderArrowSize() * style.DimScale(),
    style.LeaderHasLanding() ? style.LeaderLandingLength() * style.DimScale() : 0.0,
    style.TextGap() * style.DimScale(),
    style.LeaderContentAngleRadians(),
    static_cast<uint32_t>(style.LeaderCurveType()),
    static_cast<uint32_t>(style.LeaderArrowType()),
    static_cast<uint32_t>(style.LeaderContentAngleStyle()),
    style.DrawForward() ? 1u : 0u
  };
}

bool format_coordinates(const ViboAnnotationLengthFormat& format, double model_meters,
                        const double coordinates[3], ON_wString& result) {
  if (format.resolution < 0 || format.resolution >= 16 ||
      !std::isfinite(format.round_off)) {
    return false;
  }
  const double unit_factor = model_meters > 0.0 && format.meters_per_unit > 0.0
      ? model_meters / format.meters_per_unit : 1.0;
  const double tiny = std::pow(10.0, -(format.resolution + 1));
  const auto display = static_cast<ON_DimStyle::LengthDisplay>(format.display);
  const bool fractional = !ON_DimStyle::LengthDisplayIsDecimal(display);
  for (int axis = 0; axis < 3; ++axis) {
    double value = coordinates[axis] * unit_factor * format.length_factor;
    if (!std::isfinite(value)) return false;
    if (std::abs(value) < tiny) value = 0.0;
    // The native fractional formatter represents whole units with int.
    if (fractional && ON_NumberFormatter::RoundOff(std::abs(value), format.round_off) >=
        static_cast<double>(std::numeric_limits<int>::max() - 1)) {
      return false;
    }
    ON_wString component;
    if (!ON_TextContent::FormatLength(
        value, display, format.round_off, format.resolution,
        static_cast<ON_DimStyle::suppress_zero>(format.zero_suppress), false,
        static_cast<wchar_t>(format.decimal_separator), component)) {
      return false;
    }
    if (axis != 0) result += L", ";
    result += component;
  }
  return true;
}

}  // namespace vibo_annotation
