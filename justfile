set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

build_dir := "build"
build_type := env_var_or_default("CMAKE_BUILD_TYPE", "RelWithDebInfo")
# clang-tidy must understand C23 `constexpr`, which landed in Clang 19.
clang_tidy := env_var_or_default("CLANG_TIDY", "clang-tidy-20")
run_clang_tidy := env_var_or_default("RUN_CLANG_TIDY", "run-clang-tidy-20")
clang_format := env_var_or_default("CLANG_FORMAT", "clang-format-20")

default: fmt build test install

ci: fmt-check build test

agent-checks: ci

fmt:
    find src -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0 | xargs -0 -r {{clang_format}} -i

fmt-check:
    find src -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0 | xargs -0 -r {{clang_format}} --dry-run --Werror

tidy:
    {{run_clang_tidy}} -clang-tidy-binary {{clang_tidy}} -p {{build_dir}} -j "$(nproc)" '^.*/src/(mux|btech)/.*[.]c$'

build:
    cmake -S . -B {{build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    cmake --build {{build_dir}} -j "$(nproc)"

test:
    ctest --test-dir {{build_dir}} --output-on-failure

install:
    cmake --install {{build_dir}} --prefix "$PWD/game"

run:
    cd game && ulimit -c unlimited && exec ./stompymux stompymux.toml

install-and-run: install run

docsite:
    npm --prefix docs run build

docsite-serve:
    npm --prefix docs run serve
