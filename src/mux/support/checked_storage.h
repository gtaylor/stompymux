/* Bounds-checked access to dynamically allocated storage. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

typedef bool (*CheckedStorageSentinel)(const void *element);

[[nodiscard]] void *checked_storage_allocate(size_t bytes);
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
