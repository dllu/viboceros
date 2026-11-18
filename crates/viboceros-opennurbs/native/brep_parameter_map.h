// Move trims into a surface's NURBS parameter frame when GetNurbForm returns 2.
namespace vibo_brep_parameter_map {

// A trim exported at the domain boundary can be a few binary64 ulps outside.
void clamp_domain_roundoff(const ON_NurbsSurface& target, ON_3dPoint& uv) {
  for (int axis = 0; axis < 2; ++axis) {
    const auto domain = target.Domain(axis);
    double& value = uv[axis];
    const double scale = std::max({std::abs(domain[0]), std::abs(domain[1]),
                                   domain.Length()});
    const double roundoff = 16.0 * ON_EPSILON * scale;
    if (value < domain[0] && domain[0] - value <= roundoff) value = domain[0];
    if (value > domain[1] && value - domain[1] <= roundoff) value = domain[1];
  }
}

struct Station {
  double parameter;
  ON_3dPoint uv;
  ON_3dPoint model;
};

struct Mapping {
  const ON_BrepTrim& trim;
  const ON_Surface& source;
  const ON_NurbsSurface& target;
  double tolerance;

  bool station(double parameter, Station& result) const {
    const auto uv = trim.PointAt(parameter);
    if (!uv.IsValid()) return false;
    result.parameter = parameter;
    result.uv.z = 0.0;
    result.model = source.PointAt(uv.x, uv.y);
    if (!result.model.IsValid() ||
        !source.GetNurbFormParameterFromSurfaceParameter(
            uv.x, uv.y, &result.uv.x, &result.uv.y) ||
        !result.uv.IsValid()) return false;
    clamp_domain_roundoff(target, result.uv);
    return true;
  }

  bool subdivide(const Station& first, const Station& last, int depth,
                 std::vector<Station>& stations) const {
    // Off-quarter samples avoid aliasing circular knot spans: at each
    // quadrant boundary the angular and rational parameters can coincide.
    for (double fraction : {0.17, 0.37, 0.5, 0.63, 0.83}) {
      Station sample;
      if (!station(first.parameter + fraction * (last.parameter - first.parameter), sample))
        return false;
      const auto uv = first.uv + fraction * (last.uv - first.uv);
      const auto model = target.PointAt(uv.x, uv.y);
      if (!model.IsValid()) return false;
      if (sample.model.DistanceTo(model) > tolerance) {
        Station middle;
        if (!station(first.parameter + 0.5 * (last.parameter - first.parameter), middle))
          return false;
        return depth < 20 && middle.parameter > first.parameter &&
               middle.parameter < last.parameter &&
               subdivide(first, middle, depth + 1, stations) &&
               subdivide(middle, last, depth + 1, stations);
      }
    }
    if (stations.size() >= 65536) return false;
    stations.push_back(last);
    return true;
  }
};

bool convert(const ON_BrepTrim& trim, const ON_Surface& source,
             const ON_NurbsSurface& target, double tolerance, ON_NurbsCurve& curve) {
  if (!std::isfinite(tolerance) || tolerance <= 0.0) return false;
  const Mapping mapping{trim, source, target, tolerance};
  const auto domain = trim.Domain();
  Station first, last;
  if (!mapping.station(domain[0], first) || !mapping.station(domain[1], last)) return false;
  std::vector<Station> stations{first};
  const auto start = trim.PointAtStart();
  const auto end = trim.PointAtEnd();
  // Revolution parameter changes are separable and monotone. A straight
  // isocurve still has the same locus between its mapped endpoints.
  if (ON_RevSurface::Cast(trim.Face()->SurfaceOf()) != nullptr &&
      ((start.x == end.x) != (start.y == end.y)) && trim.IsLinear(ON_ZERO_TOLERANCE)) {
    stations.push_back(last);
  } else {
    const int span_count = trim.SpanCount();
    if (span_count <= 0 || span_count >= 65536) return false;
    std::vector<double> spans(static_cast<size_t>(span_count) + 1);
    if (!trim.GetSpanVector(spans.data())) return false;
    for (int span = 1; span <= span_count; ++span) {
      if (!mapping.station(spans[span], last) ||
          !mapping.subdivide(first, last, 0, stations)) return false;
      first = last;
    }
  }
  if (!curve.Create(2, false, 2, static_cast<int>(stations.size()))) return false;
  for (int i = 0; i < curve.CVCount(); ++i) {
    if (!curve.SetCV(i, stations[i].uv) || !curve.SetKnot(i, stations[i].parameter))
      return false;
  }
  return true;
}

}  // namespace vibo_brep_parameter_map
