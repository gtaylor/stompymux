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

typedef struct RedBlackTreeHead *RedBlackTree;

typedef struct RedBlackTreeCompareCall {
  void *lhs;
  void *rhs;
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
typedef int (*RedBlackTreeVisitor)(const RedBlackTreeVisitCall *call);
typedef void (*RedBlackTreeRelease)(const RedBlackTreeReleaseCall *call);

RedBlackTree red_black_tree_init(RedBlackTreeCompare compare, void *context);
void red_black_tree_destroy(RedBlackTree /*bt*/);

void red_black_tree_insert(RedBlackTree /*bt*/, void * /*key*/,
                           void * /*data*/);
void *red_black_tree_find(RedBlackTree /*bt*/, void * /*key*/);
bool red_black_tree_exists(RedBlackTree /*bt*/, void * /*key*/);
void *red_black_tree_delete(RedBlackTree /*bt*/, void * /*key*/);
void red_black_tree_release(RedBlackTree bt, RedBlackTreeRelease release,
                            void *context);

int red_black_tree_walk(RedBlackTree bt, int how, RedBlackTreeVisitor visitor,
                        void *context);
unsigned int red_black_tree_size(RedBlackTree /*bt*/);
void *red_black_tree_search(RedBlackTree /*bt*/, int /*method*/,
                            void * /*key*/);
void *red_black_tree_index(RedBlackTree /*bt*/, int /*index*/);
