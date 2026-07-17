/* Descriptor lifecycle, traversal, and shutdown implementations. */

#include "mux/server/platform.h"

#include "mux/network/descriptor.h"

#include <event2/buffer.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include "mux/database/flags.h"
#include "mux/network/input_flow.h"
#include "mux/network/netcommon.h"
#include "mux/network/telnet_handler.h"
#include "mux/server/diagnostics.h"
#include "mux/server/file_cache.h"
#include "mux/server/log.h"
#include "mux/server/server_state.h"

/* Human-readable labels for DescriptorShutdownReason values. */
static const char *descriptor_disconnect_reasons[] = {
    "Unspecified", "Quit",          "Inactivity Timeout",
    "Booted",      "Remote Close or Net Failure", "Game Shutdown",
    "Login Retry Limit", "Logins Disabled", "Too Many Connected Players"};

/* A_ADISCONNECT reason strings for DescriptorShutdownReason values. */
static const char *descriptor_disconnect_messages[] = {
    "unknown", "quit",     "timeout", "boot",     "netdeath",
    "shutdown", "badlogin", "nologins", "gamefull"};

/* Head of the global list of active descriptors. */
Descriptor *descriptor_list = nullptr;
/* Number of active descriptors in the global descriptor list. */
int ndescriptors = 0;

/* Clear buffered input before destroying descriptor. */
static void descriptor_clear_input(Descriptor *descriptor) {
  descriptor->input_tail = 0;
  memset(descriptor->input, 0, sizeof(descriptor->input));
}

/* Remove descriptor from the per-player descriptor hash chain. */
static void descriptor_hash_remove(Descriptor *descriptor) {
  Descriptor *hashed_descriptor;
  char buffer[4096];

  hashed_descriptor =
      red_black_tree_find(mudstate.desctree, (void *)descriptor->player);
  if (hashed_descriptor == nullptr) {
    snprintf(buffer, sizeof(buffer),
             "descriptor_hash_remove: unable to find player(%ld)'s "
             "descriptors from hashtable.\n",
             descriptor->player);
    log_text(buffer);
    return;
  }

  if (hashed_descriptor == descriptor && hashed_descriptor->hashnext) {
    red_black_tree_insert(mudstate.desctree, (void *)descriptor->player,
                          descriptor->hashnext);
    descriptor->hashnext = nullptr;
    descriptor_release(descriptor);
    return;
  }
  if (hashed_descriptor == descriptor) {
    red_black_tree_delete(mudstate.desctree, (void *)descriptor->player);
    descriptor_release(descriptor);
    return;
  }

  while (hashed_descriptor->hashnext != nullptr) {
    if (hashed_descriptor->hashnext == descriptor) {
      hashed_descriptor->hashnext = descriptor->hashnext;
      descriptor->hashnext = nullptr;
      descriptor_release(descriptor);
      return;
    }
    hashed_descriptor = hashed_descriptor->hashnext;
  }
}

/* Compare two player database references for the descriptor hash tree. */
int descriptor_compare(void *vleft, void *vright, void *token) {
  DbRef left = (DbRef)vleft;
  DbRef right = (DbRef)vright;

  return (left > right) - (left < right);
}

/* Add descriptor to the descriptor hash chain for its player. */
void descriptor_hash_add(Descriptor *descriptor) {
  Descriptor *hashed_descriptor;

  descriptor_retain(descriptor);
  hashed_descriptor =
      red_black_tree_find(mudstate.desctree, (void *)descriptor->player);
  descriptor->hashnext = hashed_descriptor;
  red_black_tree_insert(mudstate.desctree, (void *)descriptor->player,
                        descriptor);
}

/* Retain descriptor against destruction. */
void descriptor_retain(Descriptor *descriptor) { descriptor->refcount++; }

/* Release a retained descriptor and destroy it when no references remain. */
void descriptor_release(Descriptor *descriptor) {
  descriptor->refcount--;
  if (descriptor->refcount == 0) {
    dprintk("%p destructing", descriptor);
    descriptor_clear_input(descriptor);

    descriptor_flow_destroy(descriptor);
    if (descriptor->descriptor) {
      fsync(descriptor->descriptor);
      event_free(descriptor->sock_ev);
      descriptor->sock_ev = nullptr;
      shutdown(descriptor->descriptor, 2);
      close(descriptor->descriptor);
    }
    descriptor->descriptor = 0;
    descriptor_telnet_destroy(descriptor);
    if (descriptor->sock_buff)
      bufferevent_free(descriptor->sock_buff);
    descriptor->sock_buff = nullptr;

    if (descriptor->prev) {
      descriptor->prev->next = descriptor->next;
    } else {
      descriptor_list = descriptor->next;
    }
    if (descriptor->next)
      descriptor->next->prev = descriptor->prev;

    ndescriptors--;
    free(descriptor);
  }
}

/* Return the first descriptor in the global descriptor list. */
Descriptor *descriptor_first(void) { return descriptor_list; }

/* Return the descriptor following descriptor in the global list. */
Descriptor *descriptor_next(Descriptor *descriptor) {
  return descriptor->next;
}

/* Find the first authenticated descriptor in the global list. */
Descriptor *descriptor_first_connected(void) {
  Descriptor *descriptor;

  for (descriptor = descriptor_first(); descriptor != nullptr;
       descriptor = descriptor_next(descriptor)) {
    if (descriptor->is_connected)
      return descriptor;
  }
  return nullptr;
}

/* Find the next authenticated descriptor after descriptor. */
Descriptor *descriptor_next_connected(Descriptor *descriptor) {
  for (descriptor = descriptor_next(descriptor); descriptor != nullptr;
       descriptor = descriptor_next(descriptor)) {
    if (descriptor->is_connected)
      return descriptor;
  }
  return nullptr;
}

/* Find the first descriptor associated with player. */
Descriptor *descriptor_first_player(DbRef player) {
  return red_black_tree_find(mudstate.desctree, (void *)player);
}

/* Return the next descriptor associated with descriptor's player. */
Descriptor *descriptor_next_player(Descriptor *descriptor) {
  return descriptor->hashnext;
}

/* Find an authenticated descriptor by its socket descriptor. */
Descriptor *descriptor_find_by_fd(int fd) {
  Descriptor *descriptor;

  for (descriptor = descriptor_first_connected(); descriptor != nullptr;
       descriptor = descriptor_next_connected(descriptor)) {
    if (descriptor->descriptor == fd)
      return descriptor;
  }
  return nullptr;
}

/* Disconnect descriptor and notify the game of its shutdown reason. */
void descriptor_shutdown(Descriptor *descriptor,
                         DescriptorShutdownReason reason) {
  descriptor->is_dead = true;
  if (descriptor->is_connected) {
    fcache_dump(descriptor, FC_QUIT);
    if (reason == DESCRIPTOR_SHUTDOWN_QUIT && descriptor->sock_buff != nullptr &&
        evbuffer_get_length(bufferevent_get_output(descriptor->sock_buff)) >
            0) {
      descriptor->is_flush_before_close = true;
      descriptor_retain(descriptor);
      event_del(descriptor->sock_ev);
    }

    log_error(LOG_NET | LOG_LOGIN, "NET", "DISC",
              "[%d/%s] Logout by %s(#%ld), <Reason: %s>",
              descriptor->descriptor, descriptor->addr, Name(descriptor->player),
              descriptor->player, descriptor_disconnect_reasons[reason]);

    log_error(LOG_ACCOUNTING, "DIS", "ACCT", "%ld %s %d %ld %ld [%s] <%s> %s",
              descriptor->player,
              decode_flags(GOD, obj_flags(descriptor->player),
                           obj_flags2(descriptor->player),
                           obj_flags3(descriptor->player)),
              descriptor->command_count,
              mudstate.now - descriptor->connected_at,
              obj_location(descriptor->player), descriptor->addr,
              descriptor_disconnect_reasons[reason], Name(descriptor->player));

    descriptor_announce_disconnect(
        descriptor->player, descriptor,
        descriptor_disconnect_messages[reason]);
    descriptor_hash_remove(descriptor);
  }
  descriptor_release(descriptor); // NOLINT(clang-analyzer-unix.Malloc)
}
