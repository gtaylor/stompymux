/* red_black_tree.c - Red-black tree ownership and ordered queries. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/support/checked_storage.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/red_black_tree_internal.h"

[[noreturn]] static void red_black_tree_fail(const char *message) {
  (void)fprintf(stderr, "Red-black tree invariant failure: %s\n", message);
  abort();
}

RedBlackTree red_black_tree_init(RedBlackTreeCompare compare, void *context) {
  RedBlackTree temp;

  temp = checked_storage_try_allocate(sizeof(struct RedBlackTreeHead));
  if (temp == nullptr)
    return nullptr;
  memset(temp, 0, sizeof(struct RedBlackTreeHead));
  temp->compare = compare;
  temp->context = context;
  temp->size = 0;
  return temp;
}

static RbtreeNode *red_black_tree_find_minimum(RbtreeNode *node) {
  RbtreeNode *child;
  child = node;
  if (!node)
    return nullptr;
  while (child->left != nullptr)
    child = child->left;
  return child;
}

static RbtreeNode *red_black_tree_find_maximum(RbtreeNode *node) {
  RbtreeNode *child;
  child = node;
  if (!node)
    return nullptr;
  while (child->right != nullptr)
    child = child->right;
  return child;
}

RbtreeNode *red_black_tree_find_successor_node(RbtreeNode *node) {
  RbtreeNode *child;
  RbtreeNode *parent;
  if (!node)
    return nullptr;
  if (node->right != nullptr) {
    child = node->right;
    while (child->left != nullptr) {
      child = child->left;
    }
    return child;
  }
  child = node;
  parent = node->parent;
  while (parent != nullptr && child == parent->right) {
    child = parent;
    parent = child->parent;
  }
  return parent;
}

static RbtreeNode *red_black_tree_find_predecessor_node(RbtreeNode *node) {
  RbtreeNode *child;
  RbtreeNode *parent;
  if (!node)
    return nullptr;
  if (node->left != nullptr) {
    child = node->left;
    while (child->right != nullptr)
      child = child->right;
    return child;
  }
  child = node;
  parent = node->parent;
  while (parent != nullptr && child == parent->left) {
    child = parent;
    parent = parent->parent;
  }
  return parent;
}

void red_black_tree_release(RedBlackTree bt, RedBlackTreeRelease release,
                            void *context) {
  RbtreeNode *node;
  RbtreeNode *parent;
  node = bt->head;

  if (bt->head) {
    while (node != nullptr) {
      if (node->left != nullptr) {
        node = node->left;
        continue;
      }
      if (node->right != nullptr) {
        node = node->right;
        continue;
      }
      parent = node->parent;
      if (parent && parent->left == node) {
        parent->left = nullptr;
      } else if (parent && parent->right == node) {
        parent->right = nullptr;
      } else if (parent) {
        (void)fprintf(stderr, "serious braindamage.\n");
        exit(1);
      }
      release(&(RedBlackTreeReleaseCall){
          .key = node->key,
          .data = node->data,
          .context = context,
      });
      free(node);
      node = parent;
    }
  }
  free(bt);
}

void red_black_tree_destroy(RedBlackTree bt) {
  RbtreeNode *node;
  RbtreeNode *parent;
  node = bt->head;

  if (bt->head) {
    while (node != nullptr) {
      if (node->left != nullptr) {
        node = node->left;
        continue;
      }
      if (node->right != nullptr) {
        node = node->right;
        continue;
      }
      parent = node->parent;
      if (parent && parent->left == node) {
        parent->left = nullptr;
      } else if (parent && parent->right == node) {
        parent->right = nullptr;
      } else if (parent) {
        (void)fprintf(stderr, "serious braindamage.\n");
        exit(1);
      }
      free(node);
      node = parent;
    }
  }
  free(bt);
}

void *red_black_tree_find(RedBlackTree bt, const void *key) {
  RbtreeNode *node;
  int compare_result;

  if (!bt->head) {
    return nullptr;
  }
  node = bt->head;
  while (node != nullptr) {
    compare_result = bt->compare(&(RedBlackTreeCompareCall){
        .lhs = key,
        .rhs = node->key,
        .context = bt->context,
    });
    if (compare_result == 0) {
      return node->data;
    }
    if (compare_result < 0) {
      // Go Left
      if (node->left != nullptr) {
        node = node->left;
      } else {
        return nullptr;
      }
    } else {
      if (node->right != nullptr) {
        node = node->right;
      } else {
        return nullptr;
      }
    }
  }
  /* Shouldn't happen. */
  (void)fprintf(stderr,
                "Serious fault in RedBlackTree.c:red_black_tree_find!\n");
  exit(1);
}

bool red_black_tree_exists(RedBlackTree bt, const void *key) {
  RbtreeNode *node;
  int compare_result;
  if (!bt->head) {
    return false;
  }
  node = bt->head;
  while (node != nullptr) {
    compare_result = bt->compare(&(RedBlackTreeCompareCall){
        .lhs = key,
        .rhs = node->key,
        .context = bt->context,
    });
    if (compare_result == 0) {
      return true;
    }
    if (compare_result < 0) {
      // Go Left
      if (node->left != nullptr) {
        node = node->left;
      } else {
        return false;
      }
    } else {
      if (node->right != nullptr) {
        node = node->right;
      } else {
        return false;
      }
    }
  }
  /* Shouldn't happen. */
  (void)fprintf(stderr,
                "Serious fault in RedBlackTree.c:red_black_tree_exists!\n");
  exit(1);
}

bool red_black_tree_walk(RedBlackTree bt, int how, RedBlackTreeVisitor visitor,
                         void *context) {
  RbtreeNode *last;
  RbtreeNode *node;
  int depth = 0;
  if (!bt || !bt->head)
    return true;
  last = nullptr;
  node = bt->head;
  while (node != nullptr) {
    if (last == node->parent) {
      if (how == WALK_PREORDER) {
        if (!visitor(&(RedBlackTreeVisitCall){.key = node->key,
                                              .data = node->data,
                                              .depth = depth,
                                              .context = context}))
          return false;
      }
      if (node->left != nullptr) {
        depth++;
        last = node;
        node = node->left;
        continue;
      }
    }
    if (last == node->left || (last == node->parent && node->left == nullptr)) {
      if (how == WALK_INORDER) {
        if (!visitor(&(RedBlackTreeVisitCall){.key = node->key,
                                              .data = node->data,
                                              .depth = depth,
                                              .context = context}))
          return false;
      }
      if (node->right != nullptr) {
        depth++;
        last = node;
        node = node->right;
        continue;
      }
    }
    if (how == WALK_POSTORDER) {
      if (!visitor(&(RedBlackTreeVisitCall){.key = node->key,
                                            .data = node->data,
                                            .depth = depth,
                                            .context = context}))
        return false;
    }
    depth--;
    last = node;
    node = node->parent;
  }

  return true;
}

unsigned int red_black_tree_size(RedBlackTree bt) { return bt->size; }

void *red_black_tree_search(RedBlackTree bt, int method, const void *key) {
  RbtreeNode *node;
  RbtreeNode *last = nullptr;
  int compare_result = 0;
  int found = 0;

  if (!bt->head) {
    return nullptr;
  }

  if (method == SEARCH_FIRST) {
    node = red_black_tree_find_minimum(bt->head);
    return node->data;
  }
  if (method == SEARCH_LAST) {
    node = red_black_tree_find_maximum(bt->head);
    return node->data;
  }

  node = bt->head;
  while (node != nullptr) {
    last = node;
    compare_result = bt->compare(&(RedBlackTreeCompareCall){
        .lhs = key,
        .rhs = node->key,
        .context = bt->context,
    });
    if (compare_result == 0) {
      found = 1;
      break;
    }
    if (compare_result < 0) {
      // Go Left
      if (node->left != nullptr) {
        node = node->left;
      } else {
        node = nullptr;
        break;
      }
    } else {
      if (node->right != nullptr) {
        node = node->right;
      } else {
        node = nullptr;
        break;
      }
    }
  }

  if (found && (method == SEARCH_EQUAL || method == SEARCH_LTEQ ||
                method == SEARCH_GTEQ)) {
    if (node)
      return node->data;
    return nullptr;
  }

  if (!found && (method == SEARCH_EQUAL || method == SEARCH_NEXT ||
                 method == SEARCH_PREV)) {
    return nullptr;
  }

  if (method == SEARCH_GTEQ || (!found && method == SEARCH_GT)) {
    if (compare_result > 0) {
      node = red_black_tree_find_successor_node(last);
      if (node)
        return node->data;
      return node;
    }
    if (last)
      return last->data;
    return last;
  }

  if (method == SEARCH_LTEQ || (!found && method == SEARCH_LT)) {
    if (compare_result < 0) {
      node = red_black_tree_find_predecessor_node(last);
      return node->data;
    }
    return last->data;
  }

  if (method == SEARCH_NEXT || (found && method == SEARCH_GT)) {
    node = red_black_tree_find_successor_node(node);
    if (node)
      return node->data;
    return node;
  }

  if (method == SEARCH_PREV || (found && method == SEARCH_LT)) {
    node = red_black_tree_find_predecessor_node(node);
    if (node)
      return node->data;
    return node;
  }

  return nullptr;
}

void *red_black_tree_index(RedBlackTree bt, int index) {
  RbtreeNode *iter;
  int leftcount;

  if (bt == nullptr || index < 0 || (unsigned int)index >= bt->size)
    return nullptr;
  iter = bt->head;

  while (iter) {
    leftcount = (iter->left ? iter->left->count : 0);

    if (index == leftcount) {
      return iter->data;
    }
    if (index < leftcount) {
      iter = iter->left;
    } else {
      index -= leftcount + 1;
      iter = iter->right;
    }
  }
  red_black_tree_fail("index traversal did not find an expected node");
}
