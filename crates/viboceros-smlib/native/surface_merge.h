// Join private spline supports. JoinSurface can mutate its cached operands.
extern "C" int vb_nurbs_join_surfaces(const VbNurbsData *first,
    const VbNurbsData *second, int axis, VbSurfaceData **out,
    char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(first && second && out && (axis == 0 || axis == 1), "Invalid surface join inputs");
        std::unique_ptr<SmSurface> a(vb_import_query_surface(*first));
        std::unique_ptr<SmSurface> b(vb_import_query_surface(*second));
        require(a && b && a->IsKindOf(SmBSplineSurface_TYPE) && b->IsKindOf(SmBSplineSurface_TYPE),
            "Surface joining requires spline supports");
        auto *sa = static_cast<SmBSplineSurface *>(a.get());
        auto *sb = static_cast<SmBSplineSurface *>(b.get());
        SmBSplineSurface *raw = nullptr;
        // Public flags describe parameter flow: U joins MaxU to MinU, V joins
        // MaxV to MinV. The header's boundary comments are interchanged.
        const auto status = sa->JoinSurface(*sa->GetContext(), sb,
            axis == 0 ? SM_SP_U : SM_SP_V, 0., raw);
        std::unique_ptr<SmBSplineSurface> joined(raw);
        check(status, "join spline supports");
        require(joined != nullptr, "Missing joined spline support");
        *out = vb_export_surface(*joined);
    });
}
