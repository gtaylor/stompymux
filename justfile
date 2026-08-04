set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

build_dir := ".build"
iwyu_build_dir := ".iwyu-build"
build_type := env_var_or_default("CMAKE_BUILD_TYPE", "RelWithDebInfo")
clang_tidy := env_var_or_default("CLANG_TIDY", "clang-tidy-20")
run_clang_tidy := env_var_or_default("RUN_CLANG_TIDY", "run-clang-tidy-20")
clang_format := env_var_or_default("CLANG_FORMAT", "clang-format-20")
iwyu := env_var_or_default("IWYU", "include-what-you-use-20")
stylua := env_var_or_default("STYLUA", "stylua")

default: fmt build test install

ci: check-mux-source-size fmt-check build test

agent-checks: ci

check-mux-source-size:
    status=0; while IFS= read -r -d '' source; do lines=$(awk 'END { print NR }' "$source"); if (( lines > 800 )); then echo "$source: $lines lines (maximum 800)"; status=1; fi; done < <(find src/mux -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0); exit "$status"

fmt:
    find src -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0 | xargs -0 -r {{clang_format}} -i
    {{stylua}} --glob '**/*.lua' -- game/lua

fmt-check:
    find src -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0 | xargs -0 -r {{clang_format}} --dry-run --Werror
    {{stylua}} --check --glob '**/*.lua' -- game/lua

tidy:
    {{run_clang_tidy}} -clang-tidy-binary {{clang_tidy}} -p {{build_dir}} -j "$(nproc)" '^.*/src/(mux|btech)/.*[.]c$'

iwyu:
    cmake -S . -B {{iwyu_build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON -DBTECH_ENABLE_IWYU=ON -DIWYU_EXECUTABLE={{iwyu}}
    cmake --build {{iwyu_build_dir}} --clean-first --target btech stompymux -j "$(nproc)"

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
