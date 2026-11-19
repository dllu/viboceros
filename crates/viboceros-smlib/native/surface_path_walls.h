#pragma once

namespace {

std::unique_ptr<SmBSplineCurve> vb_join_surface_path_profile(
    const SmTArray<SmBSplineCurve *> &segments, const SmTArray<SmBoolean> *senses = nullptr) {
    require(segments.GetSize() > 0, "Empty surface path boundary chain");
    auto *context = SmApiGetOrCreateContext();
    if (segments.GetSize() == 1) {
        auto &curve = *segments[0];
        auto copy = std::unique_ptr<SmBSplineCurve>(new (*context) SmBSplineCurve(curve));
        vb_preserve_curve_basis(*copy, vb_curve_full_knots(curve));
        return copy;
    }
    SmBSplineCurve *joined = nullptr;
    const auto status = SmBSplineCurve::CreateByJoining(*context, segments, senses, joined);
    std::unique_ptr<SmBSplineCurve> profile(joined);
    check(status, "join surface path boundary chain");
    require(joined, "Missing joined surface path profile");
    return profile;
}

std::vector<std::unique_ptr<SmBSplineCurve>> vb_surface_path_profiles(
    const SmTArray<SmEdge *> &edges, const SmFace *sheet,
    bool split_at_tangents, double tolerance) {
    auto *context = SmApiGetOrCreateContext();
    std::vector<std::unique_ptr<SmBSplineCurve>> owned;
    SmTArray<SmCurve *> chains;
    for (ULONG i = 0; i < edges.GetSize(); ++i) {
        auto &original = *static_cast<SmBSplineCurve *>(edges[i]->GetCurve());
        auto profile = std::unique_ptr<SmBSplineCurve>(new (*context) SmBSplineCurve(original));
        vb_preserve_curve_basis(*profile, vb_curve_full_knots(original));
        auto interval = edges[i]->GetInterval();
        check(profile->Trim(interval, FALSE), "trim surface path source edge");
        chains.Add(profile.get());
        owned.push_back(std::move(profile));
    }
    if (split_at_tangents || owned.size() == 1) return owned;

    if (sheet) {
        SmTArray<SmLoop *> loops;
        sheet->GetLoops(loops);
        bool all_boundary_loops = true;
        for (ULONG i = 0; i < loops.GetSize(); ++i)
            all_boundary_loops = all_boundary_loops && loops[i]->IsLamina();
        if (all_boundary_loops) {
            // The usual command input is a single face with ordered loops.
            // Preserve loop identity and orientation without endpoint searches.
            std::unordered_map<const SmEdge *, SmBSplineCurve *> profile_for_edge;
            for (ULONG i = 0; i < edges.GetSize(); ++i)
                profile_for_edge.emplace(edges[i], owned[i].get());
            std::vector<std::unique_ptr<SmBSplineCurve>> profiles;
            for (ULONG i = 0; i < loops.GetSize(); ++i) {
                SmTArray<SmEdgeuse *> uses;
                loops[i]->GetEdgeuses(uses);
                if (uses.GetSize() == 0) continue;
                SmTArray<SmBSplineCurve *> segments;
                SmTArray<SmBoolean> senses;
                for (ULONG j = 0; j < uses.GetSize(); ++j) {
                    segments.Add(profile_for_edge.at(uses[j]->GetEdge()));
                    senses.Add(uses[j]->GetOrientation() == SM_OT_SAME);
                }
                profiles.push_back(vb_join_surface_path_profile(segments, &senses));
            }
            return profiles;
        }
    }

    // Chain edges before measuring geometric continuity across their joins.
    SmTArray<ULONG> starts;
    require(SmCurve::FixChainCurvesList(chains, starts, tolerance, FALSE) > 0,
            "Missing surface path boundary chains");
    std::vector<std::unique_ptr<SmBSplineCurve>> profiles;
    for (ULONG i = 0; i < starts.GetSize(); ++i) {
        const ULONG start = starts[i];
        const ULONG end = i + 1 < starts.GetSize() ? starts[i + 1] : chains.GetSize();
        SmTArray<SmBSplineCurve *> segments;
        for (ULONG j = start; j < end; ++j)
            segments.Add(static_cast<SmBSplineCurve *>(chains[j]));
        profiles.push_back(vb_join_surface_path_profile(segments));
    }
    return profiles;
}

void vb_create_surface_path_walls(SmBrep &output, const SmTArray<SmEdge *> &edges,
                                 const SmFace *sheet, const VbCurve &path,
                                 bool split_at_tangents, double tolerance) {
    // One path origin for all segments keeps adjacent walls coincident.
    auto relative_path = sweep_path_at_origin(path);
    SmTArray<SmBSplineCurve *> rails;
    SmObjsDelete<SmBSplineCurve *> rail_cleanup(&rails);
    vb_sweep_curve_segments(*relative_path, false, rails);
    std::vector<std::vector<double>> rail_knots;
    size_t rail_controls = 0;
    for (ULONG i = 0; i < rails.GetSize(); ++i) {
        const size_t count = rails[i]->GetNumberControlPoints();
        require(count <= limit - rail_controls, "Surface path segment control-net limit");
        rail_controls += count;
        rail_knots.push_back(vb_curve_full_knots(*rails[i]));
    }
    require(rail_controls > 0, "Missing segmented surface extrusion path");
    size_t remaining_controls = limit;
    size_t remaining_faces = 100000;
    for (const auto &profile : vb_surface_path_profiles(edges, sheet, split_at_tangents, tolerance)) {
        SmTArray<SmBSplineCurve *> sections;
        SmObjsDelete<SmBSplineCurve *> section_cleanup(&sections);
        vb_sweep_curve_segments(*profile, split_at_tangents, sections);
        require(sections.GetSize() <= remaining_faces / rails.GetSize(),
                "Surface path wall face limit");
        remaining_faces -= sections.GetSize() * rails.GetSize();
        for (ULONG j = 0; j < sections.GetSize(); ++j) {
            auto &section = *sections[j];
            const size_t count = section.GetNumberControlPoints();
            require(count <= remaining_controls / rail_controls,
                    "Surface path split control-net limit");
            remaining_controls -= count * rail_controls;
            const auto knots = vb_curve_full_knots(section);
            for (ULONG k = 0; k < rails.GetSize(); ++k) {
                auto wall = exact_translational_surface(section, *rails[k], {knots, rail_knots[k]});
                auto *raw = wall.get();
                SmFace *face = nullptr;
                const auto status = output.CreateFaceFromSurface(raw, raw->GetNaturalUVDomain(), face);
                if (raw->GetOwner()) wall.release();
                check(status, "create surface path wall");
                require(face, "Missing surface path wall");
            }
        }
    }
}

} // namespace
