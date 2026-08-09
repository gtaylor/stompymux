set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

build_dir := ".build"
build_type := env_var_or_default("CMAKE_BUILD_TYPE", "RelWithDebInfo")
clang_tidy := env_var_or_default("CLANG_TIDY", "clang-tidy-22")
run_clang_tidy := env_var_or_default("RUN_CLANG_TIDY", "run-clang-tidy-22")
clang_format := env_var_or_default("CLANG_FORMAT", "clang-format-22")
stylua := env_var_or_default("STYLUA", "stylua")

default: fmt build test install

ci: check-mux-source-size fmt-check build test

agent-checks: ci

checks: ci

check-mux-source-size:
    status=0; while IFS= read -r -d '' source; do lines=$(awk 'END { print NR }' "$source"); if (( lines > 800 )); then echo "$source: $lines lines (maximum 800)"; status=1; fi; done < <(find src/mux -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0); exit "$status"

fmt-c:
    find src -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0 | xargs -0 -r {{clang_format}} -i

fmt-lua:
    {{stylua}} --glob '**/*.lua' -- game/lua

fmt: fmt-c fmt-lua

fmt-check-c:
    find src -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0 | xargs -0 -r {{clang_format}} --dry-run --Werror

fmt-check-lua:
    {{stylua}} --check --glob '**/*.lua' -- game/lua

fmt-check: fmt-check-c fmt-check-lua

tidy:
    {{run_clang_tidy}} -clang-tidy-binary {{clang_tidy}} -p {{build_dir}} -j "$(nproc)" '^.*/src/(mux|btech)/.*[.]c$'

build:
    cmake -S . -B {{build_dir}} -DCMAKE_C_COMPILER=clang-22 -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    cmake --build {{build_dir}} -j "$(nproc)"

test:
    ctest --test-dir {{build_dir}} --output-on-failure

install:
    cmake --install {{build_dir}} --prefix "$PWD/game"

run:
    cd game && ulimit -c unlimited && exec ./stompymux stompymux.toml

build-and-run: build install run

check-and-run: checks install run

docsite:
    npm --prefix docs run build

docsite-serve:
    npm --prefix docs run serve

# Shortcut for letting gdb attach to processes that it didn't start
# This is dangerous to leave on!
ptrace-on:
    sudo sysctl -w kernel.yama.ptrace_scope=0

# Shortcut for disabling cross-process tracing.
ptrace-off:
    sudo sysctl -w kernel.yama.ptrace_scope=1
