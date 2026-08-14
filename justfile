set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

build_dir := ".build"
fuzz_build_dir := ".build-fuzz"
build_type := env("CMAKE_BUILD_TYPE", "RelWithDebInfo")
enable_asan := env("BTECH_ENABLE_ASAN", "ON")
enable_ubsan := env("BTECH_ENABLE_UBSAN", "ON")
strict_c23 := env("BTMUX_STRICT_C23", "ON")
build_fuzzers := env("BTMUX_BUILD_FUZZERS", "OFF")
clang_tidy := env("CLANG_TIDY", "clang-tidy-22")
run_clang_tidy := env("RUN_CLANG_TIDY", "run-clang-tidy-22")
clang_format := env("CLANG_FORMAT", "clang-format-22")
stylua := env("STYLUA", "stylua")

default: checks install

ci: check-source-size fmt-check build test tidy-check

agent-checks: ci

checks: ci

check-source-size:
    status=0; while IFS= read -r -d '' source; do lines=$(awk 'END { print NR }' "$source"); if (( lines > 800 )); then echo "$source: $lines lines (maximum 800)"; status=1; fi; done < <(find src/mux src/btech -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0); exit "$status"

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
    {{run_clang_tidy}} -clang-tidy-binary {{clang_tidy}} -quiet -fix -p {{build_dir}} -j "$(nproc)" '^.*/src/(mux|btech)/.*[.]c$'

tidy-check:
    {{run_clang_tidy}} -clang-tidy-binary {{clang_tidy}} -quiet -p {{build_dir}} -j "$(nproc)" '^.*/src/(mux|btech)/.*[.]c$'

build:
    cmake -S . -B {{build_dir}} -DCMAKE_C_COMPILER=clang-22 -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON -DBTECH_ENABLE_ASAN={{enable_asan}} -DBTECH_ENABLE_UBSAN={{enable_ubsan}} -DBTMUX_STRICT_C23={{strict_c23}} -DBTMUX_BUILD_FUZZERS={{build_fuzzers}}
    cmake --build {{build_dir}} -j "$(nproc)"

test:
    ctest --test-dir {{build_dir}} --output-on-failure -j "$(nproc)"

test-unit:
    ctest --test-dir {{build_dir}} --output-on-failure --no-tests=error -j "$(nproc)" -L '^unit$'

test-unit-topic topic:
    ctest --test-dir {{build_dir}} --output-on-failure --no-tests=error -j "$(nproc)" -L '^unit$' -L '^{{topic}}$'

test-integration:
    ctest --test-dir {{build_dir}} --output-on-failure --no-tests=error -j "$(nproc)" -L '^integration$'

fuzz-build:
    cmake -S . -B {{fuzz_build_dir}} -DCMAKE_C_COMPILER=clang-22 -DCMAKE_BUILD_TYPE=RelWithDebInfo -DBTMUX_BUILD_FUZZERS=ON -DBTMUX_STRICT_C23=ON -DBTECH_ENABLE_ASAN=ON -DBTECH_ENABLE_UBSAN=ON
    cmake --build {{fuzz_build_dir}} --target wild_fuzzer styled_text_fuzzer -j "$(nproc)"

fuzz-smoke: fuzz-build
    wild_corpus=$(mktemp -d); styled_text_corpus=$(mktemp -d); trap 'rm -rf -- "$wild_corpus" "$styled_text_corpus"' EXIT; cp -a tests/fuzz/corpus/wild/. "$wild_corpus"; cp -a tests/fuzz/corpus/styled_text/. "$styled_text_corpus"; xxd -r -p tests/fuzz/corpus/styled_text_invalid_utf8.hex "$styled_text_corpus/invalid_utf8"; {{fuzz_build_dir}}/tests/wild_fuzzer -runs=100 "$wild_corpus"; {{fuzz_build_dir}}/tests/styled_text_fuzzer -runs=100 "$styled_text_corpus"

install:
    cmake --install {{build_dir}} --prefix "$PWD/game"

update-submodules:
    git submodule update --init --recursive

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
