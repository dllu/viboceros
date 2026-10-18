set(fillet_source "${USD_BREP_ROOT}/source/SMLib/src/SmFilletExecutive.cpp")
set(fillet_patch "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-surface-fillet-domain.patch")
set(fillet_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmFilletExecutiveDomains.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${fillet_source}" "${fillet_patch}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${fillet_output}" "${fillet_source}" "${fillet_patch}"
                RESULT_VARIABLE fillet_patch_status)
if(NOT fillet_patch_status EQUAL 0)
    message(FATAL_ERROR "usd-brep surface fillet domain repair failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${fillet_source}")
list(APPEND KERNEL_SOURCES "${fillet_output}")
set(fillet_header "${USD_BREP_ROOT}/source/SMLib/inc/SmFilletExecutive.h")
set(fillet_header_patch "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-surface-fillet-header.patch")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${fillet_header}" "${fillet_header_patch}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0
    -o "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmFilletExecutive.h" "${fillet_header}" "${fillet_header_patch}"
    RESULT_VARIABLE fillet_header_status)
if(NOT fillet_header_status EQUAL 0)
    message(FATAL_ERROR "usd-brep surface fillet declaration patch failed; review the pinned source")
endif()

# Compatibility shim for a controlled comparison of the original 0.5 policy.
# Keep the new defaulted parameter for linkage and remove only its use.
file(READ "${fillet_output}" fillet_baseline_source)
string(FIND "${fillet_baseline_source}"
    "      pFS->SetExtensionFactor(eFilTrimType == SM_BT_MINIMAL ? 0.0 : dExtensionFactor);" fillet_policy_position)
if(fillet_policy_position EQUAL -1)
    message(FATAL_ERROR "Surface fillet baseline policy no longer matches the repaired source")
endif()
string(REPLACE "      pFS->SetExtensionFactor(eFilTrimType == SM_BT_MINIMAL ? 0.0 : dExtensionFactor);"
               "" fillet_baseline_source "${fillet_baseline_source}")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmFilletExecutiveBaseline.cpp" "${fillet_baseline_source}")
