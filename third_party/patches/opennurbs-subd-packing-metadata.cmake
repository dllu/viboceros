# Compile a small independent metadata setter beside the pinned kernel's own
# dimple definition. The upstream checkout stays immutable. Packing decisions
# and rectangle allocation live in Rust; no proprietary packing code is used.
set(packing_source "${OPENNURBS_ROOT}/opennurbs_subd.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${packing_source}" "${CMAKE_CURRENT_LIST_FILE}")
file(READ "${packing_source}" packing_text)
if(packing_text MATCHES "ON_SubDimple::SetFacePackingIdAndTopologyHash")
  message(FATAL_ERROR "OpenNURBS packing metadata adapter needs review for this upstream source")
endif()
string(APPEND packing_text [=[
// Viboceros: independent metadata assignment for the Rust packing algorithm.
void ON_SubDimple::SetFacePackingIdAndTopologyHash(
  ON_UUID id, const ON_SubDHash& topology_hash)
{
  m_face_packing_id = id;
  m_face_packing_topology_hash = topology_hash;
}
void vibo_subd_stamp_face_packs(ON_SubD& subd)
{
  // Version 1: topology partitions with a deterministic grid texture atlas.
  const ON_UUID algorithm = ON_UuidFromString("79cc795b-6031-4d1a-9230-692867f350b4");
  auto* dimple = const_cast<ON_SubDimple*>(subd.SubDimple());
  if (nullptr != dimple)
    dimple->SetFacePackingIdAndTopologyHash(algorithm,
      ON_SubDHash::Create(ON_SubDHashType::TopologyAndEdgeCreases, subd));
}
]=])
set(packing_output "${CMAKE_CURRENT_BINARY_DIR}/opennurbs-patched/opennurbs_subd.cpp")
file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/opennurbs-patched")
set(existing "")
if(EXISTS "${packing_output}")
  file(READ "${packing_output}" existing)
endif()
if(NOT existing STREQUAL packing_text)
  file(WRITE "${packing_output}" "${packing_text}")
endif()
get_target_property(packing_sources opennurbsStatic SOURCES)
set(updated_sources)
set(replaced 0)
foreach(source IN LISTS packing_sources)
  if(source STREQUAL "opennurbs_subd.cpp" OR source STREQUAL packing_source)
    list(APPEND updated_sources "${packing_output}")
    math(EXPR replaced "${replaced}+1")
  elseif(IS_ABSOLUTE "${source}" OR source MATCHES "^\\$<")
    list(APPEND updated_sources "${source}")
  else()
    list(APPEND updated_sources "${OPENNURBS_ROOT}/${source}")
  endif()
endforeach()
if(NOT replaced EQUAL 1)
  message(FATAL_ERROR "OpenNURBS static SubD source replacement failed")
endif()
set_property(TARGET opennurbsStatic PROPERTY SOURCES "${updated_sources}")
