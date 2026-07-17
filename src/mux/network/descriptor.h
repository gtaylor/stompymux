/* Descriptor state and lifecycle interfaces for active client connections. */

#pragma once

#include <event2/bufferevent.h>
#include <event2/event.h>
#include <stdbool.h>
#include <time.h>

#include "mux/database/db.h"
#include "mux/support/alloc.h"

/* Reasons passed to descriptor_shutdown(). */
typedef enum DescriptorShutdownReason {
  DESCRIPTOR_SHUTDOWN_QUIT = 1,       /* User quit */
  DESCRIPTOR_SHUTDOWN_TIMEOUT = 2,    /* Inactivity timeout */
  DESCRIPTOR_SHUTDOWN_BOOT = 3,       /* Victim of @boot or @destroy */
  DESCRIPTOR_SHUTDOWN_SOCKDIED = 4,   /* Other end of socket closed it */
  DESCRIPTOR_SHUTDOWN_GOING_DOWN = 5, /* Game is going down */
  DESCRIPTOR_SHUTDOWN_BADLOGIN = 6,   /* Too many failed login attempts */
  DESCRIPTOR_SHUTDOWN_GAMEDOWN = 7,   /* Not admitting users now */
  DESCRIPTOR_SHUTDOWN_GAMEFULL = 8,   /* Too many players logged in */
} DescriptorShutdownReason;

typedef struct telnet_t telnet_t;
typedef struct InputFlow InputFlow;

typedef struct Descriptor Descriptor;
struct Descriptor {
  /* Operating-system socket descriptor for this connection. */
  int descriptor;
  /* Whether the connection has authenticated as a player. */
  bool is_connected;
  /* Whether the server automatically darkened this wizard for idling. */
  bool is_autodark;
  /* Whether the socket has disconnected and must no longer be processed. */
  bool is_dead;
  /* Remaining login attempts before the server disconnects the client. */
  int retries_left;
  /* Number of player commands received on this connection. */
  int command_count;
  /* Idle timeout, in seconds, for this connection. */
  int timeout;
  /* Access-control and suspect-site flags for the remote address. */
  int host_info;
  /* Numeric remote network address. */
  char addr[256];
  /* Remote IDENT username, when one is available. */
  char username[11];
  /* Database reference for the connected player, or zero before login. */
  DbRef player;
  /* Current size of queued output. */
  int output_size;
  /* Total number of output bytes sent during the connection. */
  int output_tot;
  /* Number of output bytes discarded because of queue limits. */
  int output_lost;
  /* Whether to release the descriptor after pending output flushes. */
  bool is_flush_before_close;
  /* Current size of buffered input. */
  int input_size;
  /* Total number of input bytes received during the connection. */
  int input_tot;
  /* Number of input bytes discarded because of input limits. */
  int input_lost;
  /* Buffered client input awaiting command or flow processing. */
  char input[LBUF_SIZE];
  /* Write position within the input buffer. */
  int input_tail;
  /* libtelnet state for protocol negotiation. */
  telnet_t *telnet;
  /* Terminal type reported by the client. */
  char terminal_type[16];
  /* Client terminal width in character cells. */
  int terminal_width;
  /* Client terminal height in character cells. */
  int terminal_height;
  /* Whether the client enabled the GMCP telnet option. */
  bool is_gmcp_enabled;
  /* Whether MCCP output compression is active. */
  bool is_mccp_enabled;
  /* Whether the negotiated client character set is US-ASCII. */
  bool is_charset_ascii;
  /* Whether a character-set negotiation request awaits a response. */
  bool is_charset_request_pending;
  /* Time when this socket connection was accepted. */
  time_t connected_at;
  /* Time when the server last received a command from the client. */
  time_t last_time;
  /* Commands this connection may run before its quota refills. */
  int quota;
  /* References that retain this descriptor against destruction. */
  int refcount;
  /* Active interactive input flow, if any. */
  InputFlow *flow;
  /* Next descriptor for the same player in the descriptor hash chain. */
  struct Descriptor *hashnext;
  /* Next descriptor in the global descriptor list. */
  struct Descriptor *next;
  /* Previous descriptor in the global descriptor list. */
  struct Descriptor *prev;
  /* libevent watcher for socket reads. */
  struct event *sock_ev;
  /* libevent buffered socket used for output. */
  struct bufferevent *sock_buff;
};

/* Head of the global list of active descriptors. */
extern Descriptor *descriptor_list;
/* Number of active descriptors in the global descriptor list. */
extern int ndescriptors;

/* Compare two player database references for the descriptor hash tree. */
int descriptor_compare(void *vleft, void *vright, void *token);
/* Add descriptor to the descriptor hash chain for its player. */
void descriptor_hash_add(Descriptor *descriptor);
/* Retain descriptor against destruction. */
void descriptor_retain(Descriptor *descriptor);
/* Release a retained descriptor and destroy it when no references remain. */
void descriptor_release(Descriptor *descriptor);
/* Return the first descriptor in the global descriptor list. */
Descriptor *descriptor_first(void);
/* Return the descriptor following descriptor in the global list. */
Descriptor *descriptor_next(Descriptor *descriptor);
/* Return the first authenticated descriptor in the global list. */
Descriptor *descriptor_first_connected(void);
/* Return the next authenticated descriptor after descriptor. */
Descriptor *descriptor_next_connected(Descriptor *descriptor);
/* Return the first descriptor associated with player. */
Descriptor *descriptor_first_player(DbRef player);
/* Return the next descriptor associated with descriptor's player. */
Descriptor *descriptor_next_player(Descriptor *descriptor);
/* Find an authenticated descriptor by its socket descriptor. */
Descriptor *descriptor_find_by_fd(int descriptor);
/* Mark descriptor disconnected and perform its shutdown lifecycle. */
void descriptor_shutdown(Descriptor *descriptor,
                         DescriptorShutdownReason reason);
