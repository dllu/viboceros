// Owned archive inputs, built only against the public OpenNURBS API.
#include "opennurbs_public.h"
#include <filesystem>
#include <iostream>
#include <string>

int main(int argc, char** argv) {
  if (argc != 2) return 2;
  ON::Begin();
  std::filesystem::create_directories(argv[1]);
  for (const std::string name : {"zero", "zero-default", "zero-constructor",
      "zero-tolerance", "zero-grid64", "zero-simple", "zero-subd2",
      "zero-render-quality", "zero-render-custom", "tiny", "epsilon", "positive",
      "zero-compute-off", "zero-texture2", "zero-custom", "zero-absolute4",
      "default-compute-on", "default-texture1", "default-custom"}) {
    const auto path = std::filesystem::path(argv[1]) / (name + ".3dm");
    if (std::filesystem::exists(path)) continue;
    ONX_Model model;
    auto& mp = model.m_settings.m_AnalysisMeshSettings;
    if (name == "zero-default" || name.rfind("default-", 0) == 0) mp = ON_MeshParameters::DefaultMesh;
    if (name == "zero-constructor") mp = ON_MeshParameters(0.0);
    mp.SetRelativeTolerance(name == "tiny" ? 1e-300 : name == "epsilon" ? 1e-12 : name == "positive" ? 0.25 : 0.0);
    if (name == "zero-tolerance") mp.SetTolerance(0.01);
    if (name == "zero-grid64") mp.SetGridMinCount(64);
    if (name == "zero-simple") mp.SetSimplePlanes(true);
    if (name == "zero-subd2") mp.SetSubDDisplayParameters(ON_SubDDisplayParameters::CreateFromAbsoluteDisplayDensity(2));
    if (name == "zero-absolute4") mp.SetSubDDisplayParameters(ON_SubDDisplayParameters::CreateFromAbsoluteDisplayDensity(4));
    if (name == "zero-compute-off") mp.SetComputeCurvature(false);
    if (name == "default-compute-on") mp.SetComputeCurvature(true);
    if (name == "zero-texture2") mp.SetTextureRange(2);
    if (name == "default-texture1") mp.SetTextureRange(1);
    if (name == "zero-custom" || name == "default-custom") mp.SetCustomSettings(true);
    if (name == "zero-render-quality") model.m_settings.m_RenderMeshSettings = ON_MeshParameters::QualityRenderMesh;
    if (name == "zero-render-custom") {
      model.m_settings.m_CustomRenderMeshSettings = ON_MeshParameters(0.2);
      model.m_settings.m_RenderMeshSettings = model.m_settings.m_CustomRenderMeshSettings;
    }
    if (!model.Write(path.c_str(), 80)) return 3;
    ONX_Model reread;
    if (!reread.Read(path.c_str()) || reread.m_settings.m_AnalysisMeshSettings.ContentHash() != mp.ContentHash()) return 4;
    std::cout << name << '\n';
  }
  return 0;
}
