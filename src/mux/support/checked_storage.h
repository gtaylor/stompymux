/* Bounds-checked access to dynamically allocated storage. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

typedef bool (*CheckedStorageSentinel)(const void *element);

/* Allocation.
 *
 * The checked_storage_allocate* family is fail-fast: it terminates with a
 * diagnostic rather than returning nullptr, and suits call sites that cannot
 * recover. The checked_storage_try_* family is explicitly nullable and suits
 * call sites that already propagate an allocation failure.
 *
 * The allocation functions return zeroed storage, and a requested size of zero
 * yields a unique non-null one-byte allocation so callers never have to
 * special-case it. The array forms reject a count times element_size
 * multiplication overflow: the try form returns nullptr and the fail-fast form
 * terminates. */
[[nodiscard]] void *checked_storage_allocate(size_t bytes);
[[nodiscard]] void *checked_storage_allocate_array(size_t count,
                                                   size_t element_size);
[[nodiscard]] void *checked_storage_try_allocate(size_t bytes);
[[nodiscard]] void *checked_storage_try_allocate_array(size_t count,
                                                       size_t element_size);
/* Preserves realloc's contract: on failure the original storage is untouched
 * and still owned by the caller, so never assign over the only pointer. */
[[nodiscard]] void *checked_storage_try_reallocate(void *storage, size_t bytes);
void *checked_storage_at(void *storage, size_t count, size_t element_size,
                         size_t index);
const void *checked_storage_at_const(const void *storage, size_t count,
                                     size_t element_size, size_t index);
void *checked_storage_region(void *storage, size_t storage_size, size_t offset,
                             size_t region_size);
const void *checked_storage_region_const(const void *storage,
                                         size_t storage_size, size_t offset,
                                         size_t region_size);
const char *checked_string_suffix(const char *text, size_t offset);
char *checked_mutable_string_suffix(char *text, size_t offset);
size_t checked_storage_sentinel_count(const void *storage, size_t element_size,
                                      size_t maximum_count,
                                      CheckedStorageSentinel is_sentinel);
