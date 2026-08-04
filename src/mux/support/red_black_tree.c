/* red_black_tree.c - Red-black tree ownership and ordered queries. */

#include "mux/server/platform.h"

#include "mux/support/red_black_tree.h"
#include "mux/support/red_black_tree_internal.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

[[noreturn]] static void red_black_tree_fail(const char *message) {
  fprintf(stderr, "Red-black tree invariant failure: %s\n", message);
  abort();
}

RedBlackTree red_black_tree_init(int (*compare_function)(void *, void *,
                                                         void *),
                                 void *token) {
  RedBlackTree temp;

  temp = malloc(sizeof(struct RedBlackTreeHead));
  if (temp == nullptr)
    return nullptr;
  memset(temp, 0, sizeof(struct RedBlackTreeHead));
  temp->compare_function = compare_function;
  temp->token = token;
  temp->size = 0;
  return temp;
}

static rbtree_node *red_black_tree_find_minimum(rbtree_node *node) {
  rbtree_node *child;
  child = node;
  if (!node)
    return nullptr;
  while (child->left != nullptr)
    child = child->left;
  return child;
}

static rbtree_node *red_black_tree_find_maximum(rbtree_node *node) {
  rbtree_node *child;
  child = node;
  if (!node)
    return nullptr;
  while (child->right != nullptr)
    child = child->right;
  return child;
}

rbtree_node *red_black_tree_find_successor_node(rbtree_node *node) {
  rbtree_node *child, *parent;
  if (!node)
    return nullptr;
  if (node->right != nullptr) {
    child = node->right;
    while (child->left != nullptr) {
      child = child->left;
    }
    return child;
  } else {
    child = node;
    parent = node->parent;
    while (parent != nullptr && child == parent->right) {
      child = parent;
      parent = child->parent;
    }
    return parent;
  }
  return nullptr;
}

static rbtree_node *red_black_tree_find_predecessor_node(rbtree_node *node) {
  rbtree_node *child, *parent;
  if (!node)
    return nullptr;
  if (node->left != nullptr) {
    child = node->left;
    while (child->right != nullptr)
      child = child->right;
    return child;
  } else {
    child = node;
    parent = node->parent;
    while (parent != nullptr && child == parent->left) {
      child = parent;
      parent = parent->parent;
    }
    return parent;
  }
  return nullptr;
}

void red_black_tree_release(RedBlackTree bt,
                            void (*release)(void *, void *, void *),
                            void *arg) {
  rbtree_node *node, *parent;
  node = bt->head;

  if (bt->head) {
    while (node != nullptr) {
      if (node->left != nullptr) {
        node = node->left;
        continue;
      } else if (node->right != nullptr) {
        node = node->right;
        continue;
      } else {
        parent = node->parent;
        if (parent && parent->left == node)
          parent->left = nullptr;
        else if (parent && parent->right == node)
          parent->right = nullptr;
        else if (parent) {
          fprintf(stderr, "serious braindamage.\n");
          exit(1);
        }
        release(node->key, node->data, arg);
        free(node);
        node = parent;
      }
    }
  }
  free(bt);
  return;
}

void red_black_tree_destroy(RedBlackTree bt) {
  rbtree_node *node, *parent;
  node = bt->head;

  if (bt->head) {
    while (node != nullptr) {
      if (node->left != nullptr) {
        node = node->left;
        continue;
      } else if (node->right != nullptr) {
        node = node->right;
        continue;
      } else {
        parent = node->parent;
        if (parent && parent->left == node)
          parent->left = nullptr;
        else if (parent && parent->right == node)
          parent->right = nullptr;
        else if (parent) {
          fprintf(stderr, "serious braindamage.\n");
          exit(1);
        }
        free(node);
        node = parent;
      }
    }
  }
  free(bt);
  return;
}

void *red_black_tree_find(RedBlackTree bt, void *key) {
  rbtree_node *node;
  int compare_result;

  if (!bt->head) {
    return nullptr;
  }
  node = bt->head;
  while (node != nullptr) {
    compare_result = (*bt->compare_function)(key, node->key, bt->token);
    if (compare_result == 0) {
      return node->data;
    } else if (compare_result < 0) {
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
  fprintf(stderr, "Serious fault in RedBlackTree.c:red_black_tree_find!\n");
  exit(1);
}

bool red_black_tree_exists(RedBlackTree bt, void *key) {
  rbtree_node *node;
  int compare_result;
  if (!bt->head) {
    return 0;
  }
  node = bt->head;
  while (node != nullptr) {
    compare_result = (*bt->compare_function)(key, node->key, bt->token);
    if (compare_result == 0) {
      return 1;
    } else if (compare_result < 0) {
      // Go Left
      if (node->left != nullptr) {
        node = node->left;
      } else {
        return 0;
      }
    } else {
      if (node->right != nullptr) {
        node = node->right;
      } else {
        return 0;
      }
    }
  }
  /* Shouldn't happen. */
  fprintf(stderr, "Serious fault in RedBlackTree.c:red_black_tree_exists!\n");
  exit(1);
}

int red_black_tree_walk(RedBlackTree bt, int how,
                        int (*callback)(void *, void *, int, void *),
                        void *arg) {
  rbtree_node *last, *node;
  int depth = 0;
  if (!bt || !bt->head)
    return 1;
  last = nullptr;
  node = bt->head;
  while (node != nullptr) {
    if (last == node->parent) {
      if (how == WALK_PREORDER)
        if (!(*callback)(node->key, node->data, depth, arg))
          return 0;
      if (node->left != nullptr) {
        depth++;
        last = node;
        node = node->left;
        continue;
      }
    }
    if (last == node->left || (last == node->parent && node->left == nullptr)) {
      if (how == WALK_INORDER)
        if (!(*callback)(node->key, node->data, depth, arg))
          return 0;
      if (node->right != nullptr) {
        depth++;
        last = node;
        node = node->right;
        continue;
      }
    }
    if (how == WALK_POSTORDER)
      if (!(*callback)(node->key, node->data, depth, arg))
        return 0;
    depth--;
    last = node;
    node = node->parent;
  }

  return 1;
}

unsigned int red_black_tree_size(RedBlackTree bt) { return bt->size; }

void *red_black_tree_search(RedBlackTree bt, int method, void *key) {
  rbtree_node *node, *last;
  int compare_result;
  int found = 0;

  if (!bt->head) {
    return nullptr;
  }

  if (method == SEARCH_FIRST) {
    node = red_black_tree_find_minimum(bt->head);
    return node->data;
  } else if (method == SEARCH_LAST) {
    node = red_black_tree_find_maximum(bt->head);
    return node->data;
  }

  node = bt->head;
  while (node != nullptr) {
    last = node;
    compare_result = (*bt->compare_function)(key, node->key, bt->token);
    if (compare_result == 0) {
      found = 1;
      break;
    } else if (compare_result < 0) {
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
    else
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
      else
        return node;
    } else {
      if (last)
        return last->data;
      else
        return last;
    }
  }

  if (method == SEARCH_LTEQ || (!found && method == SEARCH_LT)) {
    if (compare_result < 0) {
      node = red_black_tree_find_predecessor_node(last);
      return node->data;
    } else {
      return last->data;
    }
  }

  if (method == SEARCH_NEXT || (found && method == SEARCH_GT)) {
    node = red_black_tree_find_successor_node(node);
    if (node)
      return node->data;
    else
      return node;
  }

  if (method == SEARCH_PREV || (found && method == SEARCH_LT)) {
    node = red_black_tree_find_predecessor_node(node);
    if (node)
      return node->data;
    else
      return node;
  }

  return nullptr;
}

void *red_black_tree_index(RedBlackTree bt, int index) {
  rbtree_node *iter;
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
