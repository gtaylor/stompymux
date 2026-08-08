/* Bounds-checked access to dynamically allocated storage. */

#include "mux/support/checked_storage.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

static size_t checked_storage_offset(const void *storage, size_t count,
                                     size_t element_size, size_t index) {
  if (storage == nullptr || element_size == 0 || index >= count ||
      index > SIZE_MAX / element_size) {
    abort();
  }
  return index * element_size;
}

void *checked_storage_at(void *storage, size_t count, size_t element_size,
                         size_t index) {
  const size_t offset =
      checked_storage_offset(storage, count, element_size, index);

  /* Clang's C warning does not recognize the checks above. Keep the one
   * unavoidable address calculation inside this audited storage boundary. */
#pragma clang unsafe_buffer_usage begin
  return (unsigned char *)storage + offset;
#pragma clang unsafe_buffer_usage end
}

const void *checked_storage_at_const(const void *storage, size_t count,
                                     size_t element_size, size_t index) {
  const size_t offset =
      checked_storage_offset(storage, count, element_size, index);

#pragma clang unsafe_buffer_usage begin
  return (const unsigned char *)storage + offset;
#pragma clang unsafe_buffer_usage end
}

void *checked_storage_region(void *storage, size_t storage_size, size_t offset,
                             size_t region_size) {
  if (region_size > storage_size || offset > storage_size - region_size)
    abort();
  if (region_size == 0)
    return storage;
  return checked_storage_at(storage, storage_size, sizeof(unsigned char),
                            offset);
}

const void *checked_storage_region_const(const void *storage,
                                         size_t storage_size, size_t offset,
                                         size_t region_size) {
  if (region_size > storage_size || offset > storage_size - region_size)
    abort();
  if (region_size == 0)
    return storage;
  return checked_storage_at_const(storage, storage_size, sizeof(unsigned char),
                                  offset);
}

const char *checked_string_suffix(const char *text, size_t offset) {
  return checked_storage_at_const(text, strlen(text) + 1, sizeof(char), offset);
}

char *checked_mutable_string_suffix(char *text, size_t offset) {
  return checked_storage_at(text, strlen(text) + 1, sizeof(char), offset);
}

size_t checked_storage_sentinel_count(const void *storage, size_t element_size,
                                      size_t maximum_count,
                                      CheckedStorageSentinel is_sentinel) {
  if (!storage || !element_size || !is_sentinel)
    abort();
  for (size_t index = 0; index < maximum_count; index++) {
    const void *element =
        checked_storage_at_const(storage, maximum_count, element_size, index);
    if (is_sentinel(element))
      return index;
  }
  abort();
}
