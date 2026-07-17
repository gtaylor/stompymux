/* Shared state for an active client connection. */

#pragma once

#include <event2/bufferevent.h>
#include <event2/event.h>
#include <time.h>

#include "mux/database/db.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"

/* Reasons passed to descriptor_shutdown(). */
constexpr int R_QUIT = 1;       /* User quit */
constexpr int R_TIMEOUT = 2;    /* Inactivity timeout */
constexpr int R_BOOT = 3;       /* Victim of @boot or @destroy */
constexpr int R_SOCKDIED = 4;   /* Other end of socket closed it */
constexpr int R_GOING_DOWN = 5; /* Game is going down */
constexpr int R_BADLOGIN = 6;   /* Too many failed login attempts */
constexpr int R_GAMEDOWN = 7;   /* Not admitting users now */
constexpr int R_GAMEFULL = 8;   /* Too many players logged in */

typedef struct telnet_t telnet_t;
typedef struct InputFlow InputFlow;

typedef struct Descriptor Descriptor;
struct Descriptor {
  int descriptor;
  int flags;
  int retries_left;
  int command_count;
  int timeout;
  int host_info;
  char addr[256];
  char username[11];
  DbRef player;
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
  int mccp_enabled;
  int charset_ascii;
  int charset_request_pending;
  time_t connected_at;
  time_t last_time;
  int quota;
  int refcount;
  InputFlow *flow;
  struct Descriptor *hashnext;
  struct Descriptor *next;
  struct Descriptor *prev;
  struct event *sock_ev;
  struct bufferevent *sock_buff;
};

/* Flags in DESC.flags. */
constexpr int DS_CONNECTED = 0x0001; /* Player is connected. */
constexpr int DS_AUTODARK = 0x0002;  /* Wizard was automatically set dark. */
constexpr int DS_IDENTIFIED = 0x0008;
constexpr int DS_DEAD = 0x10000000; /* Socket has disconnected; ignore it. */

extern Descriptor *descriptor_list;

#define DESC_ITER_PLAYER(p, d)                                                 \
  for (d = (Descriptor *)red_black_tree_find(mudstate.desctree, (void *)p); d; \
       d = d->hashnext)

#define DESC_ITER_CONN(d)                                                      \
  for (d = descriptor_list; (d); d = (d)->next)                                \
    if ((d)->flags & DS_CONNECTED)

#define DESC_ITER_ALL(d) for (d = descriptor_list; (d); d = (d)->next)

#define DESC_SAFEITER_PLAYER(p, d, n)                                          \
  for (d = (Descriptor *)red_black_tree_find(mudstate.desctree, (void *)p),    \
      n = ((d != nullptr) ? d->hashnext : nullptr);                            \
       d; d = n, n = ((n != nullptr) ? n->hashnext : nullptr))

#define DESC_SAFEITER_CONN(d, n)                                               \
  for (d = descriptor_list, n = ((d != nullptr) ? d->next : nullptr); d;       \
       d = n, n = ((n != nullptr) ? n->next : nullptr))                        \
    if ((d)->flags & DS_CONNECTED)

#define DESC_SAFEITER_ALL(d, n)                                                \
  for (d = descriptor_list, n = ((d != nullptr) ? d->next : nullptr); d;       \
       d = n, n = ((n != nullptr) ? n->next : nullptr))
