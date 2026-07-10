set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

build_dir := "build"
build_type := env_var_or_default("CMAKE_BUILD_TYPE", "RelWithDebInfo")

default: fmt tidy build test

fmt:
    find src -type f \( -name '*.c' -o -name '*.cc' -o -name '*.cpp' -o -name '*.cxx' -o -name '*.h' -o -name '*.hh' -o -name '*.hpp' -o -name '*.hxx' -o -name '*.h.in' \) -print0 | xargs -0 -r clang-format -i

tidy:
    cmake -S . -B {{build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    mapfile -t cxx_include_dirs < <(g++ -v -x c++ -E /dev/null 2>&1 | sed -n '/#include <...> search starts here:/,/End of search list./p' | sed '1d;$d;s/^ //'); \
    cxx_args=(); \
    for dir in "${cxx_include_dirs[@]}"; do cxx_args+=(--extra-arg=-isystem"$dir"); done; \
    find src -type f \( -name '*.c' -o -name '*.cc' -o -name '*.cpp' -o -name '*.cxx' \) | while read -r file; do \
      case "$file" in \
        *.cc|*.cpp|*.cxx) clang-tidy -p {{build_dir}} "$file" "${cxx_args[@]}" ;; \
        *) clang-tidy -p {{build_dir}} "$file" ;; \
      esac; \
    done

build:
    cmake -S . -B {{build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    cmake --build {{build_dir}} -j "$(nproc)"

test:
    ctest --test-dir {{build_dir}} --output-on-failure

install: build
    cmake --install {{build_dir}} --prefix "$PWD/game.run"
