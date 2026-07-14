/* Shared state for an active client connection. */

#pragma once

#include <event.h>
#include <time.h>

#include "alloc.h"
#include "db.h"
#include "rbtab.h"

/* Reasons passed to shutdownsock(). */
#define R_QUIT 1       /* User quit */
#define R_TIMEOUT 2    /* Inactivity timeout */
#define R_BOOT 3       /* Victim of @boot or @destroy */
#define R_SOCKDIED 4   /* Other end of socket closed it */
#define R_GOING_DOWN 5 /* Game is going down */
#define R_BADLOGIN 6   /* Too many failed login attempts */
#define R_GAMEDOWN 7   /* Not admitting users now */
#define R_GAMEFULL 8   /* Too many players logged in */

typedef struct telnet_t telnet_t;

typedef struct prog_data PROG;
struct prog_data {
  dbref wait_cause;
  char *wait_regs[MAX_GLOBAL_REGS];
};

#define DOINGLEN 45

typedef struct descriptor_data DESC;
struct descriptor_data {
  int descriptor;
  int flags;
  int retries_left;
  int command_count;
  int timeout;
  int host_info;
  char addr[256];
  char username[11];
  char doing[DOINGLEN];
  dbref player;
  char *output_prefix;
  char *output_suffix;
  int output_size;
  int output_tot;
  int output_lost;
  int input_size;
  int input_tot;
  int input_lost;
  char input[LBUF_SIZE];
  int input_tail;
  telnet_t *telnet;
  char terminal_type[16];
  int terminal_width;
  int terminal_height;
  int gmcp_enabled;
  int charset_ascii;
  int charset_request_pending;
  time_t connected_at;
  time_t last_time;
  int quota;
  int refcount;
  int wait_for_input; /* Used by @prog */
  dbref wait_cause;   /* Used by @prog */
  PROG *program_data;
  struct descriptor_data *hashnext;
  struct descriptor_data *next;
  struct descriptor_data *prev;
  struct event sock_ev;
  struct bufferevent *sock_buff;
};

/* Flags in DESC.flags. */
#define DS_CONNECTED 0x0001 /* Player is connected. */
#define DS_AUTODARK 0x0002  /* Wizard was automatically set dark. */
#define DS_IDENTIFIED 0x0008
#define DS_DEAD 0x10000000 /* Socket has disconnected; ignore it. */

extern DESC *descriptor_list;

#define DESC_ITER_PLAYER(p, d)                                                 \
  for (d = (DESC *)rb_find(mudstate.desctree, (void *)p); d; d = d->hashnext)

#define DESC_ITER_CONN(d)                                                      \
  for (d = descriptor_list; (d); d = (d)->next)                                \
    if ((d)->flags & DS_CONNECTED)

#define DESC_ITER_ALL(d) for (d = descriptor_list; (d); d = (d)->next)

#define DESC_SAFEITER_PLAYER(p, d, n)                                          \
  for (d = (DESC *)rb_find(mudstate.desctree, (void *)p),                      \
      n = ((d != NULL) ? d->hashnext : NULL);                                  \
       d; d = n, n = ((n != NULL) ? n->hashnext : NULL))

#define DESC_SAFEITER_CONN(d, n)                                               \
  for (d = descriptor_list, n = ((d != NULL) ? d->next : NULL); d;             \
       d = n, n = ((n != NULL) ? n->next : NULL))                              \
    if ((d)->flags & DS_CONNECTED)

#define DESC_SAFEITER_ALL(d, n)                                                \
  for (d = descriptor_list, n = ((d != NULL) ? d->next : NULL); d;             \
       d = n, n = ((n != NULL) ? n->next : NULL))
