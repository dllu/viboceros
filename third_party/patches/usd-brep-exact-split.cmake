# Compose with the preceding extraction repairs; keep upstream immutable.
set(split_header "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-exact-split-span.h")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${split_header}")
configure_file("${split_header}" "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/usd-brep-exact-split-span.h" COPYONLY)
foreach(split_kind IN ITEMS Curve Surface)
    if(split_kind STREQUAL "Curve")
        set(split_source "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_CrvToolSegments.cpp")
        set(split_patch "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-curve-exact-split.patch")
        set(split_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_CrvToolSplits.cpp")
    else()
        set(split_source "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_SrfToolPatch.cpp")
        set(split_patch "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-surface-exact-split.patch")
        set(split_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_SrfToolSplits.cpp")
    endif()
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${split_patch}")
    execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${split_output}" "${split_source}" "${split_patch}"
                    RESULT_VARIABLE split_patch_status)
    if(NOT split_patch_status EQUAL 0)
        message(FATAL_ERROR "usd-brep exact ${split_kind} split repair failed; review the pinned source")
    endif()
    list(REMOVE_ITEM KERNEL_SOURCES "${split_source}")
    list(APPEND KERNEL_SOURCES "${split_output}")
endforeach()
add_executable(viboceros_smlib_exact_split_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/exact_split_probe.cpp"
    "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_CrvToolSegments.cpp"
    "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_SrfToolPatch.cpp")
target_link_libraries(viboceros_smlib_exact_split_baseline PRIVATE SMLib)
