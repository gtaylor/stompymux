/* Private red-black tree node representation. */

#pragma once

#include "mux/support/red_black_tree.h"

typedef enum RedBlackTreeColor {
  RED_BLACK_TREE_RED,
  RED_BLACK_TREE_BLACK,
} RedBlackTreeColor;

typedef struct RedBlackTreeNode RedBlackTreeNode;
struct RedBlackTreeNode {
  RedBlackTreeNode *left;
  RedBlackTreeNode *right;
  RedBlackTreeNode *parent;
  void *key;
  void *data;
  RedBlackTreeColor color;
  int count;
};

struct RedBlackTreeHead {
  RedBlackTreeNode *head;
  RedBlackTreeCompare compare;
  void *context;
  unsigned int size;
};

typedef RedBlackTreeNode rbtree_node;

RedBlackTreeNode *red_black_tree_find_successor_node(RedBlackTreeNode *node);
