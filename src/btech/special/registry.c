#include "btech_event.h" // IWYU pragma: keep
#include "command_catalogs.h"
#include "command_registry.h"
#include "configuration_internal.h"
#include "context_internal.h" // IWYU pragma: keep
#include "map.h"              // IWYU pragma: keep
#include "map_api.h"
#include "map_terrain.h"
#include "mech_parts.h"               // IWYU pragma: keep
#include "mech_scan_api.h"            // IWYU pragma: keep
#include "mech_status_api.h"          // IWYU pragma: keep
#include "mux/server/runtime_clock.h" // IWYU pragma: keep

/* Implements registration and lookup of BattleTech special objects. */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/configuration.h"
#include "btech/context.h"
#include "btech/ids.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "ds_turret_api.h"
#include "mech_lifecycle.h"
#include "mech_restrict_api.h"
#include "mech_template_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "missile_hit_registry.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "part_cost_api.h"
#include "registry_api.h"
#include "special_object.h"
#include "weapon_settings.h"

/*** #include all the prototype here! ****/
#include "autopilot.h"
#include "btech/persistence.h"
#include "btech/special_objects.h"
#include "coolmenu.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_partnames_api.h"
#include "mech_stat_api.h"
#include "mechrep_api.h"
#include "mux/objects/powers.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"
#include "registry_internal.h"
#include "section_types.h"
#include "turret.h"

/* Special object parameters.  */
const BtechSpecialObjectDefinition SPECIAL_OBJECTS[BTECH_SPECIAL_OBJECT_COUNT] =
    {{"MECH", MECHCOMMANDS, 0, mech_storage_size, newfreemech, HEAT_TICK,
      mech_update, POWER_NONE},
     {"DEBUG", DEBUGCOMMANDS, sizeof(BtechSpecialObject), nullptr, nullptr, 0,
      nullptr, POWER_NONE},
     {"MAP", MAPCOMMANDS, sizeof(BattleMap), nullptr, newfreemap, LOS_TICK,
      map_update, POWER_NONE},
     {"AUTOPILOT", AUTOPILOTCOMMANDS, sizeof(Autopilot), nullptr,
      auto_newautopilot, 0, nullptr, POWER_NONE},
     {"TURRET", TURRETCOMMANDS, sizeof(Turret), nullptr,
      turret_lifecycle_update, 0, nullptr, POWER_NONE}};

const BtechSpecialObjectDefinition *btech_special_object_definition(int type) {
  if (type < 0)
    abort();
  return checked_storage_at_const(SPECIAL_OBJECTS, BTECH_SPECIAL_OBJECT_COUNT,
                                  sizeof(*SPECIAL_OBJECTS), (size_t)type);
}

static HashTable *special_command_table(BtechContext *context, size_t type) {
  return checked_storage_at(context->special_commands,
                            context->special_command_count,
                            sizeof(*context->special_commands), type);
}

size_t btech_special_command_count(int type) {
  switch (type) {
  case GTYPE_MECH:
    return mech_command_count();
  case GTYPE_DEBUG:
    return debug_command_count();
  case GTYPE_MAP:
    return map_command_count();
  case GTYPE_AUTO:
    return autopilot_command_count();
  case GTYPE_TURRET:
    return turret_command_count();
  default:
    abort();
  }
}

const BtechCommandDefinition *btech_special_command_definition(int type,
                                                               size_t index) {
  const BtechSpecialObjectDefinition *definition =
      btech_special_object_definition(type);
  return checked_storage_at_const(definition->commands,
                                  btech_special_command_count(type),
                                  sizeof(*definition->commands), index);
}

int btech_special_object_type_count(void) { return BTECH_SPECIAL_OBJECT_COUNT; }

const char *btech_special_object_type_name(int type) {
  if (type < 0 || type >= BTECH_SPECIAL_OBJECT_COUNT)
    return "Unknown";
  return btech_special_object_definition(type)->type;
}

size_t btech_special_object_storage_size(int type) {
  if (type < 0 || type >= BTECH_SPECIAL_OBJECT_COUNT)
    return 0;
  return btech_special_object_data_size(btech_special_object_definition(type));
}

/* Prototypes */

/*************CALLABLE PROTOS*****************/

/* Main entry point */

void list_hashstat(DbRef player, const char *tab_name, HashTable *htab);

/*************PERSONAL PROTOS*****************/

static int compare_dbrefs(const RedBlackTreeCompareCall *call) {
  const intptr_t KEY1_VAL = *(const intptr_t *)call->lhs;
  const intptr_t KEY2_VAL = *(const intptr_t *)call->rhs;

  if (KEY1_VAL < KEY2_VAL)
    return -1;
  if (KEY1_VAL > KEY2_VAL)
    return 1;
  return 0;
}

void btech_registry_tree_initialize(BtechContext *context) {
  context->special_objects = red_black_tree_init(compare_dbrefs, nullptr);
  if (!context->special_objects) {
    /* TODO: We could handle this more gracefully... */
    exit(EXIT_FAILURE);
  }
}

/*********************************************/

static int mech_class_command_flag(int unit_class) {
  switch (unit_class) {
  case CLASS_MECH:
    return GFLAG_MECH;
  case CLASS_VEH_GROUND:
    return GFLAG_GROUNDVEH;
  case CLASS_AERO:
    return GFLAG_AERO;
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    return GFLAG_DS;
  case CLASS_VTOL:
    return GFLAG_VTOL;
  case CLASS_VEH_NAVAL:
    return GFLAG_NAVAL;
  case CLASS_BSUIT:
    return GFLAG_BSUIT;
  case CLASS_MW:
    return GFLAG_MW;
  default:
    return 0;
  }
}

bool btech_command_allowed_for_mech(Mech *mech, int cmdflag) {
  int i;

  if (!mech)
    return false;
  if (!cmdflag)
    return true;
  i = mech_class_command_flag(mech_class(mech));
  if (!i)
    return false;
  if (cmdflag > 0) {
    if (cmdflag & i)
      return true;
  } else if (!((0 - cmdflag) & i)) {
    return true;
  }
  return false;
}

bool btech_special_command_access(BtechContext *context, DbRef object,
                                  PowerId power) {
  return (is_god(context->database, object) ||
          is_wizard(context->database, object) ||
          (power != POWER_NONE && game_object_has_power(&(ObjectPowerRequest){
                                      .database = context->database,
                                      .object = object,
                                      .power = power}))) != 0;
}

bool handled_command_sub(BtechContext *context, DbRef player, DbRef location,
                         char *command) {
  BtechSpecialObject *xcode_obj = btech_context_find_object(context, location);

  const BtechSpecialObjectDefinition *type_of_object;
  int type;
  const BtechCommandDefinition *cmd;
  char *tmpc;
  int ishelp;

  if (xcode_obj == nullptr || is_zombie(context->database, location))
    return false;
  type = (int)xcode_obj->type;
  type_of_object = btech_special_object_definition(type);
  const size_t COMMAND_NAME_LENGTH = strcspn(command, " ");
  tmpc = strstr(command, " ");
  if (tmpc)
    *tmpc = 0;
  ishelp = !strcmp(command, "HELP");
  for (size_t index = 0; index < COMMAND_NAME_LENGTH; ++index) {
    char *character =
        checked_storage_at(command, strlen(command) + 1, sizeof(char), index);
    *character = ascii_to_lower(*character);
  }
  cmd = hash_table_find_const(command,
                              special_command_table(context, (size_t)type));
  if (tmpc)
    *tmpc = ' ';
  const char *argument_start =
      checked_string_suffix(command, COMMAND_NAME_LENGTH);
  const size_t ARGUMENT_OFFSET =
      COMMAND_NAME_LENGTH + strspn(argument_start, " ");
  char *arguments = checked_storage_at(command, strlen(command) + 1,
                                       sizeof(char), ARGUMENT_OFFSET);
  if (cmd && (type != GTYPE_MECH ||
              btech_command_allowed_for_mech(
                  btech_special_object_as_mech(xcode_obj), cmd->flag))) {
    if (*cmd->helpmsg != '@' ||
        btech_special_command_access(context, player,
                                     type_of_object->power_needed)) {
      const BtechCommandInvocation INVOCATION = {
          .context = context,
          .evaluation = btech_context_evaluation(context),
          .actor = player,
          .object_id = location,
          .object = xcode_obj,
          .arguments = arguments,
      };
      cmd->handler(&INVOCATION);
    } else {
      mecha_notify(btech_context_evaluation(context), player,
                   "Sorry, that command is restricted!");
    }
    return true;
  }
  if (ishelp) {
    btech_special_object_help(&(SpecialObjectHelpRequest){
        .context = context,
        .player = player,
        .type = type_of_object->type,
        .special_type = type,
        .location = location,
        .power_needed = type_of_object->power_needed,
        .argument = arguments});
    return true;
  }
  return false;
}

static bool okay_hcode(BtechContext *context, DbRef object) {
  return (object >= 0 &&
          btech_context_find_object(context, object) != nullptr &&
          !is_zombie(context->database, object)) != 0;
}

/* Main entry point */
bool btech_command_try_execute(BtechContext *context, DbRef player, DbRef loc,
                               char *command) {
  DbRef curr;
  DbRef temp;

  if (strlen(command) > (LBUF_SIZE - MBUF_SIZE))
    return false;
  if (okay_hcode(context, player) &&
      handled_command_sub(context, player, player, command))
    return true;
  if (okay_hcode(context, loc) &&
      handled_command_sub(context, player, loc, command))
    return true;
  SAFE_DOLIST(context->database, curr, temp,
              game_object_contents(context->database, player)) {
    if (okay_hcode(context, curr))
      if (handled_command_sub(context, player, curr, command))
        return true;
  }
  return false;
}

void *new_special_object(BtechContext *context, DbRef id, int type) {
  BtechSpecialObject *xcode_obj = nullptr;
  if (type < 0 || type >= BTECH_SPECIAL_OBJECT_COUNT)
    return nullptr;
  size_t data_size =
      btech_special_object_data_size(btech_special_object_definition(type));

  if (data_size) {
    xcode_obj =
        (BtechSpecialObject *)checked_storage_try_allocate_array(1, data_size);
    if (!xcode_obj) {
      printf("Unable to calloc\n");
      exit(1);
    }
    xcode_obj->type = (BtechSpecialObjectType)type;
    xcode_obj->size = data_size;
    xcode_obj->context = context;

    if (btech_special_object_definition(type)->lifecycle)
      btech_special_object_definition(type)->lifecycle(id, (void **)&xcode_obj,
                                                       SPECIAL_ALLOC);

    red_black_tree_insert_integer(context->special_objects, id, xcode_obj);
  }

  return xcode_obj;
}

void btech_special_object_dispose(const BtechSpecialObjectAction *action) {
  BtechContext *context = action->context;
  const DbRef KEY = action->object;
  BtechSpecialObject *xcode_obj;

  xcode_obj = red_black_tree_find_integer(context->special_objects, KEY);
  if (xcode_obj) {
    const BtechSpecialObjectDefinition *type_of_object =
        btech_special_object_definition((int)xcode_obj->type);
    if (type_of_object->lifecycle)
      type_of_object->lifecycle(KEY, (void **)&xcode_obj, SPECIAL_FREE);
    red_black_tree_delete_integer(context->special_objects, KEY);
    mux_event_remove_data(context->events, xcode_obj);
    free(xcode_obj);
  }
}

static void destroy_special_object(const RedBlackTreeReleaseCall *call) {
  const DbRef KEY = (DbRef) * (const intptr_t *)call->key;
  void *data = call->data;
  void *arg = call->context;
  BtechContext *context = arg;
  BtechSpecialObject *xcode_obj = data;
  const BtechSpecialObjectDefinition *type =
      btech_special_object_definition((int)xcode_obj->type);

  mux_event_remove_data(context->events, xcode_obj);
  if (type->lifecycle)
    type->lifecycle(KEY, (void **)&xcode_obj, SPECIAL_FREE);
  free(xcode_obj);
}

void btech_context_release_owned_state(BtechContext *context) {
  if (context == nullptr)
    return;

  if (context->special_objects != nullptr) {
    red_black_tree_release(context->special_objects, destroy_special_object,
                           context);
    context->special_objects = nullptr;
  }
  btech_configuration_destroy(context);
  for (size_t i = 0; i < context->special_command_count; i++)
    hash_table_destroy(special_command_table(context, i));
  free(context->special_commands);
  context->special_commands = nullptr;
  context->special_command_count = 0;
  btech_stats_destroy(context);
  btech_part_costs_destroy(context);
  destroy_partname_tables(context);
  missile_hit_registry_destroy(&context->missile_hits);
  btech_weapon_settings_destroy(&context->weapon_settings);
  mech_template_registry_destroy(context);
  mech_reference_cache_destroy(context);
  *context = (BtechContext){};
}

void dump_mechs(BtechContext *context, DbRef player) {
  mecha_notify(btech_context_evaluation(context), player,
               "Support discontinued. Bother a wiz if this bothers you.");
}

void dump_maps(BtechContext *context, DbRef player) {
  mecha_notify(btech_context_evaluation(context), player,
               "Support discontinued. Bother a wiz if this bothers you.");
}

/***************** INTERNAL ROUTINES *************/
int btech_context_which_special(BtechContext *context, DbRef key) {
  if (context == nullptr || context->special_objects == nullptr)
    return -1;
  BtechSpecialObject *object =
      red_black_tree_find_integer(context->special_objects, key);
  return object == nullptr ? -1 : (int)object->type;
}

int btech_special_object_type(BtechContext *context, BtechObjectId object) {
  return btech_context_which_special(context, (DbRef)object);
}

bool btech_context_is_mech(BtechContext *context, DbRef key) {
  return btech_context_which_special(context, key) == GTYPE_MECH;
}

bool btech_context_is_auto(BtechContext *context, DbRef key) {
  return btech_context_which_special(context, key) == GTYPE_AUTO;
}

bool btech_context_is_map(BtechContext *context, DbRef key) {
  return btech_context_which_special(context, key) == GTYPE_MAP;
}

void *btech_context_find_object(BtechContext *context, DbRef key) {
  if (context == nullptr || context->special_objects == nullptr)
    return nullptr;
  return red_black_tree_find_integer(context->special_objects, key);
}

void init_special_hash(BtechContext *context, int which) {
  char buf[MBUF_SIZE];

  hash_table_initialize(special_command_table(context, (size_t)which),
                        20 * HASH_FACTOR);
  for (size_t index = 0; index < btech_special_command_count(which); ++index) {
    const BtechCommandDefinition *command =
        btech_special_command_definition(which, index);
    if (!btech_command_definition_has_handler(command))
      continue;
    const size_t NAME_LENGTH = strcspn(command->name, " ");
    if (NAME_LENGTH >= sizeof(buf))
      continue;
    for (size_t name_index = 0; name_index < NAME_LENGTH; ++name_index) {
      *(char *)checked_storage_at(buf, sizeof(buf), sizeof(char), name_index) =
          ascii_to_lower(*checked_string_suffix(command->name, name_index));
    }
    *(char *)checked_storage_at(buf, sizeof(buf), sizeof(char), NAME_LENGTH) =
        '\0';
    hash_table_add_const(buf, command,
                         special_command_table(context, (size_t)which));
  }
}

static int special_type_parse(const char *name) {
  for (int type = 0; type < BTECH_SPECIAL_OBJECT_COUNT; type++)
    if (!strcasecmp(name, btech_special_object_type_name(type)))
      return type;
  return -1;
}

static bool special_actor_controls(BtechContext *context, DbRef actor,
                                   DbRef object) {
  return (is_wizard(context->database, actor) &&
          is_controls(context->database, actor, object)) != 0;
}

// NOLINTNEXTLINE(bugprone-easily-swappable-parameters)
bool btech_special_object_register(BtechContext *context, BtechObjectId actor,
                                   BtechObjectId object, const char *type_name,
                                   char *error, size_t error_size) {
  const DbRef ID = (DbRef)object;
  const int TYPE = special_type_parse(type_name);
  if (!is_good_obj(context->database, ID) || !is_thing(context->database, ID) ||
      is_going(context->database, ID)) {
    (void)snprintf(error, error_size, "target must be a live thing");
    return false;
  }
  if (!special_actor_controls(context, (DbRef)actor, ID)) {
    (void)snprintf(error, error_size, "permission denied");
    return false;
  }
  if (TYPE < 0) {
    (void)snprintf(error, error_size, "invalid BTech type %s", type_name);
    return false;
  }
  const int EXISTING = btech_context_which_special(context, ID);
  if (EXISTING == TYPE)
    return true;
  if (EXISTING >= 0) {
    (void)snprintf(error, error_size,
                   "object is already registered as %s; unregister it first",
                   btech_special_object_type_name(EXISTING));
    return false;
  }
  if (new_special_object(context, ID, TYPE) == nullptr) {
    (void)snprintf(error, error_size, "unable to allocate BTech object");
    return false;
  }
  return true;
}

bool btech_special_object_unregister(BtechContext *context, BtechObjectId actor,
                                     BtechObjectId object, char *error,
                                     size_t error_size) {
  const DbRef ID = (DbRef)object;
  if (!special_actor_controls(context, (DbRef)actor, ID)) {
    (void)snprintf(error, error_size, "permission denied");
    return false;
  }
  if (btech_context_which_special(context, ID) < 0) {
    btech_configuration_forget(context, object);
    return true;
  }
  btech_special_object_dispose(&(BtechSpecialObjectAction){
      .context = context, .actor = actor, .object = object});
  btech_configuration_forget(context, object);
  return true;
}

void btech_object_forget(BtechContext *context, BtechObjectId object) {
  if (context == nullptr)
    return;
  BtechSpecialObject *registered =
      context->special_objects == nullptr
          ? nullptr
          : red_black_tree_find_integer(context->special_objects,
                                        (DbRef)object);
  if (registered != nullptr)
    btech_special_object_dispose(&(BtechSpecialObjectAction){
        .context = context, .actor = NOTHING, .object = object});
  btech_configuration_forget(context, object);
}

#undef notify
void mecha_notify(EvaluationContext *evaluation, DbRef player,
                  const char *msg) {
  raw_notify(evaluation, player, msg);
}

void mecha_notify_except(const MechaNotificationExclusion *notification) {
  EvaluationContext *evaluation = notification->evaluation;
  const DbRef LOC = notification->location;
  const DbRef PLAYER = notification->actor;
  const DbRef EXCEPTION = notification->exception;
  const char *msg = notification->message;
  DbRef first;

  if (LOC != EXCEPTION)
    notify_checked(evaluation, LOC, PLAYER, msg,
                   MSG_ME_ALL | MSG_F_UP | MSG_S_INSIDE | MSG_NBR_EXITS_A);
  DOLIST(evaluation->world->database, first,
         game_object_contents(evaluation->world->database, LOC)) {
    if (first != EXCEPTION) {
      notify_checked(evaluation, first, PLAYER, msg,
                     (MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE));
    }
  }
}

void btech_special_objects_reset(BtechContext *context) {
  mux_event_run_by_type(context->events, EVENT_HIDE);
  mux_event_run_by_type(context->events, EVENT_BLINDREC);
}

BattleMap *btech_context_get_map(BtechContext *context, DbRef d) {
  BtechSpecialObject *xcode_obj;

  if (context == nullptr || context->special_objects == nullptr ||
      !is_good_obj(context->database, d))
    return nullptr;
  xcode_obj = red_black_tree_find_integer(context->special_objects, d);
  if (!xcode_obj)
    return nullptr;
  if (xcode_obj->type != GTYPE_MAP)
    return nullptr;
  return (BattleMap *)xcode_obj;
}

Mech *btech_context_get_mech(BtechContext *context, DbRef d) {
  BtechSpecialObject *xcode_obj;

  if (context == nullptr || context->special_objects == nullptr ||
      !is_good_obj(context->database, d))
    return nullptr;
  xcode_obj = red_black_tree_find_integer(context->special_objects, d);
  if (!xcode_obj)
    return nullptr;
  if (xcode_obj->type != GTYPE_MECH)
    return nullptr;
  return (Mech *)xcode_obj;
}
