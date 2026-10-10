cmake_minimum_required(VERSION 3.20)
project(viboceros_usd_brep_trial LANGUAGES CXX)

if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
  message(FATAL_ERROR "This standalone trial supports Linux; use the upstream build on other platforms.")
endif()

# Standalone native-kernel experiment. No OpenUSD, Python or Packman required.
get_filename_component(USD_BREP_ROOT "${CMAKE_CURRENT_LIST_DIR}/../../third_party/usd-brep" ABSOLUTE)
if(NOT EXISTS "${USD_BREP_ROOT}/source/SMLib/inc/SmBrep.h")
  message(FATAL_ERROR "Initialize third_party/usd-brep with GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep")
endif()
find_package(TBB REQUIRED COMPONENTS tbb tbbmalloc)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

file(GLOB KERNEL_SOURCES CONFIGURE_DEPENDS "${USD_BREP_ROOT}/source/SMLib/src/*.cpp")
file(GLOB API_SOURCES CONFIGURE_DEPENDS "${USD_BREP_ROOT}/source/SM_API/src/*.cpp")
list(FILTER KERNEL_SOURCES EXCLUDE REGEX "/StdAfx\\.cpp$")
list(FILTER API_SOURCES EXCLUDE REGEX "/StdAfx\\.cpp$")

add_library(SMLib SHARED ${KERNEL_SOURCES})
target_include_directories(SMLib PUBLIC "${USD_BREP_ROOT}/source/SMLib/inc")
target_link_libraries(SMLib PRIVATE TBB::tbb TBB::tbbmalloc)
# Match the upstream release configuration, including its exported class layout.
target_compile_definitions(SMLib PUBLIC NOMINMAX SM_GRAPHICS_CALLBACKS=1)
target_compile_options(SMLib PRIVATE -Wno-write-strings)

add_library(SM_API SHARED ${API_SOURCES})
target_include_directories(SM_API PUBLIC "${USD_BREP_ROOT}/source/SM_API/inc")
target_link_libraries(SM_API PUBLIC SMLib)
target_compile_definitions(SM_API PRIVATE SM_API_EXPORTS)
target_compile_options(SM_API PRIVATE -Wno-write-strings)

add_executable(sm_api_boolean_bounds_example "${USD_BREP_ROOT}/Examples/SM_API/boolean_bounds.cpp")
target_compile_definitions(sm_api_boolean_bounds_example PRIVATE SM_BOOLEAN_BOUNDS_EXAMPLE_STANDALONE)
target_link_libraries(sm_api_boolean_bounds_example PRIVATE SM_API)
add_executable(viboceros_usd_brep_smoke smoke.cpp)
target_link_libraries(viboceros_usd_brep_smoke PRIVATE SM_API)
enable_testing()
add_test(NAME upstream_boolean_bounds COMMAND sm_api_boolean_bounds_example)
add_test(NAME native_smoke COMMAND viboceros_usd_brep_smoke)
