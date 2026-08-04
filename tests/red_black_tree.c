#include "mux/support/red_black_tree.h"

#include <stdbool.h>
#include <stddef.h>

typedef struct WalkResult {
  int values[16];
  size_t count;
} WalkResult;

static int compare_ints(void *left, void *right, void *context) {
  const int a = *(int *)left;
  const int b = *(int *)right;

  (void)context;
  return (a > b) - (a < b);
}

static int collect_walk(void *key, void *data, int depth, void *context) {
  WalkResult *result = context;

  (void)key;
  (void)depth;
  result->values[result->count++] = *(int *)data;
  return 1;
}

static void count_release(void *key, void *data, void *context) {
  size_t *count = context;

  (void)key;
  (void)data;
  (*count)++;
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
      red_black_tree_find(tree, &keys[0]) != nullptr ||
      red_black_tree_exists(tree, &keys[0]) ||
      red_black_tree_search(tree, SEARCH_FIRST, nullptr) != nullptr ||
      red_black_tree_index(tree, 0) != nullptr ||
      red_black_tree_delete(tree, &keys[0]) != nullptr)
    return 1;

  for (size_t index = 0; index < 7; index++)
    red_black_tree_insert(tree, &keys[index], &values[index]);
  if (red_black_tree_size(tree) != 7)
    return 1;

  red_black_tree_insert(tree, &keys[4], &replacement);
  if (red_black_tree_size(tree) != 7 ||
      red_black_tree_find(tree, &keys[4]) != &replacement ||
      !red_black_tree_exists(tree, &keys[4]))
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
    if (walk.values[index] != expected)
      return 1;
  }

  int deletion_order[] = {1, 2, 6, 4};
  for (size_t index = 0; index < 4; index++) {
    int key = deletion_order[index];
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
  return 0;
}
