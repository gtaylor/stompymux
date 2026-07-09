set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

build_dir := "build"
build_type := env_var_or_default("CMAKE_BUILD_TYPE", "RelWithDebInfo")

default: fmt tidy build test

fmt:
    git ls-files '*.c' '*.cc' '*.cpp' '*.cxx' '*.h' '*.hh' '*.hpp' '*.hxx' '*.h.in' | xargs -r clang-format -i

tidy:
    cmake -S . -B {{build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    git ls-files '*.c' '*.cc' '*.cpp' '*.cxx' | xargs -r clang-tidy -p {{build_dir}}

build:
    cmake -S . -B {{build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    cmake --build {{build_dir}} -j "$(nproc)"

test:
    ctest --test-dir {{build_dir}} --output-on-failure

install: build
    cmake --install {{build_dir}} --prefix "$PWD/game.run"
