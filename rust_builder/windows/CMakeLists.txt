# The Flutter tooling requires that developers have a version of Visual Studio
# installed that includes CMake 3.14 or later. You should not increase this
# version, as doing so will cause the plugin to fail to compile for some
# customers of the plugin.
cmake_minimum_required(VERSION 3.14)

# Project-level configuration.
set(PROJECT_NAME "rust_lib_reel")
project(${PROJECT_NAME} LANGUAGES CXX)

include("../cargokit/cmake/cargokit.cmake")

# On Windows, CMAKE_CURRENT_SOURCE_DIR goes through plugin symlinks and ../../
# doesn't resolve correctly. Use CMAKE_SOURCE_DIR (the top-level windows/ dir)
# which is NOT through symlinks, then navigate to the rust directory.
get_filename_component(RUST_MANIFEST_DIR "${CMAKE_SOURCE_DIR}/../rust" ABSOLUTE)

apply_cargokit(${PROJECT_NAME} "${RUST_MANIFEST_DIR}" rust_lib_reel "")

# List of absolute paths to libraries that should be bundled with the plugin.
# This list could contain prebuilt libraries, or libraries created by an
# external build triggered from this build file.
set(rust_lib_reel_bundled_libraries
  "${${PROJECT_NAME}_cargokit_lib}"
  PARENT_SCOPE
)
