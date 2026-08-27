#include "mux/support/red_black_tree.h"
#include "mux/support/checked_storage.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct WalkResult {
  int values[16];
  size_t count;
} WalkResult;

typedef struct IntegerReleaseResult {
  intptr_t sum;
  size_t count;
} IntegerReleaseResult;

static int *int_slot(int *values, size_t count, size_t index) {
  return checked_storage_at(values, count, sizeof(*values), index);
}

static int compare_ints(const RedBlackTreeCompareCall *call) {
  const void *left = call->lhs;
  const void *right = call->rhs;
  [[maybe_unused]] void *context = call->context;
  const int a = *(const int *)left;
  const int b = *(const int *)right;

  return (a > b) - (a < b);
}

static int compare_intptrs(const RedBlackTreeCompareCall *call) {
  const intptr_t left = *(const intptr_t *)call->lhs;
  const intptr_t right = *(const intptr_t *)call->rhs;
  return (left > right) - (left < right);
}

static bool collect_walk(const RedBlackTreeVisitCall *call) {
  void *key [[maybe_unused]] = call->key;
  void *data = call->data;
  int depth [[maybe_unused]] = call->depth;
  void *context = call->context;
  WalkResult *result = context;

  *int_slot(result->values, 16, result->count++) = *(int *)data;
  return 1;
}

static void count_release(const RedBlackTreeReleaseCall *call) {
  void *key [[maybe_unused]] = call->key;
  void *data [[maybe_unused]] = call->data;
  void *context = call->context;
  size_t *count = context;

  (*count)++;
}

static void collect_integer_release(const RedBlackTreeReleaseCall *call) {
  IntegerReleaseResult *result = call->context;
  result->sum += *(const intptr_t *)call->key;
  result->count++;
}

static bool expect_value(RedBlackTree tree, int method, int *key,
                         int expected) {
  int *value = red_black_tree_search(tree, method, key);

  return value != nullptr && *value == expected;
}

int main(void) {
  int keys[] = {4, 2, 6, 1, 3, 5, 7};
  int values[] = {4, 2, 6, 1, 3, 5, 7};
  int replacement = 30;
  WalkResult walk = {0};
  size_t released = 0;
  RedBlackTree tree = red_black_tree_init(compare_ints, nullptr);

  if (tree == nullptr || red_black_tree_size(tree) != 0 ||
      red_black_tree_find(tree, int_slot(keys, 7, 0)) != nullptr ||
      red_black_tree_exists(tree, int_slot(keys, 7, 0)) ||
      red_black_tree_search(tree, SEARCH_FIRST, nullptr) != nullptr ||
      red_black_tree_index(tree, 0) != nullptr ||
      red_black_tree_delete(tree, int_slot(keys, 7, 0)) != nullptr)
    return 1;

  for (size_t index = 0; index < 7; index++)
    red_black_tree_insert(tree, int_slot(keys, 7, index),
                          int_slot(values, 7, index));
  if (red_black_tree_size(tree) != 7)
    return 1;

  red_black_tree_insert(tree, int_slot(keys, 7, 4), &replacement);
  if (red_black_tree_size(tree) != 7 ||
      red_black_tree_find(tree, int_slot(keys, 7, 4)) != &replacement ||
      !red_black_tree_exists(tree, int_slot(keys, 7, 4)))
    return 1;

  int below = 0;
  int middle = 4;
  int above = 8;
  if (!expect_value(tree, SEARCH_FIRST, nullptr, 1) ||
      !expect_value(tree, SEARCH_LAST, nullptr, 7) ||
      !expect_value(tree, SEARCH_EQUAL, &middle, 4) ||
      !expect_value(tree, SEARCH_GTEQ, &below, 1) ||
      !expect_value(tree, SEARCH_LTEQ, &above, 7) ||
      !expect_value(tree, SEARCH_GT, &middle, 5) ||
      !expect_value(tree, SEARCH_LT, &middle, 30) ||
      !expect_value(tree, SEARCH_NEXT, &middle, 5) ||
      !expect_value(tree, SEARCH_PREV, &middle, 30))
    return 1;

  for (int index = 0; index < 7; index++) {
    int *value = red_black_tree_index(tree, index);
    int expected = index == 2 ? replacement : index + 1;

    if (value == nullptr || *value != expected)
      return 1;
  }
  if (!red_black_tree_walk(tree, WALK_INORDER, collect_walk, &walk) ||
      walk.count != 7)
    return 1;
  for (size_t index = 0; index < walk.count; index++) {
    int expected = index == 2 ? replacement : (int)index + 1;
    if (*int_slot(walk.values, 16, index) != expected)
      return 1;
  }

  int deletion_order[] = {1, 2, 6, 4};
  for (size_t index = 0; index < 4; index++) {
    int key = *int_slot(deletion_order, 4, index);
    if (red_black_tree_delete(tree, &key) == nullptr ||
        red_black_tree_exists(tree, &key))
      return 1;
  }
  if (red_black_tree_size(tree) != 3)
    return 1;

  red_black_tree_release(tree, count_release, &released);
  if (released != 3)
    return 1;

  tree = red_black_tree_init(compare_ints, nullptr);
  if (tree == nullptr)
    return 1;
  red_black_tree_destroy(tree);

  int minimum = 1;
  int zero = 2;
  int maximum = 3;
  int replacement_zero = 4;
  IntegerReleaseResult integer_release = {};
  tree = red_black_tree_init(compare_intptrs, nullptr);
  if (tree == nullptr)
    return 1;
  red_black_tree_insert_integer(tree, INTPTR_MIN, &minimum);
  red_black_tree_insert_integer(tree, 0, &zero);
  red_black_tree_insert_integer(tree, INTPTR_MAX, &maximum);
  red_black_tree_insert_integer(tree, 0, &replacement_zero);
  if (red_black_tree_find_integer(tree, INTPTR_MIN) != &minimum ||
      red_black_tree_find_integer(tree, 0) != &replacement_zero ||
      red_black_tree_find_integer(tree, INTPTR_MAX) != &maximum ||
      !red_black_tree_exists_integer(tree, INTPTR_MIN) ||
      red_black_tree_size(tree) != 3 ||
      red_black_tree_delete_integer(tree, 0) != &replacement_zero ||
      red_black_tree_exists_integer(tree, 0))
    return 1;
  red_black_tree_release(tree, collect_integer_release, &integer_release);
  if (integer_release.count != 2 || integer_release.sum != -1)
    return 1;

  tree = red_black_tree_init(compare_intptrs, nullptr);
  if (tree == nullptr)
    return 1;
  red_black_tree_insert_integer(tree, 1, &minimum);
  red_black_tree_insert_integer(tree, 2, &zero);
  red_black_tree_insert_integer(tree, 3, &maximum);
  if (red_black_tree_delete_integer(tree, 1) != &minimum ||
      red_black_tree_delete_integer(tree, 2) != &zero ||
      red_black_tree_delete_integer(tree, 3) != &maximum ||
      red_black_tree_size(tree) != 0)
    return 1;
  red_black_tree_destroy(tree);
  return 0;
}
