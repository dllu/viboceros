#pragma once
#include "opennurbs_public.h"
#include <vector>

namespace vibo_analysis {
// Public geometry-setting comparisons retain custom zero-density profiles.
// Qualified against RhinoDoc.OpenHeadless/Write3dmFile in analysis_profile*.json.
inline ON_MeshParameters for_document(const ON_MeshParameters& stored) {
  auto result = stored;
  if (0 == ON_MeshParameters::CompareGeometrySettings(stored, ON_MeshParameters::DefaultMesh))
    result = ON_MeshParameters::DefaultAnalysisMesh;
  auto legacy = ON_MeshParameters::DefaultAnalysisMesh;
  legacy.SetRelativeTolerance(0.0);
  if (0 == ON_MeshParameters::CompareGeometrySettings(stored, legacy))
    result = ON_MeshParameters::FastRenderMesh;
  // Analysis texture coordinates use the packed [0,1] range, even for custom profiles.
  result.SetTextureRange(1);
  return result;
}

inline bool encode(const ON_MeshParameters& parameters, std::vector<uint8_t>& bytes) {
  ON_Write3dmBufferArchive archive(256, 65536, 80, ON::Version());
  if (!parameters.Write(archive)) return false;
  const auto* data = static_cast<const uint8_t*>(archive.Buffer());
  bytes.assign(data, data + archive.SizeOfArchive());
  return true;
}

inline bool decode(const uint8_t* data, size_t count, ON_MeshParameters& parameters) {
  if (!data || count == 0 || count > 65536) return false;
  ON_Read3dmBufferArchive archive(count, data, false, 80, ON::Version());
  return parameters.Read(archive) && archive.CurrentPosition() == count;
}

inline bool retained(const ON_MeshParameters& parameters, std::vector<uint8_t>& bytes) {
  if (!encode(parameters, bytes)) return false;
  // The public fields reconstruct this profile without retaining native bytes.
  auto canonical = ON_MeshParameters::DefaultAnalysisMesh;
  canonical.SetRelativeTolerance(parameters.RelativeTolerance());
  canonical.SetSimplePlanes(parameters.SimplePlanes());
  auto subd = canonical.SubDDisplayParameters();
  const auto source = parameters.SubDDisplayParameters();
  if (source.DisplayDensityIsAdaptive()) subd.SetAdaptiveDisplayDensity(source.GetRawDisplayDensityForExperts());
  else subd.SetAbsoluteDisplayDensity(source.GetRawDisplayDensityForExperts());
  canonical.SetSubDDisplayParameters(subd);
  std::vector<uint8_t> expected;
  if (!encode(canonical, expected)) return false;
  if (bytes == expected) bytes.clear();
  return true;
}
} // namespace vibo_analysis
