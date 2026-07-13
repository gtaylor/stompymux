set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

build_dir := "build"
build_type := env_var_or_default("CMAKE_BUILD_TYPE", "RelWithDebInfo")

default: lint build test install

fmt:
    find src -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0 | xargs -0 -r clang-format -i

tidy:
    cmake -S . -B {{build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    find src -type f -name '*.c' -print0 | xargs -0 -r -n1 clang-tidy -p {{build_dir}}

lint: fmt tidy

lint-changes:
    git diff --name-only -z --diff-filter=ACMR HEAD -- src | while IFS= read -r -d '' file; do case "$file" in *.c|*.h|*.h.in) clang-format -i "$file" ;; esac; done
    cmake -S . -B {{build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    git diff --name-only -z --diff-filter=ACMR HEAD -- src | while IFS= read -r -d '' file; do case "$file" in *.c) clang-tidy -p {{build_dir}} "$file" ;; esac; done

build:
    cmake -S . -B {{build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    cmake --build {{build_dir}} -j "$(nproc)"

test:
    ctest --test-dir {{build_dir}} --output-on-failure

install: build
    cmake --install {{build_dir}} --prefix "$PWD/game.run"
