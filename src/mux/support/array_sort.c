/* array_sort.c - In-place sorting for contiguous project arrays. */

#include "mux/support/array_sort.h"

#include <stddef.h>

#include "mux/support/checked_storage.h"

static void *array_sort_item(void *items, const ArraySortRequest *request,
                             size_t index) {
  return checked_storage_at(items, request->count, request->item_size, index);
}

static const void *array_sort_const_item(const void *items,
                                         const ArraySortRequest *request,
                                         size_t index) {
  return checked_storage_at_const(items, request->count, request->item_size,
                                  index);
}

typedef struct ArraySortIndexes {
  size_t left;
  size_t right;
} ArraySortIndexes;

static int array_sort_compare(const ArraySortRequest *request,
                              ArraySortIndexes indexes) {
  return request->compare(&(ArraySortComparison){
      .left = array_sort_const_item(request->items, request, indexes.left),
      .right = array_sort_const_item(request->items, request, indexes.right),
      .context = request->context});
}

static void array_sort_swap(const ArraySortRequest *request,
                            ArraySortIndexes indexes) {
  unsigned char *left_item =
      array_sort_item(request->items, request, indexes.left);
  unsigned char *right_item =
      array_sort_item(request->items, request, indexes.right);

  for (size_t byte = 0; byte < request->item_size; byte++) {
    unsigned char *left_byte = checked_storage_at(left_item, request->item_size,
                                                  sizeof(*left_item), byte);
    unsigned char *right_byte = checked_storage_at(
        right_item, request->item_size, sizeof(*right_item), byte);
    unsigned char value = *left_byte;
    *left_byte = *right_byte;
    *right_byte = value;
  }
}

static void array_sort_sift(const ArraySortRequest *request,
                            ArraySortIndexes range) {
  size_t root = range.left;
  size_t end = range.right;
  while (root < end && root <= (end - 1) / 2) {
    size_t child = root * 2 + 1;
    if (child < end &&
        array_sort_compare(
            request, (ArraySortIndexes){.left = child, .right = child + 1}) < 0)
      child++;
    if (array_sort_compare(
            request, (ArraySortIndexes){.left = root, .right = child}) >= 0)
      return;
    array_sort_swap(request, (ArraySortIndexes){.left = root, .right = child});
    root = child;
  }
}

void array_sort(const ArraySortRequest *request) {
  if (!request || !request->items || !request->compare ||
      request->item_size == 0 || request->count < 2)
    return;

  for (size_t start = request->count / 2; start > 0; start--)
    array_sort_sift(request, (ArraySortIndexes){.left = start - 1,
                                                .right = request->count - 1});
  for (size_t end = request->count - 1; end > 0; end--) {
    array_sort_swap(request, (ArraySortIndexes){.right = end});
    array_sort_sift(request, (ArraySortIndexes){.right = end - 1});
  }
}

ArraySearchResult array_search(const ArraySearchRequest *request) {
  if (!request || !request->key || !request->items || !request->compare ||
      request->item_size == 0)
    return (ArraySearchResult){0};

  size_t first = 0;
  size_t remaining = request->count;
  while (remaining > 0) {
    size_t offset = remaining / 2;
    size_t index = first + offset;
    const void *item = checked_storage_at_const(request->items, request->count,
                                                request->item_size, index);
    int ordering = request->compare(&(ArraySortComparison){
        .left = request->key, .right = item, .context = request->context});

    if (ordering == 0)
      return (ArraySearchResult){.found = true, .index = index};
    if (ordering < 0) {
      remaining = offset;
    } else {
      first = index + 1;
      remaining -= offset + 1;
    }
  }
  return (ArraySearchResult){0};
}
