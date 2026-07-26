# Forces rtaudio-sys's vendored RtAudio build (CMAKE_BUILD_TYPE=Debug, see
# root Cargo.toml's opt-level pin) to use the static *release* CRT (/MT),
# matching Rust's build (compiled with -C target-feature=+crt-static, see
# release.yml). Without this, RtAudio's own CMakeLists.txt rewrites CMake's
# default Debug flags ("/MDd") to "/MTd" (static *debug* CRT), which pulls
# in debug-only allocator symbols (_CrtDbgReport, _malloc_dbg, etc.) that
# don't exist in the release CRT Rust links against, failing at link time.
# RTAUDIO_STATIC_MSVCRT is left at its default (ON for a static lib), since
# it's what performs the flag rewrite we're relying on here.
set(CMAKE_C_FLAGS_DEBUG "/MD" CACHE STRING "" FORCE)
set(CMAKE_CXX_FLAGS_DEBUG "/MD" CACHE STRING "" FORCE)
