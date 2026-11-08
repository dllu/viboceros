// Fit smooth supports in inherited parameters, before B-rep normalization.
extern "C" int vb_nurbs_offset_surface(const VbNurbsData *input, double distance,
    double approximation, VbSurfaceData **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(input && out && std::isfinite(distance) && distance != 0 &&
            std::isfinite(approximation) && approximation > SM_EFF_ZERO, "Invalid surface offset distance or approximation");
        std::unique_ptr<SmSurface> source(vb_import_query_surface(*input));
        require(source && source->IsKindOf(SmBSplineSurface_TYPE), "Expected spline surface");
        auto *spline = static_cast<SmBSplineSurface *>(source.get());
        SmBSplineSurface *raw = nullptr;
        const auto status = spline->ApproximateOffsetSurface(*spline->GetContext(), distance, approximation, raw, 0);
        std::unique_ptr<SmBSplineSurface> fitted(raw);
        check(status, "fit normal-offset support");
        require(fitted != nullptr, "Missing normal-offset support");
        *out = vb_export_surface(*fitted);
    });
}
