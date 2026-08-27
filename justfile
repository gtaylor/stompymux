set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

build_dir := ".build"
fuzz_build_dir := ".build-fuzz"
build_type := env("CMAKE_BUILD_TYPE", "RelWithDebInfo")
enable_asan := env("BTECH_ENABLE_ASAN", "ON")
enable_ubsan := env("BTECH_ENABLE_UBSAN", "ON")
enable_hardening := env("BTECH_ENABLE_HARDENING", "ON")
strict_c23 := env("BTECH_STRICT_C23", "ON")
build_fuzzers := env("BTECH_BUILD_FUZZERS", "OFF")
frame_check_build_dir := ".build-frame-check"
ast_policy_checker_build_dir := ".build-ast-policy-checker"
clang_tidy := env("CLANG_TIDY", "clang-tidy-22")
run_clang_tidy := env("RUN_CLANG_TIDY", "run-clang-tidy-22")
clang := env("CLANG", "clang-22")
clangxx := env("CLANGXX", "clang++-22")
llvm_config := env("LLVM_CONFIG", "llvm-config-22")
clang_format := env("CLANG_FORMAT", "clang-format-22")
stylua := env("STYLUA", "stylua")

default: checks install

ci: ci-build-test ci-analysis

ci-build-test: build test

ci-analysis: check-source-size check-typed-constants check-nullptr check-unsafe-apis check-bounded-copy check-allocation-discipline check-allocation-multiplication check-retired-buffer-apis check-complexity-suppressions fmt-check build frame-check check-ast-policies tidy-check

agent-checks: ci

checks: ci

check-source-size:
    status=0; while IFS= read -r -d '' source; do lines=$(awk 'END { print NR }' "$source"); if (( lines > 800 )); then echo "$source: $lines lines (maximum 800)"; status=1; fi; done < <(find src/mux src/btech -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0); exit "$status"

# Complexity debt must be paid through decomposition rather than hidden from
# the global ratchet. Keep this lexical so suppressions cannot disable it.
check-complexity-suppressions:
    status=0; rg -n --glob '*.[ch]' 'NOLINT[^[:space:]]*\([^)]*readability-function-cognitive-complexity' src || status=$?; if (( status == 0 )); then echo 'Cognitive-complexity suppressions are not permitted in src/.' >&2; exit 1; fi; if (( status != 1 )); then exit "$status"; fi

# Object-like constants in headers and implementations should use typed C23
# objects or enums. String literals remain macros because constexpr pointers
# cannot name them. This lexical gate also keeps examples in comments current.
check-typed-constants:
    found=0; status=0; grep -RInE --include='*.h' --include='*.h.in' '^[[:space:]]*#[[:space:]]*define[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]+[^[:space:]"]' src || status=$?; if (( status == 0 )); then found=1; elif (( status != 1 )); then exit "$status"; fi; status=0; grep -RInE --include='*.c' '^[[:space:]]*#[[:space:]]*define[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]+[^[:space:]"]' src || status=$?; if (( status == 0 )); then found=1; elif (( status != 1 )); then exit "$status"; fi; if (( found )); then echo 'Untyped object-like constant found in guarded sources.' >&2; exit 1; fi

# Named enums use the house-standard signed int representation explicitly.
# Anonymous enums remain the typed-constant idiom and need no underlying type.
check-enum-underlying-type: build ast-policy-checker-build
    mapfile -d '' -t sources < <(find src/mux src/btech -type f -name '*.c' -print0); output=$(mktemp); trap 'rm -f "$output"' EXIT; status=0; {{ast_policy_checker_build_dir}}/ast-policy-checker -p {{build_dir}} --checks=enum-underlying-type "${sources[@]}" >"$output" 2>&1 || status=$?; if (( status != 0 )); then rg -v '^\[[0-9]+/[0-9]+\]' "$output" >&2 || true; exit "$status"; fi

# Persisted unit status words are typed enums. Raw bitwise operations on those
# fields must go through the status-specific accessors so masks cannot cross
# status-word boundaries accidentally. The matcher also catches local aliases
# that retain one of the six status enum types, while ignoring enum constants.
# It keys on operand type, so an explicit cast of both operands to int is a
# deliberate blind spot; that loud escape hatch remains review-visible.
check-status-accessors: build ast-policy-checker-build
    mapfile -d '' -t sources < <(find src/mux src/btech tests -path tests/fixtures -prune -o -type f -name '*.c' -print0); output=$(mktemp); trap 'rm -f "$output"' EXIT; status=0; {{ast_policy_checker_build_dir}}/ast-policy-checker -p {{build_dir}} --checks=status-accessors "${sources[@]}" >"$output" 2>&1 || status=$?; if (( status != 0 )); then rg -v '^\[[0-9]+/[0-9]+\]' "$output" >&2 || true; exit "$status"; fi

# modernize-use-nullptr catches typed null-pointer constants, including bare 0,
# but intentionally skips tokens inside macro expansions. Clang's raw lexer
# closes that gap without mistaking comments or SQL string literals for code.
check-nullptr:
    candidates=$(mktemp); tokens=$(mktemp); trap 'rm -f "$candidates" "$tokens"' EXIT; status=0; rg -l -0 --glob '*.[ch]' --glob '*.h.in' '\bNULL\b' src >"$candidates" || status=$?; if (( status != 0 && status != 1 )); then exit "$status"; fi; if [[ ! -s "$candidates" ]]; then exit 0; fi; xargs -0 -r {{clang}} -x c -std=c23 -Xclang -dump-raw-tokens -fsyntax-only <"$candidates" 2>"$tokens"; if grep -F "raw_identifier 'NULL'" "$tokens"; then echo 'NULL identifier found in src/; use nullptr.' >&2; exit 1; fi

# The unbounded string_copy wrapper and strcpy are banned tree-wide; callers
# use the project's bounded helpers instead.
check-bounded-copy:
    status=0; grep -RInE --include='*.c' --include='*.h' --include='*.h.in' '\b(string_copy|strcpy)[[:space:]]*\(' src || status=$?; if (( status == 0 )); then echo 'Unbounded copy found in src/; use string_copy_bounded.' >&2; exit 1; fi; if (( status != 1 )); then exit "$status"; fi

# This is a straight-line source-order guard for the capture-after-write form
# of the checked-suffix bug. Clang's AST identifies the same pointer variable
# and both direct dereference and subscript writes; local arrays are excluded.
# The source-location pass is intentionally not control-flow analysis: writes
# in loops or mutually exclusive branches can be missed or conservatively
# reported.
check-checked-suffix-order: build ast-policy-checker-build
    mapfile -d '' -t sources < <(find src/mux src/btech -type f -name '*.c' -print0); output=$(mktemp); trap 'rm -f "$output"' EXIT; status=0; {{ast_policy_checker_build_dir}}/ast-policy-checker -p {{build_dir}} --checks=checked-suffix-order "${sources[@]}" >"$output" 2>&1 || status=$?; if (( status != 0 )); then rg -v '^\[[0-9]+/[0-9]+\]' "$output" >&2 || true; exit "$status"; fi

# Allocation goes through the checked_storage family so each site states its
# OOM policy: checked_storage_allocate* fails fast, checked_storage_try_* is
# nullable for callers that recover. checked_storage.c owns the raw calls.
check-allocation-discipline:
    status=0; grep -RInE --include='*.c' --include='*.h' --exclude='checked_storage.c' '\b(malloc|calloc|realloc)[[:space:]]*\(' src || status=$?; if (( status == 0 )); then echo 'Raw allocation found in src/; use the checked_storage API.' >&2; exit 1; fi; if (( status != 1 )); then exit "$status"; fi

# Array allocation sites pass count and element size separately so the checked
# storage API owns multiplication-overflow handling.
check-allocation-multiplication:
    status=0; rg -n -U --glob '*.c' --glob '!**/checked_storage.c' '\bchecked_storage_(try_)?(allocate|reallocate)\([^;]*[[:alnum:]_)]\s*\*\s*[[:alnum:]_(]' src || status=$?; if (( status == 0 )); then echo 'Array-size multiplication found in a scalar checked_storage call; use an array form.' >&2; exit 1; fi; if (( status != 1 )); then exit "$status"; fi

# Buffer ownership uses one size-agnostic release API and one ownership type.
check-retired-buffer-apis:
    status=0; grep -RInE --include='*.c' --include='*.h' --include='*.h.in' '\b(free_lbuf|free_mbuf|free_sbuf|LbufText|lbuf_text_[A-Za-z0-9_]*)\b' src || status=$?; if (( status == 0 )); then echo 'Retired buffer ownership API found in src/; use free_buf and OwnedText.' >&2; exit 1; fi; if (( status != 1 )); then exit "$status"; fi

# Production code must use the project's checked parsing and copy helpers.
check-unsafe-apis:
    status=0; grep -RInE --include='*.c' --include='*.h' --include='*.h.in' '\b(strncpy|strcat|strncat|strlcpy|strlcat|sprintf|vsprintf|gets|alloca|strtok|atoi)[[:space:]]*\(' src || status=$?; if (( status == 0 )); then echo 'Unsafe API usage found in src/.' >&2; exit 1; fi; if (( status != 1 )); then exit "$status"; fi

# Predicate implementations use bool contracts. Callback/status interfaces and
# the two documented non-predicates retain int because their values have wider
# semantics than true/false.
check-boolean-contracts: build ast-policy-checker-build
    mapfile -d '' -t sources < <(find src/mux src/btech -type f -name '*.c' -print0); output=$(mktemp); trap 'rm -f "$output"' EXIT; status=0; {{ast_policy_checker_build_dir}}/ast-policy-checker -p {{build_dir}} --checks=boolean-contracts "${sources[@]}" >"$output" 2>&1 || status=$?; if (( status != 0 )); then rg -v '^\[[0-9]+/[0-9]+\]' "$output" >&2 || true; exit "$status"; fi

ast-policy-checker-build:
    llvm_cmake_dir="$({{llvm_config}} --cmakedir)"; cmake -S tools/ast_policy_checker -B {{ast_policy_checker_build_dir}} -DCMAKE_CXX_COMPILER={{clangxx}} -DLLVM_DIR="$llvm_cmake_dir" -DClang_DIR="${llvm_cmake_dir%/llvm}/clang"
    cmake --build {{ast_policy_checker_build_dir}} -j "$(nproc)"

check-ast-policies: build ast-policy-checker-build
    mapfile -d '' -t sources < <(find src/mux src/btech tests -path tests/fixtures -prune -o -type f -name '*.c' -print0); output=$(mktemp); trap 'rm -f "$output"' EXIT; status=0; {{ast_policy_checker_build_dir}}/ast-policy-checker -p {{build_dir}} --checks=all "${sources[@]}" >"$output" 2>&1 || status=$?; if (( status != 0 )); then rg -v '^\[[0-9]+/[0-9]+\]' "$output" >&2 || true; exit "$status"; fi
    ctest --test-dir {{ast_policy_checker_build_dir}} --output-on-failure

# readability-implicit-bool-conversion also proposes noisy casts in the other
# direction. Enforce only conversions into bool, matching the policy recorded
# at the top of .clang-tidy.
check-boolean-conversions:
    output=$(mktemp); trap 'rm -f "$output"' EXIT; status=0; {{run_clang_tidy}} -clang-tidy-binary {{clang_tidy}} -quiet -p {{build_dir}} -j "$(nproc)" -checks='-*,readability-implicit-bool-conversion' -config="{Checks: '-*,readability-implicit-bool-conversion', WarningsAsErrors: '', HeaderFilterRegex: '^.*/src/(mux|btech)/.*', CheckOptions: {readability-implicit-bool-conversion.AllowIntegerConditions: 'true', readability-implicit-bool-conversion.AllowPointerConditions: 'true'}}" '^.*/src/(mux|btech)/.*[.]c$' >"$output" 2>&1 || status=$?; if (( status != 0 )); then cat "$output" >&2; exit "$status"; fi; if rg -n -- "-> 'bool'" "$output"; then echo 'Implicit conversion into bool found; make the conversion explicit.' >&2; exit 1; fi

fmt-c:
    find src tools/ast_policy_checker -type f \( -name '*.c' -o -name '*.cpp' -o -name '*.h' -o -name '*.h.in' \) -print0 | xargs -0 -r {{clang_format}} -i

fmt-lua:
    {{stylua}} --glob '**/*.lua' -- game/lua

fmt: fmt-c fmt-lua

fmt-check-c:
    find src tools/ast_policy_checker -type f \( -name '*.c' -o -name '*.cpp' -o -name '*.h' -o -name '*.h.in' \) -print0 | xargs -0 -r {{clang_format}} --dry-run --Werror

fmt-check-lua:
    {{stylua}} --check --glob '**/*.lua' -- game/lua

fmt-check: fmt-check-c fmt-check-lua

# Refresh the checked-in LuaLS definitions from the native package bindings.
update-lua-types:
    codex --sandbox workspace-write --ask-for-approval never --cd "$PWD" exec --ephemeral - < tools/update_lua_types_prompt.md

tidy:
    {{run_clang_tidy}} -clang-tidy-binary {{clang_tidy}} -quiet -fix -p {{build_dir}} -j "$(nproc)" '^.*/src/(mux|btech)/.*[.]c$'

tidy-check:
    output=$(mktemp); trap 'rm -f "$output"' EXIT; status=0; {{run_clang_tidy}} -clang-tidy-binary {{clang_tidy}} -quiet -p {{build_dir}} -j "$(nproc)" -checks='readability-implicit-bool-conversion' -warnings-as-errors='*,-readability-implicit-bool-conversion' '^.*/src/(mux|btech)/.*[.]c$' >"$output" 2>&1 || status=$?; if (( status != 0 )); then cat "$output" >&2; exit "$status"; fi; if rg -n -- "-> 'bool'" "$output"; then echo 'Implicit conversion into bool found; make the conversion explicit.' >&2; exit 1; fi

# Report every non-trivial function's measured score, highest first. Header
# diagnostics are deduplicated because clang-tidy can see them from many TUs.
complexity-report: build
    output=$(mktemp); trap 'rm -f "$output"' EXIT; {{run_clang_tidy}} -clang-tidy-binary {{clang_tidy}} -quiet -p {{build_dir}} -j "$(nproc)" -checks='-*,readability-function-cognitive-complexity' -config="{Checks: '-*,readability-function-cognitive-complexity', WarningsAsErrors: '', HeaderFilterRegex: '^.*/src/(mux|btech)/.*', CheckOptions: {readability-function-cognitive-complexity.Threshold: '0', readability-function-cognitive-complexity.DescribeBasicIncrements: 'false'}}" '^.*/src/(mux|btech)/.*[.]c$' >"$output" 2>&1; perl -ne 'if (m{^(.+?):(\d+):\d+: warning: function '\''([^'\'']+)'\'' has cognitive complexity of (\d+)}) { print "$4\t$1:$2\t$3\n" }' "$output" | sort -k1,1nr -k2,2 -u

build:
    cmake -S . -B {{build_dir}} -DCMAKE_C_COMPILER=clang-22 -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON -DBTECH_ENABLE_ASAN={{enable_asan}} -DBTECH_ENABLE_UBSAN={{enable_ubsan}} -DBTECH_ENABLE_HARDENING={{enable_hardening}} -DBTECH_STRICT_C23={{strict_c23}} -DBTECH_BUILD_FUZZERS={{build_fuzzers}}
    cmake --build {{build_dir}} -j "$(nproc)"

# Compile with real code generation and fail if any production frame crosses
# the current 32 KiB ratchet. Keep this separate from the instrumented build:
# sanitizer redzones change frame sizes and are not the metric being gated.
# --fresh prevents a cache copied from another worktree from poisoning CI.
frame-check:
    cmake --fresh -S . -B {{frame_check_build_dir}} -DCMAKE_C_COMPILER=clang-22 -DCMAKE_BUILD_TYPE=RelWithDebInfo -DCMAKE_EXPORT_COMPILE_COMMANDS=ON -DBTECH_ENABLE_ASAN=OFF -DBTECH_ENABLE_UBSAN=OFF -DBTECH_STRICT_C23=ON -DBTECH_BUILD_FUZZERS=OFF -DBTECH_ENABLE_FRAME_SIZE_GATE=ON
    cmake --build {{frame_check_build_dir}} -j "$(nproc)"

test:
    ctest --test-dir {{build_dir}} --output-on-failure -j "$(nproc)"

test-unit:
    ctest --test-dir {{build_dir}} --output-on-failure --no-tests=error -j "$(nproc)" -L '^unit$'

test-unit-topic topic:
    ctest --test-dir {{build_dir}} --output-on-failure --no-tests=error -j "$(nproc)" -L '^unit$' -L '^{{topic}}$'

test-integration:
    ctest --test-dir {{build_dir}} --output-on-failure --no-tests=error -j "$(nproc)" -L '^integration$'

fuzz-build:
    cmake -S . -B {{fuzz_build_dir}} -DCMAKE_C_COMPILER=clang-22 -DCMAKE_BUILD_TYPE=RelWithDebInfo -DBTECH_BUILD_FUZZERS=ON -DBTECH_STRICT_C23=ON -DBTECH_ENABLE_ASAN=ON -DBTECH_ENABLE_UBSAN=ON
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
