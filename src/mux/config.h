/* config.h */

#pragma once

#include "btmux_build_config.h"

#define _GNU_SOURCE

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

typedef long dbref;
typedef long FLAG;
typedef int POWER;
typedef char boolexp_type;
typedef char IBUF[16];
typedef struct map_data MAP;
typedef struct mech_data MECH;

#include "debug.h"

#include <sys/resource.h>

#include <event.h>

/* TEST_MALLOC:	Defining this makes a malloc that keeps track of the number
 *		of blocks allocated.  Good for testing for Memory leaks.
 * ATR_NAME:	Define if you want name to be stored as an attribute on the
 *		object rather than in the object structure.
 */

/* Compile time options */

#define CONF_FILE "netmux.conf" /* Default config file */
#define FILEDIR "files/"        /* Source for @cat */

/* #define TEST_MALLOC */     /* Keep track of block allocs */
#define SIDE_EFFECT_FUNCTIONS /* Those neat funcs that should be               \
                               * commands */
#define ENTERLEAVE_PARANOID   /* Enter/leave commands                          \
                                 require opposite locks succeeding             \
                                 as well */
#define PLAYER_NAME_LIMIT 22  /* Max length for player names */
#define NUM_ENV_VARS 10       /* Number of env vars (%0 et al) */
#define MAX_ARG 100           /* max # args from command processor */
#define MAX_GLOBAL_REGS 10    /* r() registers */

#define HASH_FACTOR 16 /* How much hashing you want. */

#define OUTPUT_BLOCK_SIZE 16384
#define StringCopy strcpy
#define StringCopyTrunc strncpy

#define CHANNEL_HISTORY
#define CHANNEL_HISTORY_LEN 20 /* at max 20 last msgs */
#define COMMAND_HISTORY_LEN 10 /* at max 10 last msgs */

/* magic lock cookies */
#define NOT_TOKEN '!'
#define AND_TOKEN '&'
#define OR_TOKEN '|'
#define LOOKUP_TOKEN '*'
#define NUMBER_TOKEN '#'
#define INDIR_TOKEN '@' /* One of these two should go. */
#define CARRY_TOKEN '+' /* One of these two should go. */
#define IS_TOKEN '='
#define OWNER_TOKEN '$'

/* matching attribute tokens */
#define AMATCH_CMD '$'
#define AMATCH_LISTEN '^'

/* delimiters for various things */
#define EXIT_DELIMITER ';'
#define ARG_DELIMITER '='
#define ARG_LIST_DELIM ','

/* These chars get replaced by the current item from a list in commands and
 * functions that do iterative replacement, such as @apply_marked, dolist,
 * the eval= operator for @search, and iter().
 */

#define BOUND_VAR "##"
#define LISTPLACE_VAR "#@"

/* amount of object endowment, based on cost */
#define OBJECT_ENDOWMENT(cost)                                                 \
  (((cost) / mudconf.sacfactor) + mudconf.sacadjust)

/* !!! added for recycling, return value of object */
#define OBJECT_DEPOSIT(pennies)                                                \
  (((pennies) - mudconf.sacadjust) * mudconf.sacfactor)

#define DEV_NULL "/dev/null"
#define READ read
#define WRITE write

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
   ((x) ? malloc_count--, free((x)), (x) = NULL : (x)))
#else
#define XMALLOC(x, y) (char *)malloc((x))
#define XFREE(x, y) (free((x)), (x) = NULL)
#endif /* TEST_MALLOC */

#ifdef ENTERLEAVE_PARANOID
#define ENTER_REQUIRES_LEAVESUCC /* Enter checks leaveloc of player's          \
                                    origin */
#define LEAVE_REQUIRES_ENTERSUCC /* Leave checks enterlock of player's         \
                                    origin */
#endif

#include <sys/socket.h>
