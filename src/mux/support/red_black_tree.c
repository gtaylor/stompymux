/*
 * RedBlackTree.c - a redblack tree implementation
 *
 * Copyright (c) 2004,2005 Martin Murray <mmurray@monkey.org>
 * All rights reserved.
 *
 * Permission to use, copy, modify, and distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
 * ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 * ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 * OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 *
 */

/* red_black_tree.c - Red-black tree implementation for ordered lookups. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define NODE_RED 0
#define NODE_BLACK 1

#define SEARCH_EQUAL 0x1
#define SEARCH_GTEQ 0x2
#define SEARCH_LTEQ 0x3
#define SEARCH_GT 0x4
#define SEARCH_LT 0x5
#define SEARCH_NEXT 0x6
#define SEARCH_PREV 0x7
#define SEARCH_FIRST 0x8
#define SEARCH_LAST 0x9

#define WALK_PREORDER 0x100
#define WALK_INORDER 0x101
#define WALK_POSTORDER 0x102

typedef struct RedBlackTreeNode {
  struct RedBlackTreeNode *left, *right, *parent;
  void *key;
  void *data;
  int color;
  int count;
} rbtree_node;

typedef struct RedBlackTreeHead {
  struct RedBlackTreeNode *head;
  int (*compare_function)(void *, void *, void *);
  void *token;
  unsigned int size;
} *RedBlackTree;

RedBlackTree red_black_tree_init(int (*)(void *, void *, void *), void *);
void red_black_tree_destroy(RedBlackTree);

void red_black_tree_insert(RedBlackTree, void *key, void *data);
void *red_black_tree_find(RedBlackTree, void *key);
int red_black_tree_exists(RedBlackTree, void *key);
void *red_black_tree_delete(RedBlackTree, void *key);
void red_black_tree_release(RedBlackTree, void (*)(void *, void *, void *),
                            void *);

int red_black_tree_walk(RedBlackTree, int, int (*)(void *, void *, int, void *),
                        void *);
unsigned int red_black_tree_size(RedBlackTree);
void *red_black_tree_search(RedBlackTree, int, void *);
void *red_black_tree_index(RedBlackTree, int);

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

static rbtree_node *red_black_tree_find_successor_node(rbtree_node *node) {
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

static rbtree_node *red_black_tree_allocate(rbtree_node *parent, void *key,
                                            void *data) {
  rbtree_node *temp;
  temp = malloc(sizeof(struct RedBlackTreeNode));
  memset(temp, 0, sizeof(struct RedBlackTreeNode));
  temp->parent = parent;
  temp->key = key;
  temp->data = data;
  temp->count = 1;
  return temp;
}

static void red_black_tree_rotate_right(RedBlackTree bt, rbtree_node *pivot) {
  rbtree_node *child;

  if (!pivot || !pivot->left)
    return;
  child = pivot->left;

  pivot->left = child->right;
  if (child->right != nullptr)
    child->right->parent = pivot;

  child->parent = pivot->parent;

  if (pivot->parent) {
    if (pivot->parent->left == pivot)
      pivot->parent->left = child;
    else
      pivot->parent->right = child;
  } else
    bt->head = child;
  child->right = pivot;
  pivot->parent = child;
  child->count = pivot->count;
  pivot->count = 1 + (pivot->left ? pivot->left->count : 0) +
                 (pivot->right ? pivot->right->count : 0);
}

static void red_black_tree_rotate_left(RedBlackTree bt, rbtree_node *pivot) {
  rbtree_node *child;

  if (!pivot || !pivot->right)
    return;
  child = pivot->right;

  pivot->right = child->left;
  if (child->left != nullptr)
    child->left->parent = pivot;

  child->parent = pivot->parent;

  if (pivot->parent) {
    if (pivot->parent->right == pivot)
      pivot->parent->right = child;
    else
      pivot->parent->left = child;
  } else
    bt->head = child;
  child->left = pivot;
  pivot->parent = child;
  child->count = pivot->count;
  pivot->count = 1 + (pivot->left ? pivot->left->count : 0) +
                 (pivot->right ? pivot->right->count : 0);
}

void red_black_tree_insert(RedBlackTree bt, void *key, void *data) {
  rbtree_node *node;
  rbtree_node *iter;
  int compare_result;

  if (!bt->head) {
    bt->head = red_black_tree_allocate(nullptr, key, data);
    bt->size++;
    bt->head->color = NODE_BLACK;
    return;
  }

  node = bt->head;
  while (node != nullptr) {
    compare_result = (*bt->compare_function)(key, node->key, bt->token);
    if (compare_result == 0) {
      // Key already exists, replace data.
      node->key = key;
      node->data = data;
      return;
    } else if (compare_result < 0) {
      // Go Left
      if (node->left != nullptr) {
        node = node->left;
      } else {
        node->left = red_black_tree_allocate(node, key, data);
        bt->size++;
        node = node->left;
        break;
      }
    } else {
      if (node->right != nullptr) {
        node = node->right;
      } else {
        node->right = red_black_tree_allocate(node, key, data);
        bt->size++;
        node = node->right;
        break;
      }
    }
  }

  iter = node->parent;
  while (iter) {
    iter->count++;
    iter = iter->parent;
  }

  node->color = NODE_RED;
  if (node->parent && node->parent->color == NODE_RED) {
    iter = node;
    while (iter != bt->head && iter->parent && iter->parent->parent &&
           iter->parent->color == NODE_RED) {
      bt->head->color = NODE_BLACK;
      if (iter->parent == iter->parent->parent->left) {
        // parent is left child of grandparent
        if (iter->parent->parent->right != nullptr &&
            iter->parent->parent->right->color == NODE_RED) {
          // Case 1:
          // The current node has a red uncle and it's parent is parent node is
          // a red left child.
          iter->parent->color = NODE_BLACK;
          iter->parent->parent->color = NODE_RED;
          if (iter->parent->parent->right)
            iter->parent->parent->right->color = NODE_BLACK;
          iter = iter->parent->parent;
          continue;
        } else {
          // Case 2 or 3:
          // The current node has a black uncle.
          if (iter->parent->right == iter) {
            // Case 2:
            // The current node has a black uncle and is the right child
            // of the parent. The parent is the red left child. The parent's
            // sibling, the current node's uncle, is black.
            red_black_tree_rotate_left(bt, iter->parent);
            iter = iter->left;
          }
          // Case 3:
          // The current node is a left child. It's parent is a red left child
          // and has a black sibling.
          iter->parent->color = NODE_BLACK;
          iter->parent->parent->color = NODE_RED;
          red_black_tree_rotate_right(bt, iter->parent->parent);
          break;
        }
      } else {
        // parent is right child of grandparent
        if (iter->parent->parent->left != nullptr &&
            iter->parent->parent->left->color == NODE_RED) {
          // Case 1:
          // The current node has a red uncle and it's parent is parent node is
          // a red right child.
          iter->parent->color = NODE_BLACK;
          iter->parent->parent->color = NODE_RED;
          if (iter->parent->parent->left)
            iter->parent->parent->left->color = NODE_BLACK;
          iter = iter->parent->parent;
          continue;
        } else {
          // Case 2 or 3:
          // The current node has a black uncle.
          if (iter->parent->left == iter) {
            // Case 2:
            // The current node has a black uncle and is the left child
            // of the parent. The parent is the red right child. The parent's
            // sibling, the current node's uncle, is black.
            red_black_tree_rotate_right(bt, iter->parent);
            iter = iter->right;
          }
          // Case 3:
          // The current node is a right child. It's parent is a red right child
          // and has a black sibling.
          iter->parent->color = NODE_BLACK;
          iter->parent->parent->color = NODE_RED;
          red_black_tree_rotate_left(bt, iter->parent->parent);
          continue;
        }
      }
    }
  }
  bt->head->color = NODE_BLACK;
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

int red_black_tree_exists(RedBlackTree bt, void *key) {
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

#define rbann(...)                                                             \
  do {                                                                         \
    printf("%d: ", __LINE__);                                                  \
    printf(__VA_ARGS__);                                                       \
    printf("\n");                                                              \
  } while (0)
#define rbfail(...)                                                            \
  do {                                                                         \
    printf("%d: ", __LINE__);                                                  \
    printf(__VA_ARGS__);                                                       \
    printf("\n");                                                              \
    abort();                                                                   \
  } while (0)

static void red_black_tree_unlink_leaf(RedBlackTree bt, rbtree_node *leaf) {
  rbtree_node *sibling = nullptr, *node;

  node = leaf;

  if (node->color == NODE_RED) {
    // if node is red and has at most one child, then it has no child.
    if (node->parent->left == node) {
      node->parent->left = nullptr;
    } else {
      node->parent->right = nullptr;
    }
    node->parent = nullptr;
    return;
  }
  // node is black so it has only one red child, two black children, or no
  // children. If it had two children, we would've handled that in
  // red_black_tree_delete()
  if (node->left) {
    if (node == bt->head) {
      bt->head = node->left;
      node->left->parent = nullptr;
    } else if (node->parent->left == node) {
      node->parent->left = node->left;
      node->left->parent = node->parent;
    } else {
      node->parent->right = node->left;
      node->left->parent = node->parent;
    }
    if (node->color == NODE_BLACK) {
      if (node->left->color == NODE_RED) {
        node->left->color = NODE_BLACK;
      } else {
        rbfail("shit.");
      }
    }
    node->parent = nullptr;
    node->left = nullptr;
    return;
  }

  if (node->right) {
    if (node == bt->head) {
      bt->head = node->right;
      node->right->parent = nullptr;
    } else if (node->parent->right == node) {
      node->parent->right = node->right;
      node->right->parent = node->parent;
    } else {
      node->parent->left = node->right;
      node->right->parent = node->parent;
    }
    if (node->color == NODE_BLACK) {
      if (node->right->color == NODE_RED) {
        node->right->color = NODE_BLACK;
      } else {
        rbfail("shit.");
      }
    }
    node->right = nullptr;
    node->left = nullptr;
    return;
  }
  // node is black and has no children, if it had two children, then
  // red_black_tree_delete would have handled the situation. Since the node is
  // black and has no children, things get complicated.

  while (node != bt->head) {
    // First we loop through the Case 2a situations.
    //
    if (node->parent->left == node) {
      sibling = node->parent->right;
    } else {
      sibling = node->parent->left;
    }
    // if the parent is black, it has two black children, or no children.
    // since we are a child, we're guaranteed a sibling.
    if (!sibling) // Sanity Check
      rbfail(
          "serious braindamage: black child of black parent has no sibling.");
    if (node->parent->color == NODE_BLACK && sibling->color == NODE_BLACK &&
        (!sibling->right || sibling->right->color == NODE_BLACK) &&
        (!sibling->left || sibling->left->color == NODE_BLACK)) {
      sibling->color = NODE_RED;
      node = node->parent;
      continue;
    }
    break;
  }

  if (node == bt->head) {
    node->color = NODE_BLACK;
    goto done;
  }

  if (node->parent->left == node) {
    sibling = node->parent->right;
  } else {
    sibling = node->parent->left;
  }

  if (node->parent->color == NODE_BLACK && sibling &&
      sibling->color == NODE_RED &&
      (!sibling->right || sibling->right->color == NODE_BLACK) &&
      (!sibling->left || sibling->left->color == NODE_BLACK)) {
    node->parent->color = NODE_RED;
    sibling->color = NODE_BLACK;
    if (node->parent->left == node) {
      red_black_tree_rotate_left(bt, node->parent);
      sibling = node->parent->right;
    } else {
      red_black_tree_rotate_right(bt, node->parent);
      sibling = node->parent->left;
    }
  }

  if (!sibling) {
    if (node->parent->color == NODE_RED)
      node->parent->color = NODE_BLACK;
    goto done;
  }

  if (node->parent->color == NODE_RED && sibling->color == NODE_BLACK &&
      (!sibling->right || sibling->right->color == NODE_BLACK) &&
      (!sibling->left || sibling->left->color == NODE_BLACK)) {

    sibling->color = NODE_RED;
    node->parent->color = NODE_BLACK;
    goto done;
  }

  if (node->parent->left == node) {

    if (sibling->color == NODE_BLACK &&
        (sibling->left && sibling->left->color == NODE_RED) &&
        (!sibling->right || sibling->right->color == NODE_BLACK)) {
      sibling->color = NODE_RED;
      sibling->left->color = NODE_BLACK;
      red_black_tree_rotate_right(bt, sibling);
      sibling = sibling->parent;
    }

    if (sibling->color == NODE_BLACK &&
        (sibling->right && sibling->right->color == NODE_RED)) {
      sibling->right->color = NODE_BLACK;
      sibling->color = sibling->parent->color;
      sibling->parent->color = NODE_BLACK;
      red_black_tree_rotate_left(bt, sibling->parent);
    }
  } else {

    if (sibling->color == NODE_BLACK &&
        (sibling->right && sibling->right->color == NODE_RED) &&
        (!sibling->left || sibling->left->color == NODE_BLACK)) {
      sibling->color = NODE_RED;
      sibling->right->color = NODE_BLACK;
      red_black_tree_rotate_left(bt, sibling);
      sibling = sibling->parent;
    }

    if (sibling->color == NODE_BLACK &&
        (sibling->left && sibling->left->color == NODE_RED)) {
      sibling->left->color = NODE_BLACK;
      sibling->color = sibling->parent->color;
      sibling->parent->color = NODE_BLACK;
      red_black_tree_rotate_right(bt, sibling->parent);
    }
  }

done:
  if (leaf->parent->left == leaf) {
    leaf->parent->left = nullptr;
  } else if (leaf->parent->right == leaf) {
    leaf->parent->right = nullptr;
  } else {
    rbfail("major braindamage.");
  }
  return;
}

void *red_black_tree_delete(RedBlackTree bt, void *key) {
  rbtree_node *node = nullptr, *child = nullptr, *tail;
  void *data;
  int compare_result;

  if (!bt->head) {
    return nullptr;
  }

  node = bt->head;
  while (node != nullptr) {
    compare_result = (*bt->compare_function)(key, node->key, bt->token);
    if (compare_result == 0) {
      break;
    } else if (compare_result < 0) {
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

  if (node == nullptr) {
    return node;
  }

  data = node->data;
  bt->size--;

  // XXX: handle deleting the head.

  if (node == bt->head && node->left == nullptr && node->right == nullptr) {
    bt->head = nullptr;
    free(node);
    return data;
  }

  /*
   * PROPERTY 3 OF RED BLACK TREES STATES:
   *
   * Any two paths from a given node v down to a leaf node contain
   * the same number of black nodes.
   *
   * MEANING:
   * That all paths to all leaf nodes should contain the same
   * number of black nodes. Thus, we need to handle deleting a
   * black node in every situation, even if it is a leaf.
   */

  // our child has at most one child (or none.)
  if (node->left == nullptr || node->right == nullptr) {
    tail = node;
    while (tail) {
      tail->count--;
      tail = tail->parent;
    }
    red_black_tree_unlink_leaf(bt, node);
    free(node);
    return data;
  }
  // If we have full children, then we're guaranteed a successor
  // without empty children.

  child = red_black_tree_find_successor_node(node);
  if (!child)
    return data;

  tail = child;
  while (tail) {
    tail->count--;
    tail = tail->parent;
  }
  red_black_tree_unlink_leaf(bt, child);

  node->data = child->data;
  node->key = child->key;

  // XXX: finish delete

  free(child);
  return data;
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
  rbfail("major braindamage.");
}
