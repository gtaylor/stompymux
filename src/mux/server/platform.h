/* platform.h - Build configuration, platform types, and compile-time limits */

#pragma once

#include "btmux_build_config.h"

#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>

#include <netinet/in.h>
#include <netinet/tcp.h>
#include <sys/socket.h>

typedef long DbRef;
typedef long Flag;
typedef int Power;
typedef char boolexp_type;
typedef char IBUF[16];
typedef struct map_data MAP;
typedef struct mech_data MECH;

#include "mux/server/debug.h"

#include <sys/resource.h>

#include <event2/event.h>

/* TEST_MALLOC:	Defining this makes a malloc that keeps track of the number
 *		of blocks allocated.  Good for testing for Memory leaks.
 * ATR_NAME:	Define if you want name to be stored as an attribute on the
 *		object rather than in the object structure.
 */

/* Compile time options */

/* #define TEST_MALLOC */             /* Keep track of block allocs */
#define SIDE_EFFECT_FUNCTIONS         /* Those neat funcs that should be       \
                                       * commands */
#define ENTERLEAVE_PARANOID           /* Enter/leave commands                  \
                                         require opposite locks succeeding     \
                                         as well */
constexpr int PLAYER_NAME_LIMIT = 22; /* Max length for player names */
constexpr int NUM_ENV_VARS = 10;      /* Number of env vars (%0 et al) */
constexpr int MAX_ARG = 100;          /* max # args from command processor */
constexpr int MAX_GLOBAL_REGS = 10;   /* r() registers */

constexpr int HASH_FACTOR = 16; /* How much hashing you want. */

constexpr int OUTPUT_BLOCK_SIZE = 16384;
static inline char *StringCopy(char *dst, const char *src) {
  return strcpy(dst, src);
}
static inline char *StringCopyTrunc(char *dst, const char *src, size_t n) {
  return strncpy(dst, src, n);
}

#define CHANNEL_HISTORY
constexpr int CHANNEL_HISTORY_LEN = 20; /* at max 20 last msgs */
constexpr int COMMAND_HISTORY_LEN = 10; /* at max 10 last msgs */

/* magic lock cookies */
constexpr char NOT_TOKEN = '!';
constexpr char AND_TOKEN = '&';
constexpr char OR_TOKEN = '|';
constexpr char LOOKUP_TOKEN = '*';
constexpr char NUMBER_TOKEN = '#';
constexpr char INDIR_TOKEN = '@'; /* One of these two should go. */
constexpr char CARRY_TOKEN = '+'; /* One of these two should go. */
constexpr char IS_TOKEN = '=';
constexpr char OWNER_TOKEN = '$';

/* matching attribute tokens */
constexpr char AMATCH_CMD = '$';
constexpr char AMATCH_LISTEN = '^';

/* delimiters for various things */
constexpr char EXIT_DELIMITER = ';';
constexpr char ARG_DELIMITER = '=';
constexpr char ARG_LIST_DELIM = ',';

/* These chars get replaced by the current item from a list in commands and
 * functions that do iterative replacement, such as @apply_marked, dolist,
 * the eval= operator for @search, and iter().
 */

constexpr char BOUND_VAR[] = "##";
constexpr char LISTPLACE_VAR[] = "#@";

#ifdef BRAIN_DAMAGE /* a kludge to get it to work on a mutant                  \
                     * DENIX system */
#undef toupper
#endif

#ifdef TEST_MALLOC
extern int malloc_count;

#define XMALLOC(x, y)                                                          \
  (fprintf(stderr, "Malloc: %s\n", (y)), malloc_count++, (char *)malloc((x)))
#define XFREE(x, y)                                                            \
  (fprintf(stderr, "Free: %s\n", (y)),                                         \
   ((x) ? malloc_count--, free((x)), (x) = nullptr : (x)))
#else
#define XMALLOC(x, y) (char *)malloc((x))
#define XFREE(x, y) (free((x)), (x) = nullptr)
#endif /* TEST_MALLOC */

#ifdef ENTERLEAVE_PARANOID
#define ENTER_REQUIRES_LEAVESUCC /* Enter checks leaveloc of player's          \
                                    origin */
#define LEAVE_REQUIRES_ENTERSUCC /* Leave checks enterlock of player's         \
                                    origin */
#endif

#include <sys/socket.h>
