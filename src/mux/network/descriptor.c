#include "mux/commands/command_context.h"
#include "mux/server/runtime_clock.h" // IWYU pragma: keep
#include "mux/support/owned_text.h"
#include "mux/world/world_context.h"
/* Descriptor lifecycle, traversal, and shutdown implementations. */

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <uv.h>

#include "mux/network/connection_events.h"
#include "mux/network/descriptor.h"
#include "mux/network/input_flow.h"
#include "mux/network/telnet_environment.h"
#include "mux/network/telnet_handler.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/diagnostics.h"
#include "mux/server/file_cache.h"
#include "mux/server/log.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/server/server_control.h"
#include "mux/support/checked_storage.h"

/* Human-readable labels for DescriptorShutdownReason values. */
static const char *const DESCRIPTOR_DISCONNECT_REASONS[] = {
    "Unspecified",
    "Quit",
    "Inactivity Timeout",
    "Booted",
    "Remote Close or Net Failure",
    "Game Shutdown",
    "Login Retry Limit",
    "Logins Disabled",
    "Too Many Connected Players"};

/* Lua on_player_disconnect reason strings for shutdown reason values. */
static const char *const DESCRIPTOR_DISCONNECT_MESSAGES[] = {
    "unknown",  "quit",     "timeout",  "boot",    "netdeath",
    "shutdown", "badlogin", "nologins", "gamefull"};

/* Number of slots allocated when the descriptor registry first grows. */
constexpr size_t DESCRIPTOR_REGISTRY_INITIAL_CAPACITY = 16;

/* Flat storage for every descriptor owned by the server event loop. */
struct DescriptorRegistry {
  CommandRuntime *runtime;
  BtechContext *btech;
  ServerLog *log;
  /* Stable slots containing descriptors or nullptr when unused. */
  Descriptor **slots;
  /* Number of slots allocated in slots. */
  size_t capacity;
  /* Number of non-null descriptors in slots. */
  size_t count;
};

static Descriptor **descriptor_registry_slot(DescriptorRegistry *registry,
                                             size_t index) {
  return (Descriptor **)checked_storage_at((void *)registry->slots,
                                           registry->capacity,
                                           sizeof(*registry->slots), index);
}

static const char *
descriptor_shutdown_reason_at(DescriptorShutdownReason reason) {
  const size_t COUNT = sizeof(DESCRIPTOR_DISCONNECT_REASONS) /
                       sizeof(*DESCRIPTOR_DISCONNECT_REASONS);

  return *(const char *const *)checked_storage_at_const(
      (const void *)DESCRIPTOR_DISCONNECT_REASONS, COUNT,
      sizeof(*DESCRIPTOR_DISCONNECT_REASONS), (size_t)reason);
}

static const char *
descriptor_shutdown_message_at(DescriptorShutdownReason reason) {
  const size_t COUNT = sizeof(DESCRIPTOR_DISCONNECT_MESSAGES) /
                       sizeof(*DESCRIPTOR_DISCONNECT_MESSAGES);

  return *(const char *const *)checked_storage_at_const(
      (const void *)DESCRIPTOR_DISCONNECT_MESSAGES, COUNT,
      sizeof(*DESCRIPTOR_DISCONNECT_MESSAGES), (size_t)reason);
}

/* Clear buffered input before destroying descriptor. */
static void descriptor_clear_input(Descriptor *descriptor) {
  descriptor->input_tail = 0;
  memset(descriptor->input, 0, sizeof(descriptor->input));
}

/* Grow the descriptor registry enough to hold at least one more entry. */
static bool descriptor_registry_grow(DescriptorRegistry *registry) {
  Descriptor **slots;
  size_t old_capacity = registry->capacity;
  size_t new_capacity = old_capacity == 0 ? DESCRIPTOR_REGISTRY_INITIAL_CAPACITY
                                          : old_capacity * 2;

  if (new_capacity < old_capacity)
    return false;
  slots = (Descriptor **)checked_storage_try_reallocate_array(
      (void *)registry->slots, new_capacity, sizeof(*registry->slots));
  if (slots == nullptr)
    return false;
  memset(checked_storage_region((void *)slots, new_capacity * sizeof(*slots),
                                old_capacity * sizeof(*slots),
                                (new_capacity - old_capacity) * sizeof(*slots)),
         0, (new_capacity - old_capacity) * sizeof(*slots));
  registry->slots = slots;
  registry->capacity = new_capacity;
  return true;
}

DescriptorRegistry *descriptor_registry_create(CommandRuntime *runtime,
                                               BtechContext *btech,
                                               ServerLog *log) {
  DescriptorRegistry *registry =
      checked_storage_try_allocate_array(1, sizeof(*registry));
  if (registry != nullptr) {
    registry->runtime = runtime;
    registry->btech = btech;
    registry->log = log;
  }
  return registry;
}

CommandRuntime *descriptor_runtime(Descriptor *descriptor) {
  return descriptor->registry->runtime;
}

BtechContext *descriptor_btech(Descriptor *descriptor) {
  return descriptor->registry->btech;
}

ServerLog *descriptor_log(Descriptor *descriptor) {
  return descriptor->registry->log;
}

void descriptor_registry_destroy(DescriptorRegistry *registry) {
  if (registry == nullptr)
    return;
  DASSERT(registry->count == 0);
  free((void *)registry->slots);
  free(registry);
}

/* Add descriptor to the flat registry and retain its active reference. */
bool descriptor_register(DescriptorRegistry *registry, Descriptor *descriptor) {
  size_t slot;

  for (slot = 0; slot < registry->capacity; slot++) {
    if (*descriptor_registry_slot(registry, slot) == nullptr)
      break;
  }
  if (slot == registry->capacity && !descriptor_registry_grow(registry))
    return false;
  *descriptor_registry_slot(registry, slot) = descriptor;
  registry->count++;
  descriptor->registry = registry;
  descriptor_retain(descriptor);
  return true;
}

/* Remove descriptor from the flat registry before destroying it. */
static void descriptor_unregister(Descriptor *descriptor) {
  DescriptorRegistry *registry = descriptor->registry;
  size_t slot;

  for (slot = 0; slot < registry->capacity; slot++) {
    Descriptor **stored = descriptor_registry_slot(registry, slot);

    if (*stored != descriptor)
      continue;
    *stored = nullptr;
    registry->count--;
    descriptor->registry = nullptr;
    return;
  }
  DASSERT(false);
}

/* Return the number of descriptors in the flat registry. */
size_t descriptor_count(const DescriptorRegistry *registry) {
  return registry->count;
}

/* Construct a descriptor iterator with the requested selection rule. */
static DescriptorIterator
descriptor_iterator_create(DescriptorRegistry *registry,
                           DescriptorIteratorKind kind, DbRef player) {
  return (DescriptorIterator){
      .registry = registry, .next_slot = 0, .kind = kind, .player = player};
}

/* Start an iterator over every registered descriptor. */
DescriptorIterator descriptor_iterator_all(DescriptorRegistry *registry) {
  return descriptor_iterator_create(registry, DESCRIPTOR_ITERATOR_ALL, NOTHING);
}

/* Start an iterator over live authenticated descriptors. */
DescriptorIterator descriptor_iterator_connected(DescriptorRegistry *registry) {
  return descriptor_iterator_create(registry, DESCRIPTOR_ITERATOR_CONNECTED,
                                    NOTHING);
}

/* Start an iterator over live authenticated descriptors for player. */
DescriptorIterator descriptor_iterator_player(DescriptorRegistry *registry,
                                              DbRef player) {
  return descriptor_iterator_create(registry, DESCRIPTOR_ITERATOR_PLAYER,
                                    player);
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

  while (iterator->next_slot < iterator->registry->capacity) {
    descriptor =
        *descriptor_registry_slot(iterator->registry, iterator->next_slot++);
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
  DASSERT(descriptor->refcount > 0);
  descriptor->refcount--;
  if (descriptor->refcount != 0)
    return;
  if (descriptor->is_socket_closed)
    descriptor_free(descriptor);
  else
    descriptor_force_close(descriptor);
}

/* Find an authenticated descriptor by its socket descriptor. */
Descriptor *descriptor_find_by_fd(DescriptorRegistry *registry, int fd) {
  Descriptor *descriptor;
  DescriptorIterator iterator = descriptor_iterator_connected(registry);

  while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr) {
    if (descriptor->descriptor == fd)
      return descriptor;
  }
  return nullptr;
}

/* Disconnect descriptor and notify the game of its shutdown reason. */
void descriptor_shutdown(Descriptor *descriptor,
                         DescriptorShutdownReason reason) {
  CommandRuntime *runtime = descriptor_runtime(descriptor);
  if (descriptor->is_dead)
    return;
  descriptor->is_dead = true;
  uv_read_stop((uv_stream_t *)descriptor->socket);
  if (descriptor->is_connected) {
    OwnedText flags;

    fcache_dump(runtime->files, descriptor, FC_QUIT);
    log_error((LogEntry){.log = descriptor_log(descriptor),
                         .key = LOG_NET | LOG_LOGIN,
                         .primary = "NET",
                         .secondary = "DISC"},
              "[%d/%s] Logout by %s(#%ld), <Reason: %s>",
              descriptor->descriptor, descriptor->addr,
              game_object_name(runtime->world->database, descriptor->player),
              descriptor->player, descriptor_shutdown_reason_at(reason));

    flags = unparse_flags(descriptor_runtime(descriptor)->world->database, GOD,
                          descriptor->player);
    log_error(
        (LogEntry){.log = descriptor_log(descriptor),
                   .key = LOG_ACCOUNTING,
                   .primary = "DIS",
                   .secondary = "ACCT"},
        "%ld %s %d %ld %ld [%s] <%s> %s", descriptor->player, flags.text,
        descriptor->command_count,
        runtime->clock->now - descriptor->connected_at,
        game_object_location(descriptor_runtime(descriptor)->world->database,
                             descriptor->player),
        descriptor->addr, descriptor_shutdown_reason_at(reason),
        game_object_name(runtime->world->database, descriptor->player));
    owned_text_release(&flags);

    descriptor_announce_disconnect(descriptor->player, descriptor,
                                   descriptor_shutdown_message_at(reason));
  }
  descriptor_release(descriptor); // NOLINT(clang-analyzer-unix.Malloc)
}
