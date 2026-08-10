/* array_sort.h - Typed invocation interface for sorting contiguous arrays. */

#pragma once

#include <stddef.h>

typedef struct ArraySortComparison {
  const void *left;
  const void *right;
  void *context;
} ArraySortComparison;

typedef int (*ArraySortComparator)(const ArraySortComparison *comparison);

typedef struct ArraySortRequest {
  void *items;
  size_t count;
  size_t item_size;
  ArraySortComparator compare;
  void *context;
} ArraySortRequest;

typedef struct ArraySearchRequest {
  const void *key;
  const void *items;
  size_t count;
  size_t item_size;
  ArraySortComparator compare;
  void *context;
} ArraySearchRequest;

typedef struct ArraySearchResult {
  bool found;
  size_t index;
} ArraySearchResult;

void array_sort(const ArraySortRequest *request);
ArraySearchResult array_search(const ArraySearchRequest *request);
