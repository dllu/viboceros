# Evaluation uses exact nonzero spans without changing the editing helpers.
find_program(PATCH_EXECUTABLE patch REQUIRED)
foreach(repair IN ITEMS
    "NL_BasisBasic.cpp|usd-brep-basis-evaluation.patch"
    "NL_CrvNurb.cpp|usd-brep-curve-evaluation-basis.patch")
    string(REPLACE "|" ";" repair_parts "${repair}")
    list(GET repair_parts 0 source_name)
    list(GET repair_parts 1 patch_name)
    set(eval_source "${USD_BREP_ROOT}/source/SMLib/src/${source_name}")
    set(eval_patch "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/${patch_name}")
    set(eval_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/${source_name}")
    file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched")
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${eval_source}" "${eval_patch}")
    execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${eval_output}" "${eval_source}" "${eval_patch}"
                    RESULT_VARIABLE eval_patch_status)
    if(NOT eval_patch_status EQUAL 0)
        message(FATAL_ERROR "usd-brep basis evaluation repair failed; review the pinned source")
    endif()
    list(REMOVE_ITEM KERNEL_SOURCES "${eval_source}")
    list(APPEND KERNEL_SOURCES "${eval_output}")
endforeach()

# An opt-in control executable replaces only these two archive objects with
# the pinned parent implementations, retaining all other local repairs.
add_executable(viboceros_smlib_basis_evaluation_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/basis_evaluation_probe.cpp"
    "${USD_BREP_ROOT}/source/SMLib/src/NL_BasisBasic.cpp"
    "${USD_BREP_ROOT}/source/SMLib/src/NL_CrvNurb.cpp")
target_link_libraries(viboceros_smlib_basis_evaluation_baseline PRIVATE SMLib)
