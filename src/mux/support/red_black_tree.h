/*
 * red_black_tree.h
 *
 * Copyright (c) 2004,2005 Martin Murray <mmurray@mon.org>
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

/* red_black_tree.h - Red-black tree types and ordered lookup interface. */

#pragma once

enum : int {
  SEARCH_EQUAL = 0x1,
  SEARCH_GTEQ = 0x2,
  SEARCH_LTEQ = 0x3,
  SEARCH_GT = 0x4,
  SEARCH_LT = 0x5,
  SEARCH_NEXT = 0x6,
  SEARCH_PREV = 0x7,
  SEARCH_FIRST = 0x8,
  SEARCH_LAST = 0x9,
};

enum : int {
  WALK_PREORDER = 0x100,
  WALK_INORDER = 0x101,
  WALK_POSTORDER = 0x102,
};

#ifndef DEBUG
typedef void *RedBlackTree;
#else
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
#endif

RedBlackTree red_black_tree_init(int (*)(void *, void *, void *), void *);
void red_black_tree_destroy(RedBlackTree);

void red_black_tree_insert(RedBlackTree, void *, void *);
void *red_black_tree_find(RedBlackTree, void *);
int red_black_tree_exists(RedBlackTree, void *);
void *red_black_tree_delete(RedBlackTree, void *);
void red_black_tree_release(RedBlackTree, void (*)(void *, void *, void *),
                            void *);

int red_black_tree_walk(RedBlackTree, int, int (*)(void *, void *, int, void *),
                        void *);
unsigned int red_black_tree_size(RedBlackTree);
void *red_black_tree_search(RedBlackTree, int, void *);
void *red_black_tree_index(RedBlackTree, int);
