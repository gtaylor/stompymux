/** @file
 * Public MUX support interface for red black tree.
 */
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

#include <stdint.h>

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

typedef struct RedBlackTreeHead *RedBlackTree;

typedef struct RedBlackTreeCompareCall {
  const void *lhs;
  const void *rhs;
  void *context;
} RedBlackTreeCompareCall;

typedef struct RedBlackTreeVisitCall {
  void *key;
  void *data;
  int depth;
  void *context;
} RedBlackTreeVisitCall;

typedef struct RedBlackTreeReleaseCall {
  void *key;
  void *data;
  void *context;
} RedBlackTreeReleaseCall;

typedef int (*RedBlackTreeCompare)(const RedBlackTreeCompareCall *call);
typedef bool (*RedBlackTreeVisitor)(const RedBlackTreeVisitCall *call);
typedef void (*RedBlackTreeRelease)(const RedBlackTreeReleaseCall *call);

/** Executes red black tree init. @param[in] compare Compare. @param[in] context
 * Operation context. */

RedBlackTree red_black_tree_init(RedBlackTreeCompare compare, void *context);
/** Destroys red black tree. @param[in] bt Bt. */

void red_black_tree_destroy(RedBlackTree bt);

/** Executes red black tree insert. @param[in] bt Bt. @param[in,out] key Lookup
 * key or command flags. @param[in,out] data Caller-provided data. */

void red_black_tree_insert(RedBlackTree bt, void *key, void *data);
/** Inserts data under a typed integer key stored inline with the tree node. */
void red_black_tree_insert_integer(RedBlackTree bt, intptr_t key, void *data);
/** Finds red black tree find. @param[in] bt Bt. @param[in] key Lookup key or
 * command flags. */

void *red_black_tree_find(RedBlackTree bt, const void *key);
/** Finds data stored under an integer key. */
void *red_black_tree_find_integer(RedBlackTree bt, intptr_t key);
/** Executes red black tree exists. @param[in] bt Bt. @param[in] key Lookup key
 * or command flags. */

bool red_black_tree_exists(RedBlackTree bt, const void *key);
/** Reports whether an integer key is present. */
bool red_black_tree_exists_integer(RedBlackTree bt, intptr_t key);
/** Executes red black tree delete. @param[in] bt Bt. @param[in] key Lookup key
 * or command flags. */

void *red_black_tree_delete(RedBlackTree bt, const void *key);
/** Deletes an integer key and returns its associated data. */
void *red_black_tree_delete_integer(RedBlackTree bt, intptr_t key);
/** Executes red black tree release. @param[in] bt Bt. @param[in] release
 * Release. @param[in] context Operation context. */

void red_black_tree_release(RedBlackTree bt, RedBlackTreeRelease release,
                            void *context);

/** Executes red black tree walk. @param[in] bt Bt. @param[in] how How.
 * @param[in] visitor Visitor. @param[in,out] context Operation context. */

bool red_black_tree_walk(RedBlackTree bt, int how, RedBlackTreeVisitor visitor,
                         void *context);
/** Executes red black tree size. @param[in] bt Bt. */

unsigned int red_black_tree_size(RedBlackTree bt);
/** Executes red black tree search. @param[in] bt Bt. @param[in] method Method.
 * @param[in] key Lookup key or command flags. */

void *red_black_tree_search(RedBlackTree bt, int method, const void *key);
/** Executes red black tree index. @param[in] bt Bt. @param[in] index Zero-based
 * index. */

void *red_black_tree_index(RedBlackTree bt, int index);
