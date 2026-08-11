/* red_black_tree_mutation.c - Red-black tree insertion and deletion. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/support/red_black_tree.h"
#include "mux/support/red_black_tree_internal.h"

[[noreturn]] static void red_black_tree_fail(const char *message) {
  (void)fprintf(stderr, "Red-black tree invariant failure: %s\n", message);
  abort();
}

typedef struct RedBlackTreeNodeAllocation {
  RbtreeNode *parent;
  void *key;
  void *data;
} RedBlackTreeNodeAllocation;

static RbtreeNode *
red_black_tree_allocate(const RedBlackTreeNodeAllocation *allocation) {
  RbtreeNode *temp;
  temp = malloc(sizeof(struct RedBlackTreeNode));
  if (temp == nullptr)
    red_black_tree_fail("unable to allocate a node");
  memset(temp, 0, sizeof(struct RedBlackTreeNode));
  temp->parent = allocation->parent;
  temp->key = allocation->key;
  temp->data = allocation->data;
  temp->count = 1;
  return temp;
}

static RbtreeNode *red_black_tree_require_parent(RbtreeNode *node) {
  if (node == nullptr || node->parent == nullptr)
    red_black_tree_fail("non-root node has no parent");
  return node->parent;
}

static void red_black_tree_rotate_right(RedBlackTree bt, RbtreeNode *pivot) {
  RbtreeNode *child;

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

static void red_black_tree_rotate_left(RedBlackTree bt, RbtreeNode *pivot) {
  RbtreeNode *child;

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
  RbtreeNode *node;
  RbtreeNode *iter;
  int compare_result;

  if (!bt->head) {
    bt->head = red_black_tree_allocate(
        &(RedBlackTreeNodeAllocation){.key = key, .data = data});
    bt->size++;
    bt->head->color = RED_BLACK_TREE_BLACK;
    return;
  }

  node = bt->head;
  while (node != nullptr) {
    compare_result = bt->compare(&(RedBlackTreeCompareCall){
        .lhs = key,
        .rhs = node->key,
        .context = bt->context,
    });
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
        node->left = red_black_tree_allocate(&(RedBlackTreeNodeAllocation){
            .parent = node, .key = key, .data = data});
        bt->size++;
        node = node->left;
        break;
      }
    } else {
      if (node->right != nullptr) {
        node = node->right;
      } else {
        node->right = red_black_tree_allocate(&(RedBlackTreeNodeAllocation){
            .parent = node, .key = key, .data = data});
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

  node->color = RED_BLACK_TREE_RED;
  if (node->parent && node->parent->color == RED_BLACK_TREE_RED) {
    iter = node;
    while (iter != bt->head && iter->parent && iter->parent->parent &&
           iter->parent->color == RED_BLACK_TREE_RED) {
      bt->head->color = RED_BLACK_TREE_BLACK;
      if (iter->parent == iter->parent->parent->left) {
        // parent is left child of grandparent
        if (iter->parent->parent->right != nullptr &&
            iter->parent->parent->right->color == RED_BLACK_TREE_RED) {
          // Case 1:
          // The current node has a red uncle and it's parent is parent node is
          // a red left child.
          iter->parent->color = RED_BLACK_TREE_BLACK;
          iter->parent->parent->color = RED_BLACK_TREE_RED;
          if (iter->parent->parent->right)
            iter->parent->parent->right->color = RED_BLACK_TREE_BLACK;
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
          iter->parent->color = RED_BLACK_TREE_BLACK;
          iter->parent->parent->color = RED_BLACK_TREE_RED;
          red_black_tree_rotate_right(bt, iter->parent->parent);
          break;
        }
      } else {
        // parent is right child of grandparent
        if (iter->parent->parent->left != nullptr &&
            iter->parent->parent->left->color == RED_BLACK_TREE_RED) {
          // Case 1:
          // The current node has a red uncle and it's parent is parent node is
          // a red right child.
          iter->parent->color = RED_BLACK_TREE_BLACK;
          iter->parent->parent->color = RED_BLACK_TREE_RED;
          if (iter->parent->parent->left)
            iter->parent->parent->left->color = RED_BLACK_TREE_BLACK;
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
          iter->parent->color = RED_BLACK_TREE_BLACK;
          iter->parent->parent->color = RED_BLACK_TREE_RED;
          red_black_tree_rotate_left(bt, iter->parent->parent);
          continue;
        }
      }
    }
  }
  bt->head->color = RED_BLACK_TREE_BLACK;
}

static void red_black_tree_unlink_leaf(RedBlackTree bt, RbtreeNode *leaf) {
  RbtreeNode *sibling = nullptr, *node;

  node = leaf;

  if (node->color == RED_BLACK_TREE_RED) {
    RbtreeNode *parent = red_black_tree_require_parent(node);

    // if node is red and has at most one child, then it has no child.
    if (parent->left == node) {
      parent->left = nullptr;
    } else {
      parent->right = nullptr;
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
    } else {
      RbtreeNode *parent = red_black_tree_require_parent(node);

      if (parent->left == node) {
        parent->left = node->left;
      } else {
        parent->right = node->left;
      }
      node->left->parent = parent;
    }
    if (node->color == RED_BLACK_TREE_BLACK) {
      if (node->left->color == RED_BLACK_TREE_RED) {
        node->left->color = RED_BLACK_TREE_BLACK;
      } else {
        red_black_tree_fail("black node has a non-red left child");
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
    } else {
      RbtreeNode *parent = red_black_tree_require_parent(node);

      if (parent->right == node) {
        parent->right = node->right;
      } else {
        parent->left = node->right;
      }
      node->right->parent = parent;
    }
    if (node->color == RED_BLACK_TREE_BLACK) {
      if (node->right->color == RED_BLACK_TREE_RED) {
        node->right->color = RED_BLACK_TREE_BLACK;
      } else {
        red_black_tree_fail("black node has a non-red right child");
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
    RbtreeNode *parent = red_black_tree_require_parent(node);

    // First we loop through the Case 2a situations.
    //
    if (parent->left == node) {
      sibling = parent->right;
    } else {
      sibling = parent->left;
    }
    // if the parent is black, it has two black children, or no children.
    // since we are a child, we're guaranteed a sibling.
    if (!sibling) // Sanity Check
      red_black_tree_fail(
          "serious braindamage: black child of black parent has no sibling.");
    if (parent->color == RED_BLACK_TREE_BLACK &&
        sibling->color == RED_BLACK_TREE_BLACK &&
        (!sibling->right || sibling->right->color == RED_BLACK_TREE_BLACK) &&
        (!sibling->left || sibling->left->color == RED_BLACK_TREE_BLACK)) {
      sibling->color = RED_BLACK_TREE_RED;
      node = parent;
      continue;
    }
    break;
  }

  if (node == bt->head) {
    node->color = RED_BLACK_TREE_BLACK;
    goto done;
  }

  RbtreeNode *parent = red_black_tree_require_parent(node);
  if (parent->left == node) {
    sibling = parent->right;
  } else {
    sibling = parent->left;
  }

  if (parent->color == RED_BLACK_TREE_BLACK && sibling &&
      sibling->color == RED_BLACK_TREE_RED &&
      (!sibling->right || sibling->right->color == RED_BLACK_TREE_BLACK) &&
      (!sibling->left || sibling->left->color == RED_BLACK_TREE_BLACK)) {
    parent->color = RED_BLACK_TREE_RED;
    sibling->color = RED_BLACK_TREE_BLACK;
    if (parent->left == node) {
      red_black_tree_rotate_left(bt, parent);
      parent = red_black_tree_require_parent(node);
      sibling = parent->right;
    } else {
      red_black_tree_rotate_right(bt, parent);
      parent = red_black_tree_require_parent(node);
      sibling = parent->left;
    }
  }

  if (!sibling) {
    if (parent->color == RED_BLACK_TREE_RED)
      parent->color = RED_BLACK_TREE_BLACK;
    goto done;
  }

  if (parent->color == RED_BLACK_TREE_RED &&
      sibling->color == RED_BLACK_TREE_BLACK &&
      (!sibling->right || sibling->right->color == RED_BLACK_TREE_BLACK) &&
      (!sibling->left || sibling->left->color == RED_BLACK_TREE_BLACK)) {

    sibling->color = RED_BLACK_TREE_RED;
    parent->color = RED_BLACK_TREE_BLACK;
    goto done;
  }

  if (parent->left == node) {

    if (sibling->color == RED_BLACK_TREE_BLACK &&
        (sibling->left && sibling->left->color == RED_BLACK_TREE_RED) &&
        (!sibling->right || sibling->right->color == RED_BLACK_TREE_BLACK)) {
      sibling->color = RED_BLACK_TREE_RED;
      sibling->left->color = RED_BLACK_TREE_BLACK;
      red_black_tree_rotate_right(bt, sibling);
      sibling = sibling->parent;
    }

    if (sibling->color == RED_BLACK_TREE_BLACK &&
        (sibling->right && sibling->right->color == RED_BLACK_TREE_RED)) {
      sibling->right->color = RED_BLACK_TREE_BLACK;
      sibling->color = sibling->parent->color;
      sibling->parent->color = RED_BLACK_TREE_BLACK;
      red_black_tree_rotate_left(bt, sibling->parent);
    }
  } else {

    if (sibling->color == RED_BLACK_TREE_BLACK &&
        (sibling->right && sibling->right->color == RED_BLACK_TREE_RED) &&
        (!sibling->left || sibling->left->color == RED_BLACK_TREE_BLACK)) {
      sibling->color = RED_BLACK_TREE_RED;
      sibling->right->color = RED_BLACK_TREE_BLACK;
      red_black_tree_rotate_left(bt, sibling);
      sibling = sibling->parent;
    }

    if (sibling->color == RED_BLACK_TREE_BLACK &&
        (sibling->left && sibling->left->color == RED_BLACK_TREE_RED)) {
      sibling->left->color = RED_BLACK_TREE_BLACK;
      sibling->color = sibling->parent->color;
      sibling->parent->color = RED_BLACK_TREE_BLACK;
      red_black_tree_rotate_right(bt, sibling->parent);
    }
  }

done:
  RbtreeNode *leaf_parent = red_black_tree_require_parent(leaf);
  if (leaf_parent->left == leaf) {
    leaf_parent->left = nullptr;
  } else if (leaf_parent->right == leaf) {
    leaf_parent->right = nullptr;
  } else {
    red_black_tree_fail("leaf is detached from its parent");
  }
  return;
}

void *red_black_tree_delete(RedBlackTree bt, void *key) {
  RbtreeNode *node = nullptr, *child = nullptr, *tail;
  void *data;
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
