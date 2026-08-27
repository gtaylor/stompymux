/* configuration_toml_load.c - TOML file loading and include resolution. */

#include "mux/server/configuration_toml.h"

#include <stdio.h>
#include <string.h>

#include "mux/support/checked_storage.h"
#include "tomlc17.h"

static constexpr int CONFIG_TOML_MAX_INCLUDE_DEPTH = 8;

static toml_datum_t configuration_toml_include_at(toml_datum_t array,
                                                  size_t index) {
  return *(const toml_datum_t *)checked_storage_at_const(
      array.u.arr.elem, (size_t)array.u.arr.size, sizeof(*array.u.arr.elem),
      index);
}

static void configuration_toml_dirname(const char *path, char *out,
                                       size_t out_size) {
  const char *slash;
  size_t len;

  if (out_size == 0)
    return;
  slash = strrchr(path, '/');
  if (slash == nullptr) {
    out[0] = '\0';
    return;
  }
  len = (size_t)(slash - path);
  if (len >= out_size)
    len = out_size - 1;
  memcpy(out, path, len);
  *(char *)checked_storage_at(out, out_size, sizeof(char), len) = '\0';
}

static void configuration_toml_resolve(const char *base_dir, const char *rel,
                                       char *out, size_t out_size) {
  if (rel[0] == '/' || base_dir[0] == '\0')
    (void)snprintf(out, out_size, "%s", rel);
  else
    (void)snprintf(out, out_size, "%s/%s", base_dir, rel);
}

// NOLINTNEXTLINE(misc-no-recursion)
static bool configuration_toml_load_merged(const char *path, int depth,
                                           toml_result_t *out, char *errbuf,
                                           size_t errbuf_size) {
  toml_result_t self;
  toml_datum_t include_array;
  toml_result_t acc = {};
  bool have_acc = false;
  char base_dir[512];

  if (depth > CONFIG_TOML_MAX_INCLUDE_DEPTH) {
    (void)snprintf(errbuf, errbuf_size,
                   "include depth exceeded while loading '%s'", path);
    return false;
  }

  self = toml_parse_file_ex(path);
  if (!self.ok) {
    (void)snprintf(errbuf, errbuf_size, "%s", self.errmsg);
    toml_free(self);
    return false;
  }

  include_array = toml_get(self.toptab, "include");
  if (include_array.type != TOML_ARRAY) {
    *out = self;
    return true;
  }

  configuration_toml_dirname(path, base_dir, sizeof(base_dir));
  for (int i = 0; i < include_array.u.arr.size; i++) {
    toml_datum_t entry =
        configuration_toml_include_at(include_array, (size_t)i);
    char resolved[768];
    toml_result_t inc_result;

    if (entry.type != TOML_STRING) {
      (void)snprintf(errbuf, errbuf_size,
                     "'include' entries must be strings (in '%s')", path);
      if (have_acc)
        toml_free(acc);
      toml_free(self);
      return false;
    }
    configuration_toml_resolve(base_dir, entry.u.s, resolved, sizeof(resolved));
    if (!configuration_toml_load_merged(resolved, depth + 1, &inc_result,
                                        errbuf, errbuf_size)) {
      if (have_acc)
        toml_free(acc);
      toml_free(self);
      return false;
    }
    if (!have_acc) {
      acc = inc_result;
      have_acc = true;
    } else {
      toml_result_t merged = toml_merge(&acc, &inc_result);

      toml_free(acc);
      toml_free(inc_result);
      acc = merged;
    }
  }

  if (have_acc) {
    toml_result_t merged = toml_merge(&acc, &self);

    toml_free(acc);
    toml_free(self);
    *out = merged;
  } else {
    *out = self;
  }
  return true;
}

bool configuration_toml_load(const char *path, ConfigDirectiveSetFn set_fn,
                             void *ctx, char *errbuf, size_t errbuf_size) {
  toml_result_t result;

  errbuf[0] = '\0';
  if (!configuration_toml_load_merged(path, 0, &result, errbuf, errbuf_size))
    return false;
  configuration_toml_walk(result.toptab, set_fn, ctx);
  toml_free(result);
  return true;
}
