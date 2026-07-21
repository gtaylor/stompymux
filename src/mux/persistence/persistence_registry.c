/* persistence_registry.c - Registration for snapshot persistence extensions. */

#include "mux/persistence/gamedb.h"

#include <assert.h>
#include <string.h>

void persistence_context_initialize(
    PersistenceContext *context, const ServerConfiguration *configuration,
    GameDatabase *database, ChannelRegistry *channels, MacroRegistry *macros,
    time_t *now, int *record_players, WorldContext *world, ServerLog *log) {
  assert(context != nullptr);
  memset(context, 0, sizeof(*context));
  context->configuration = configuration;
  context->database = database;
  context->channels = channels;
  context->macros = macros;
  context->now = now;
  context->record_players = record_players;
  context->world = world;
  context->log = log;
}

int persistence_register_sqlite_extension(PersistenceContext *context,
                                          const char *name,
                                          PersistenceSqliteLoad load,
                                          PersistenceSqliteStore store,
                                          void *extension_context) {
  assert(context != nullptr);

  if (name == nullptr || *name == '\0' || load == nullptr || store == nullptr)
    return -1;
  for (size_t index = 0; index < context->extension_count; index++) {
    PersistenceSqliteExtension *extension = &context->extensions[index];
    if (!strcmp(extension->name, name))
      return extension->load == load && extension->store == store &&
                     extension->context == extension_context
                 ? 0
                 : -1;
  }
  if (context->extension_count == PERSISTENCE_MAX_SQLITE_EXTENSIONS)
    return -1;
  context->extensions[context->extension_count++] =
      (PersistenceSqliteExtension){.name = name,
                                   .load = load,
                                   .store = store,
                                   .context = extension_context};
  return 0;
}
