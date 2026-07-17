/* Descriptor lifecycle, traversal, and shutdown implementations. */

#include "mux/server/platform.h"

#include "mux/network/descriptor.h"

#include "mux/server/libuv.h"
#include <stdint.h>
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
    "Unspecified",
    "Quit",
    "Inactivity Timeout",
    "Booted",
    "Remote Close or Net Failure",
    "Game Shutdown",
    "Login Retry Limit",
    "Logins Disabled",
    "Too Many Connected Players"};

/* A_ADISCONNECT reason strings for DescriptorShutdownReason values. */
static const char *descriptor_disconnect_messages[] = {
    "unknown",  "quit",     "timeout",  "boot",    "netdeath",
    "shutdown", "badlogin", "nologins", "gamefull"};

/* Number of slots allocated when the descriptor registry first grows. */
constexpr size_t DESCRIPTOR_REGISTRY_INITIAL_CAPACITY = 16;

/* Flat storage for every descriptor owned by the server event loop. */
typedef struct DescriptorRegistry {
  /* Stable slots containing descriptors or nullptr when unused. */
  Descriptor **slots;
  /* Number of slots allocated in slots. */
  size_t capacity;
  /* Number of non-null descriptors in slots. */
  size_t count;
} DescriptorRegistry;

/* Process-wide registry of descriptors that have completed setup. */
static DescriptorRegistry descriptor_registry;

/* Clear buffered input before destroying descriptor. */
static void descriptor_clear_input(Descriptor *descriptor) {
  descriptor->input_tail = 0;
  memset(descriptor->input, 0, sizeof(descriptor->input));
}

/* Grow the descriptor registry enough to hold at least one more entry. */
static bool descriptor_registry_grow(void) {
  Descriptor **slots;
  size_t old_capacity = descriptor_registry.capacity;
  size_t new_capacity = old_capacity == 0 ? DESCRIPTOR_REGISTRY_INITIAL_CAPACITY
                                          : old_capacity * 2;

  if (new_capacity < old_capacity ||
      new_capacity > SIZE_MAX / sizeof(*descriptor_registry.slots))
    return false;
  slots = realloc(descriptor_registry.slots,
                  new_capacity * sizeof(*descriptor_registry.slots));
  if (slots == nullptr)
    return false;
  memset(&slots[old_capacity], 0,
         (new_capacity - old_capacity) * sizeof(*slots));
  descriptor_registry.slots = slots;
  descriptor_registry.capacity = new_capacity;
  return true;
}

/* Add descriptor to the flat registry and retain its active reference. */
bool descriptor_register(Descriptor *descriptor) {
  size_t slot;

  for (slot = 0; slot < descriptor_registry.capacity; slot++) {
    if (descriptor_registry.slots[slot] == nullptr)
      break;
  }
  if (slot == descriptor_registry.capacity && !descriptor_registry_grow())
    return false;
  descriptor_registry.slots[slot] = descriptor;
  descriptor_registry.count++;
  descriptor_retain(descriptor);
  return true;
}

/* Remove descriptor from the flat registry before destroying it. */
static void descriptor_unregister(Descriptor *descriptor) {
  size_t slot;

  for (slot = 0; slot < descriptor_registry.capacity; slot++) {
    if (descriptor_registry.slots[slot] != descriptor)
      continue;
    descriptor_registry.slots[slot] = nullptr;
    descriptor_registry.count--;
    return;
  }
  dassert(false);
}

/* Return the number of descriptors in the flat registry. */
size_t descriptor_count(void) { return descriptor_registry.count; }

/* Construct a descriptor iterator with the requested selection rule. */
static DescriptorIterator
descriptor_iterator_create(DescriptorIteratorKind kind, DbRef player) {
  return (DescriptorIterator){.next_slot = 0, .kind = kind, .player = player};
}

/* Start an iterator over every registered descriptor. */
DescriptorIterator descriptor_iterator_all(void) {
  return descriptor_iterator_create(DESCRIPTOR_ITERATOR_ALL, NOTHING);
}

/* Start an iterator over live authenticated descriptors. */
DescriptorIterator descriptor_iterator_connected(void) {
  return descriptor_iterator_create(DESCRIPTOR_ITERATOR_CONNECTED, NOTHING);
}

/* Start an iterator over live authenticated descriptors for player. */
DescriptorIterator descriptor_iterator_player(DbRef player) {
  return descriptor_iterator_create(DESCRIPTOR_ITERATOR_PLAYER, player);
}

/* Return whether descriptor satisfies iterator's selection rule. */
static bool descriptor_iterator_matches(const DescriptorIterator *iterator,
                                        const Descriptor *descriptor) {
  if (iterator->kind == DESCRIPTOR_ITERATOR_ALL)
    return true;
  if (!descriptor->is_connected || descriptor->is_dead)
    return false;
  if (iterator->kind == DESCRIPTOR_ITERATOR_PLAYER)
    return descriptor->player == iterator->player;
  return true;
}

/* Return the next descriptor selected by iterator, or nullptr at the end. */
Descriptor *descriptor_iterator_next(DescriptorIterator *iterator) {
  Descriptor *descriptor;

  while (iterator->next_slot < descriptor_registry.capacity) {
    descriptor = descriptor_registry.slots[iterator->next_slot++];
    if (descriptor != nullptr &&
        descriptor_iterator_matches(iterator, descriptor))
      return descriptor;
  }
  return nullptr;
}

/* Retain descriptor against destruction. */
void descriptor_retain(Descriptor *descriptor) { descriptor->refcount++; }

/* Destroy descriptor after all references and socket ownership have ended. */
static void descriptor_free(Descriptor *descriptor) {
  dprintk("%p destructing", descriptor);
  descriptor_clear_input(descriptor);
  descriptor_flow_destroy(descriptor);
  descriptor_telnet_destroy(descriptor);
  descriptor_unregister(descriptor);
  free(descriptor);
}

/* Complete socket teardown and destroy an otherwise unreferenced descriptor. */
static void descriptor_socket_closed(uv_handle_t *handle) {
  Descriptor *descriptor = handle->data;

  free(handle);
  descriptor->socket = nullptr;
  descriptor->is_socket_closed = true;
  descriptor->descriptor = 0;
  if (descriptor->refcount == 0)
    descriptor_free(descriptor);
}

/* Force libuv to cancel pending I/O and close this descriptor. */
void descriptor_force_close(Descriptor *descriptor) {
  if (descriptor->is_socket_closing)
    return;
  descriptor->is_socket_closing = true;
  uv_read_stop((uv_stream_t *)descriptor->socket);
  uv_close((uv_handle_t *)descriptor->socket, descriptor_socket_closed);
}

/* Release a retained descriptor and destroy it when no references remain. */
void descriptor_release(Descriptor *descriptor) {
  dassert(descriptor->refcount > 0);
  descriptor->refcount--;
  if (descriptor->refcount != 0)
    return;
  if (descriptor->is_socket_closed)
    descriptor_free(descriptor);
  else
    descriptor_force_close(descriptor);
}

/* Find an authenticated descriptor by its socket descriptor. */
Descriptor *descriptor_find_by_fd(int fd) {
  Descriptor *descriptor;
  DescriptorIterator iterator = descriptor_iterator_connected();

  while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr) {
    if (descriptor->descriptor == fd)
      return descriptor;
  }
  return nullptr;
}

/* Disconnect descriptor and notify the game of its shutdown reason. */
void descriptor_shutdown(Descriptor *descriptor,
                         DescriptorShutdownReason reason) {
  if (descriptor->is_dead)
    return;
  descriptor->is_dead = true;
  uv_read_stop((uv_stream_t *)descriptor->socket);
  if (descriptor->is_connected) {
    fcache_dump(descriptor, FC_QUIT);
    log_error(LOG_NET | LOG_LOGIN, "NET", "DISC",
              "[%d/%s] Logout by %s(#%ld), <Reason: %s>",
              descriptor->descriptor, descriptor->addr,
              Name(descriptor->player), descriptor->player,
              descriptor_disconnect_reasons[reason]);

    log_error(LOG_ACCOUNTING, "DIS", "ACCT", "%ld %s %d %ld %ld [%s] <%s> %s",
              descriptor->player,
              decode_flags(GOD, obj_flags(descriptor->player),
                           obj_flags2(descriptor->player),
                           obj_flags3(descriptor->player)),
              descriptor->command_count,
              mudstate.now - descriptor->connected_at,
              obj_location(descriptor->player), descriptor->addr,
              descriptor_disconnect_reasons[reason], Name(descriptor->player));

    descriptor_announce_disconnect(descriptor->player, descriptor,
                                   descriptor_disconnect_messages[reason]);
  }
  descriptor_release(descriptor); // NOLINT(clang-analyzer-unix.Malloc)
}
