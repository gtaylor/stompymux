/* Descriptor state and lifecycle interfaces for active client connections. */

#pragma once

#include <time.h>

#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/styled_text/render.h"

// IWYU pragma: no_include "context.h"
// IWYU pragma: no_include "mux/commands/command_runtime.h"
// IWYU pragma: no_include "mux/server/log.h"
// IWYU pragma: no_include "uv.h"

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

/* Opaque libtelnet protocol state owned by a descriptor. */
typedef struct telnet_t TelnetT;
typedef struct TelnetEnvironment TelnetEnvironment;
/* Opaque interactive input-flow state owned by a descriptor. */
typedef struct InputFlow InputFlow;
/* Opaque libuv TCP handle owned by a descriptor. */
// NOLINTNEXTLINE(readability-identifier-naming) — external libuv tag.
typedef struct uv_tcp_s UvTcpT;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct BtechContext BtechContext;
typedef struct CommandRuntime CommandRuntime;
typedef struct ServerLog ServerLog;

/* Runtime state and resources for one client connection. */
typedef struct Descriptor {
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
  /* Whether libuv has started closing this socket. */
  bool is_socket_closing;
  /* Whether libuv has completed closing this socket. */
  bool is_socket_closed;
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
  TelnetT *telnet;
  /* Environment variables reported through RFC 1572 NEW-ENVIRON. */
  TelnetEnvironment *telnet_environment;
  /* Terminal type reported by the client. */
  char terminal_type[64];
  /* MUD client name reported by the first MTTS TTYPE response. */
  char terminal_client[64];
  /* Number of TTYPE responses received during MTTS discovery. */
  int terminal_type_responses;
  /* Maximum color depth advertised by this connection. */
  TerminalColorDepth terminal_color_depth;
  /* Whether MTTS reports that the client is a screen reader. */
  bool is_screen_reader;
  /* Whether this connection overrides automatic color-depth selection. */
  bool has_color_override;
  /* Color depth explicitly selected for this connection. */
  TerminalColorDepth color_override;
  /* Client terminal width in character cells. */
  int terminal_width;
  /* Client terminal height in character cells. */
  int terminal_height;
  /* Whether the client agreed to terminal-type negotiation. */
  bool is_ttype_enabled;
  /* Whether the client agreed to window-size negotiation. */
  bool is_naws_enabled;
  /* Whether the client agreed to NEW-ENVIRON negotiation. */
  bool is_new_environ_enabled;
  /* Whether this connection has received its OSC 8 preset definitions. */
  bool osc8_presets_emitted;
  /* Whether the client requested the server-status response. */
  bool is_mssp_enabled;
  /* Whether the client enabled the GMCP telnet option. */
  bool is_gmcp_enabled;
  /* Whether MCCP output compression is active. */
  bool is_mccp_enabled;
  /* Whether the client accepted UTF-8 character-set negotiation. */
  bool is_charset_utf8;
  /* Whether the client agreed to character-set negotiation. */
  bool is_charset_enabled;
  /* Whether a character-set negotiation request awaits a response. */
  bool is_charset_request_pending;
  /* Whether the server is suppressing client-side echo for this connection. */
  bool is_echo_suppressed;
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
  /* Number of asynchronous writes still owned by libuv. */
  size_t pending_writes;
  /* libuv TCP stream for this client. */
  UvTcpT *socket;
  /* Registry that owns this descriptor's active reference. */
  DescriptorRegistry *registry;
} Descriptor;

/* Kinds of descriptor selected by a DescriptorIterator. */
typedef enum DescriptorIteratorKind {
  /* Select every descriptor, including descriptors that are closing. */
  DESCRIPTOR_ITERATOR_ALL,
  /* Select authenticated descriptors that are not closing. */
  DESCRIPTOR_ITERATOR_CONNECTED,
  /* Select live authenticated descriptors for one player. */
  DESCRIPTOR_ITERATOR_PLAYER,
} DescriptorIteratorKind;

/* Cursor for a single pass through the flat descriptor registry. */
typedef struct DescriptorIterator {
  /* Registry traversed by this iterator. */
  DescriptorRegistry *registry;
  /* Registry slot to examine on the next call. */
  size_t next_slot;
  /* Selection rule applied while advancing the iterator. */
  DescriptorIteratorKind kind;
  /* Player selected by DESCRIPTOR_ITERATOR_PLAYER. */
  DbRef player;
} DescriptorIterator;

DescriptorRegistry *descriptor_registry_create(CommandRuntime *runtime,
                                               BtechContext *btech,
                                               ServerLog *log);
CommandRuntime *descriptor_runtime(Descriptor *descriptor);
BtechContext *descriptor_btech(Descriptor *descriptor);
ServerLog *descriptor_log(Descriptor *descriptor);
void descriptor_registry_destroy(DescriptorRegistry *registry);
/* Add descriptor to the flat registry and retain its active reference. */
bool descriptor_register(DescriptorRegistry *registry, Descriptor *descriptor);
/* Return the number of descriptors in the flat registry. */
size_t descriptor_count(const DescriptorRegistry *registry);
/* Start an iterator over every registered descriptor. */
DescriptorIterator descriptor_iterator_all(DescriptorRegistry *registry);
/* Start an iterator over live authenticated descriptors. */
DescriptorIterator descriptor_iterator_connected(DescriptorRegistry *registry);
/* Start an iterator over live authenticated descriptors for player. */
DescriptorIterator descriptor_iterator_player(DescriptorRegistry *registry,
                                              DbRef player);
/* Return the next descriptor selected by iterator, or nullptr at the end. */
Descriptor *descriptor_iterator_next(DescriptorIterator *iterator);
/* Retain descriptor against destruction. */
void descriptor_retain(Descriptor *descriptor);
/* Release a retained descriptor and destroy it when no references remain. */
void descriptor_release(Descriptor *descriptor);
/* Find an authenticated descriptor by its socket descriptor. */
Descriptor *descriptor_find_by_fd(DescriptorRegistry *registry, int descriptor);
/* Mark descriptor disconnected and perform its shutdown lifecycle. */
void descriptor_shutdown(Descriptor *descriptor,
                         DescriptorShutdownReason reason);
/* Force libuv to cancel pending I/O and close this descriptor. */
void descriptor_force_close(Descriptor *descriptor);
