cmake_minimum_required(VERSION 3.13)

project(tree-sitter
        VERSION "0.27.0"
        DESCRIPTION "An incremental parsing system for programming tools"
        HOMEPAGE_URL "https://tree-sitter.github.io/tree-sitter/"
        LANGUAGES C)

option(BUILD_SHARED_LIBS "Build using shared libraries" ON)
option(TREE_SITTER_FEATURE_WASM "Enable the Wasm feature" OFF)
option(AMALGAMATED "Build using an amalgamated source" OFF)

if(AMALGAMATED)
  set(TS_SOURCE_FILES "${PROJECT_SOURCE_DIR}/lib/src/lib.c")
else()
  file(GLOB TS_SOURCE_FILES lib/src/*.c)
  list(REMOVE_ITEM TS_SOURCE_FILES "${PROJECT_SOURCE_DIR}/lib/src/lib.c")
  # Modules ported to Rust (crates/runtime). Their symbols come from the Rust
  # staticlib built below; including the C source as well would cause duplicate
  # symbol errors at link time.
  list(REMOVE_ITEM TS_SOURCE_FILES "${PROJECT_SOURCE_DIR}/lib/src/point.c")
endif()

add_library(tree-sitter ${TS_SOURCE_FILES})

# --- Rust runtime (crates/runtime) ------------------------------------------
# Pure-Rust reimplementation of modules under lib/src/, built as a staticlib
# and linked into the tree-sitter library. Modules are ported incrementally;
# see the REMOVE_ITEM calls above for the current set.
if(NOT AMALGAMATED)
  set(TS_RUNTIME_DIR "${PROJECT_SOURCE_DIR}/crates/runtime")

  # Map CMake build type -> cargo profile directory.
  #   Debug            -> target/<triple?>/debug
  #   everything else  -> target/<triple?>/release (Release, RelWithDebInfo, MinSizeRel, unset)
  if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(_ts_cargo_flag "")
    set(_ts_cargo_profile_dir "debug")
  else()
    set(_ts_cargo_flag "--release")
    set(_ts_cargo_profile_dir "release")
  endif()

  # Respect cross-compilation: derive a Rust target triple from CMake and pass
  # it via --target so cargo emits into target/<triple>/<profile>/. If no
  # triple is available we fall back to the host target.
  set(_ts_cargo_target_arg "")
  set(_ts_cargo_target_dir "")
  if(CMAKE_CROSSCOMPILING AND DEFINED CMAKE_C_COMPILER_TARGET AND NOT CMAKE_C_COMPILER_TARGET STREQUAL "")
    set(_ts_cargo_target_arg "--target=${CMAKE_C_COMPILER_TARGET}")
    set(_ts_cargo_target_dir "${CMAKE_C_COMPILER_TARGET}/")
  endif()

  # Platform-appropriate staticlib name. On MSVC Cargo emits
  # `tree_sitter_runtime.lib` (no `lib` prefix); on Unix, `libtree_sitter_runtime.a`.
  # `CMAKE_STATIC_LIBRARY_{PREFIX,SUFFIX}` reflect the target platform's conventions.
  set(TS_RUNTIME_STATIC
    "${PROJECT_SOURCE_DIR}/target/${_ts_cargo_target_dir}${_ts_cargo_profile_dir}/${CMAKE_STATIC_LIBRARY_PREFIX}tree_sitter_runtime${CMAKE_STATIC_LIBRARY_SUFFIX}")

  file(GLOB_RECURSE TS_RUNTIME_SOURCES
       "${TS_RUNTIME_DIR}/src/*.rs"
       "${TS_RUNTIME_DIR}/Cargo.toml")

  add_custom_command(
    OUTPUT  "${TS_RUNTIME_STATIC}"
    COMMAND ${CMAKE_COMMAND} -E env cargo build ${_ts_cargo_flag} ${_ts_cargo_target_arg}
            -p tree-sitter-runtime
    WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
    DEPENDS ${TS_RUNTIME_SOURCES}
    COMMENT "Building tree-sitter-runtime (Rust, profile=${_ts_cargo_profile_dir})"
    VERBATIM)
  add_custom_target(tree-sitter-runtime-build DEPENDS "${TS_RUNTIME_STATIC}")
  add_dependencies(tree-sitter tree-sitter-runtime-build)
  target_link_libraries(tree-sitter PRIVATE "${TS_RUNTIME_STATIC}")
endif()
# ---------------------------------------------------------------------------

target_include_directories(tree-sitter PRIVATE lib/src lib/src/wasm PUBLIC lib/include)

if(MSVC)
  target_compile_options(tree-sitter PRIVATE
                         /wd4018 # disable 'signed/unsigned mismatch'
                         /wd4232 # disable 'nonstandard extension used'
                         /wd4244 # disable 'possible loss of data'
                         /wd4267 # disable 'possible loss of data (size_t)'
                         /wd4701 # disable 'potentially uninitialized local variable'
                         /we4022 # treat 'incompatible types' as an error
                         /W4)
else()
  target_compile_options(tree-sitter PRIVATE
                         -Wall -Wextra -Wshadow -Wpedantic
                         -Werror=incompatible-pointer-types)
endif()

if(TREE_SITTER_FEATURE_WASM)
  if(NOT DEFINED CACHE{WASMTIME_INCLUDE_DIR})
    message(CHECK_START "Looking for wasmtime headers")
    find_path(WASMTIME_INCLUDE_DIR wasmtime.h
              PATHS ENV DEP_WASMTIME_C_API_INCLUDE)
    if(NOT WASMTIME_INCLUDE_DIR)
      unset(WASMTIME_INCLUDE_DIR CACHE)
      message(FATAL_ERROR "Could not find wasmtime headers.\nDid you forget to set CMAKE_INCLUDE_PATH?")
    endif()
    message(CHECK_PASS "found")
  endif()

  if(NOT DEFINED CACHE{WASMTIME_LIBRARY})
    message(CHECK_START "Looking for wasmtime library")
    find_library(WASMTIME_LIBRARY wasmtime)
    if(NOT WASMTIME_LIBRARY)
      unset(WASMTIME_LIBRARY CACHE)
      message(FATAL_ERROR "Could not find wasmtime library.\nDid you forget to set CMAKE_LIBRARY_PATH?")
    endif()
    message(CHECK_PASS "found")
  endif()

  target_compile_definitions(tree-sitter PUBLIC TREE_SITTER_FEATURE_WASM)
  target_include_directories(tree-sitter SYSTEM PRIVATE "${WASMTIME_INCLUDE_DIR}")
  target_link_libraries(tree-sitter PUBLIC "${WASMTIME_LIBRARY}")
  set_property(TARGET tree-sitter PROPERTY C_STANDARD_REQUIRED ON)

  if(NOT BUILD_SHARED_LIBS)
    if(WIN32)
      target_compile_definitions(tree-sitter PRIVATE WASM_API_EXTERN= WASI_API_EXTERN=)
      target_link_libraries(tree-sitter INTERFACE ws2_32 advapi32 userenv ntdll shell32 ole32 bcrypt)
    elseif(NOT APPLE)
      target_link_libraries(tree-sitter INTERFACE pthread dl m)
    endif()
  endif()
endif()

set_target_properties(tree-sitter
                      PROPERTIES
                      C_STANDARD 11
                      C_VISIBILITY_PRESET hidden
                      POSITION_INDEPENDENT_CODE ON
                      SOVERSION "${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR}"
                      DEFINE_SYMBOL "")

target_compile_definitions(tree-sitter PRIVATE _POSIX_C_SOURCE=200112L _DEFAULT_SOURCE _BSD_SOURCE _DARWIN_C_SOURCE)

include(GNUInstallDirs)

configure_file(lib/tree-sitter.pc.in "${CMAKE_CURRENT_BINARY_DIR}/tree-sitter.pc" @ONLY)

install(FILES lib/include/tree_sitter/api.h
        DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}/tree_sitter")
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/tree-sitter.pc"
        DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig")
install(TARGETS tree-sitter
        LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}")
