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

/*
 * Generic function-pointer type for tables that store heterogeneous
 * handler/interpreter functions in a single field (e.g. command dispatch
 * tables, config directive tables). Storing a function pointer as `void *`
 * is not portable ISO C (only POSIX guarantees it); casting through this
 * type instead, then back to the real signature at the call site, is fully
 * portable since function-pointer-to-function-pointer conversion is always
 * well-defined as long as the call goes through the original type.
 */
typedef void (*GenericFnPtr)(void);

#include "mux/server/diagnostics.h"

#include <sys/resource.h>

/* ATR_NAME:	Define if you want name to be stored as an attribute on the
 *		object rather than in the object structure.
 */

/* Compile time options */

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
