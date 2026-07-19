set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

build_dir := "build"
build_type := env_var_or_default("CMAKE_BUILD_TYPE", "RelWithDebInfo")
# clang-tidy must understand C23 `constexpr`, which landed in Clang 19; the
# bare `clang-tidy` on this system still resolves to v18.
clang_tidy := env_var_or_default("CLANG_TIDY", "clang-tidy-20")
clang_format := env_var_or_default("CLANG_FORMAT", "clang-format-20")

default: lint build test install

fmt:
    find src -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0 | xargs -0 -r {{clang_format}} -i

fmt-check:
    find src -type f \( -name '*.c' -o -name '*.h' -o -name '*.h.in' \) -print0 | xargs -0 -r {{clang_format}} --dry-run --Werror

tidy:
    cmake -S . -B {{build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    find src -type f -name '*.c' -print0 | xargs -0 -r -n1 {{clang_tidy}} -p {{build_dir}}

lint: fmt lint-legacy-context tidy

# Core and BTech code must use scoped contexts rather than legacy aliases.
lint-legacy-context:
    count="$(rg -l '\b(mudstate|mudconf)\b' src/mux src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -eq 0 || { echo "legacy context used by $count source files" >&2; exit 1; }
    count="$(rg -o 'btech_context_(current_unthreaded|bind_unthreaded)|BTECH_EVALUATION_CONTEXT|\bDOCHECK(N|0|1)?\(' src/mux src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -eq 0 || { echo "legacy ambient BTech context usage returned ($count occurrences)" >&2; exit 1; }
    count="$(rg -o '\b(getMap|getMech|WhichSpecial|FindObjectsData|IsMech|IsAuto|IsMap)\(' src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -eq 0 || { echo "legacy BTech object lookup APIs used $count times" >&2; exit 1; }
    count="$(rg -o '\b(Number|Roll)\(|\b(init_genrand|genrand_[a-z0-9_]+)\(|\brollstat\b|mt19937ar\.h' src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -eq 0 || { echo "legacy ambient BTech random API used $count times" >&2; exit 1; }
    count="$(rg -o '\bMissileHitTable\b|\bmissile_hit_table_struct\b' src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -eq 0 || { echo "legacy mutable missile-hit table used $count times" >&2; exit 1; }
    count="$(rg -o 'MechWeapons\[[^]]+\]\.(vrt|battlevalue)[[:space:]]*=' src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -eq 0 || { echo "canonical weapon definitions mutated $count times" >&2; exit 1; }
    count="$(rg -o 'btech_context_evaluation\(' src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -le 506 || { echo "ambient BTech evaluation accessor grew to $count uses (maximum 506)" >&2; exit 1; }
    count="$(rg -o 'LuaRuntime[[:space:]]*\*\*|\bspath_map\b' src/mux src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -eq 0 || { echo "removed raw owner-slot or path-map global returned ($count occurrences)" >&2; exit 1; }
    count="$(rg -o '\b(MechIDS|GetMechID|GetMechToMechID|GetMechToMechID_base|UnitPartsList|TraceLOS|ShortArmorSectionString|getStatusString|AI_Info|auto_show_command|silly_get_uptime_to_string|silly_atr_get_from|my_dump_flag|sensor_mode_name|add_color|GetLRSMech|LRSTerrain|LRSElevation|get_lrshexstr|MakeMapText|BuildBitString|BuildBitString2|BuildBitString2WithDelim|BuildBitString3|PrintArmorDamageString|ArmorKeyInfo|RetrieveValue)\(' src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -eq 0 || { echo "legacy static-buffer helper returned ($count occurrences)" >&2; exit 1; }
    count="$(rg -o 'static char (buf|buf2|buffer)\[[^]]+\]' src/btech/src/glue.scode.c src/btech/src/btech/mech.sensor.c src/btech/src/btech/mech.status.c src/btech/src/btech/template.c src/btech/src/btech/mech.advanced.c | wc -l || true)"; test "$count" -eq 0 || { echo "BTech callback or formatter static buffer returned ($count occurrences)" >&2; exit 1; }
    count="$(rg -o 'char[[:space:]]*\*[[:space:]]*(structure_name|center_string)\(' src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -eq 0 || { echo "caller-owned formatter regressed to a pointer return ($count occurrences)" >&2; exit 1; }
    count="$(rg -o '\b(cachemech|cacheref)\b' src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -eq 0 || { echo "ambient reference-mech cache returned ($count occurrences)" >&2; exit 1; }
    count="$(rg -o '\bDestroySpecialObjects\b|free\(server->configuration\)' src/mux src/btech --glob '*.c' --glob '*.h' | wc -l || true)"; test "$count" -eq 0 || { echo "legacy composition-root teardown returned ($count occurrences)" >&2; exit 1; }

lint-changes:
    git diff --name-only -z --diff-filter=ACMR HEAD -- src | while IFS= read -r -d '' file; do case "$file" in *.c|*.h|*.h.in) {{clang_format}} -i "$file" ;; esac; done
    just lint-legacy-context
    cmake -S . -B {{build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    git diff --name-only -z --diff-filter=ACMR HEAD -- src | while IFS= read -r -d '' file; do case "$file" in *.c) {{clang_tidy}} -p {{build_dir}} "$file" ;; esac; done

build:
    cmake -S . -B {{build_dir}} -DCMAKE_BUILD_TYPE={{build_type}} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    cmake --build {{build_dir}} -j "$(nproc)"

test:
    ctest --test-dir {{build_dir}} --output-on-failure

install: build
    cmake --install {{build_dir}} --prefix "$PWD/game"

run:
    cd game && ulimit -c unlimited && exec ./stompymux stompymux.toml

docsite:
    npm --prefix docs run build

docsite-serve:
    npm --prefix docs run serve
