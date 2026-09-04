/* object_lifecycle.c - validated world-object lifecycle operations. */

#include "mux/world/object_lifecycle.h"

#include "btech/special_objects.h"
#include "mux/commands/command_context.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"

static bool
object_destroy_is_protected(const ObjectDestroyScheduleRequest *request) {
  const ServerConfiguration *configuration =
      request->evaluation->world->configuration;
  DbRef object = request->object;

  if (object == 0)
    return true;
  if (is_god(request->evaluation->world->database, object))
    return true;
  if (object == configuration->default_home)
    return true;
  if (object == configuration->start_home)
    return true;
  return object == configuration->start_room;
}

ObjectDestroyStatus
object_destroy_schedule(const ObjectDestroyScheduleRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  GameDatabase *database = evaluation->world->database;
  DbRef object = request->object;

  if (is_safe(database, object) && !request->override_safe)
    return OBJECT_DESTROY_SAFE;
  if (object_destroy_is_protected(request))
    return OBJECT_DESTROY_PROTECTED;
  if (is_going(database, object))
    return OBJECT_DESTROY_ALREADY_GOING;
  if (is_player(database, object)) {
    if (!is_wizard(database, request->actor))
      return OBJECT_DESTROY_PLAYER_PERMISSION;
    if (is_wizard(database, object))
      return OBJECT_DESTROY_WIZARD_PLAYER;
  }

  btech_object_forget(evaluation->btech, object);
  s_going(database, object);
  if (is_player(database, object))
    game_database_object(database, object)->pending_destroyer = request->actor;
  return OBJECT_DESTROY_SCHEDULED;
}
