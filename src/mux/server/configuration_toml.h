/* configuration_toml.h - TOML configuration file loading and dispatch. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "tomlc17.h"

/* Callback invoked once per resolved (directive, argument-string) pair.
 * Mirrors configuration_set()'s (char *, char *, DbRef) contract minus the
 * player argument, so production code can forward straight into it. */
typedef int (*ConfigDirectiveSetFn)(const char *pname, const char *args,
                                    void *ctx);

/* Walks an already-parsed TOML document (e.g. the result of toml_parse())
 * and invokes set_fn once per directive resolved against the built-in
 * schema mapping. Ignores a root-level "include" key (that's handled by
 * configuration_toml_load(), not here). Unrecognized keys and value-type
 * mismatches are reported to stderr and skipped; they never abort the
 * walk. Returns false only if `root` itself is not a table. */
bool configuration_toml_walk(toml_datum_t root, ConfigDirectiveSetFn set_fn,
                             void *ctx);

/* Loads `path`, resolving any top-level `include = [...]` array (each
 * entry resolved relative to `path`'s own directory, loaded recursively up
 * to a fixed depth, and merged so that `path`'s own keys win over anything
 * pulled in via include), then walks the merged document via
 * configuration_toml_walk(). Returns false with a message written into
 * errbuf on I/O failure, malformed TOML, or excessive include depth. */
bool configuration_toml_load(const char *path, ConfigDirectiveSetFn set_fn,
                             void *ctx, char *errbuf, size_t errbuf_size);
