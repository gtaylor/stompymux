/** @file
 * Bounds-checked access to dynamically allocated storage.
 */
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
/** Allocates checked storage. @param[in] bytes Number of bytes. */

[[nodiscard]] void *checked_storage_allocate(size_t bytes);
/** Allocates a checked storage array. @param[in] count Number of elements.
 * @param[in] element_size Size of each element in bytes. */

[[nodiscard]] void *checked_storage_allocate_array(size_t count,
                                                   size_t element_size);
/** Allocates checked storage try. @param[in] bytes Number of bytes. */

[[nodiscard]] void *checked_storage_try_allocate(size_t bytes);
/** Attempts to allocate a checked storage array. @param[in] count Number of
 * elements. @param[in] element_size Size of each element in bytes. */

[[nodiscard]] void *checked_storage_try_allocate_array(size_t count,
                                                       size_t element_size);
/* Preserves realloc's contract: on failure the original storage is untouched
 * and still owned by the caller, so never assign over the only pointer. The
 * array form rejects count times element_size overflow. Neither form zeroes a
 * grown region. */
/** Resizes checked storage try. @param[in] storage Storage. @param[in] bytes
 * Number of bytes. */

[[nodiscard]] void *checked_storage_try_reallocate(void *storage, size_t bytes);
/** Attempts to resize a checked storage array. @param[in] storage Storage.
 * @param[in] count Number of elements. @param[in] element_size Size of each
 * element in bytes. */

[[nodiscard]] void *checked_storage_try_reallocate_array(void *storage,
                                                         size_t count,
                                                         size_t element_size);
/** Returns checked storage at. @param[in] storage Storage. @param[in] count
 * Number of elements. @param[in] element_size Size of each element in bytes.
 * @param[in] index Zero-based index. */

void *checked_storage_at(void *storage, size_t count, size_t element_size,
                         size_t index);
/** Returns checked storage at. @param[in] storage Storage. @param[in] count
 * Number of elements. @param[in] element_size Size of each element in bytes.
 * @param[in] index Zero-based index. */

const void *checked_storage_at_const(const void *storage, size_t count,
                                     size_t element_size, size_t index);
/** Returns checked storage region. @param[in] storage Storage. @param[in]
 * storage_size Size of storage in bytes. @param[in] offset Byte offset.
 * @param[in] region_size Size of region in bytes. */

void *checked_storage_region(void *storage, size_t storage_size, size_t offset,
                             size_t region_size);
/** Returns checked storage region. @param[in] storage Storage. @param[in]
 * storage_size Size of storage in bytes. @param[in] offset Byte offset.
 * @param[in] region_size Size of region in bytes. */

const void *checked_storage_region_const(const void *storage,
                                         size_t storage_size, size_t offset,
                                         size_t region_size);
/** Returns checked string suffix. @param[in] text Text to process. @param[in]
 * offset Byte offset. */

const char *checked_string_suffix(const char *text, size_t offset);
/** Returns checked mutable string suffix. @param[in] text Text to process.
 * @param[in] offset Byte offset. */

char *checked_mutable_string_suffix(char *text, size_t offset);
/** Counts checked storage sentinel. @param[in] storage Storage. @param[in]
 * element_size Size of each element in bytes. @param[in] maximum_count Maximum
 * number of elements to inspect. @param[in] is_sentinel Predicate used to
 * identify sentinel elements. */

size_t checked_storage_sentinel_count(const void *storage, size_t element_size,
                                      size_t maximum_count,
                                      CheckedStorageSentinel is_sentinel);
