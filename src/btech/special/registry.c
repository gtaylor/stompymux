#include "btech_event.h" // IWYU pragma: keep
#include "command_catalogs.h"
#include "command_registry.h"
#include "context_internal.h" // IWYU pragma: keep
#include "map.h"              // IWYU pragma: keep
#include "map_api.h"
#include "map_terrain.h"
#include "mech_parts.h"               // IWYU pragma: keep
#include "mech_scan_api.h"            // IWYU pragma: keep
#include "mech_status_api.h"          // IWYU pragma: keep
#include "mux/server/runtime_clock.h" // IWYU pragma: keep

/* Implements registration and lookup of BattleTech special objects. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btconfig.h"
#include "btech/context.h"
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
#include "mux/objects/attrs.h"
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
#include "mechrep.h"
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
     {"MECHREP", MECHREPCOMMANDS, sizeof(RepairFacility), nullptr,
      newfreemechrep, 0, nullptr, POWER_NONE},
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
  case GTYPE_MECHREP:
    return repair_command_count();
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
  const void *key1 = call->lhs;
  const void *key2 = call->rhs;
  const DbRef KEY1_VAL = (DbRef)key1;
  const DbRef KEY2_VAL = (DbRef)key2;

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

  if (!cmdflag)
    return true;
  if (!mech)
    return false;
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
  BtechSpecialObject *xcode_obj = nullptr;

  const BtechSpecialObjectDefinition *type_of_object;
  int type;
  const BtechCommandDefinition *cmd;
  char *tmpc;
  int ishelp;

  type = btech_context_which_special(context, location);
  bool special_data_missing = false;
  if (type >= 0 && btech_special_object_data_size(
                       btech_special_object_definition(type)) > 0) {
    xcode_obj = red_black_tree_find(context->special_objects, (void *)location);
    special_data_missing = xcode_obj == nullptr;
  }
  if (type < 0 || special_data_missing) {
    if (type >= 0 || !is_xcode(context->database, location) ||
        is_zombie(context->database, location))
      return false;
    type = btech_context_which_special_attribute(context, location);
    if (type >= 0) {
      if (btech_special_object_data_size(
              btech_special_object_definition(type)) > 0)
        return false;
    } else {
      return false;
    }
  }
  if (type > BTECH_SPECIAL_OBJECT_COUNT)
    return false;
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
              (type == GTYPE_MECH && btech_command_allowed_for_mech(
                                         ((Mech *)xcode_obj), cmd->flag)))) {
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
  return (object >= 0 && is_xcode(context->database, object) &&
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

    red_black_tree_insert(context->special_objects, (void *)id, xcode_obj);
  }

  return xcode_obj;
}

static void create_special_object(const BtechSpecialObjectAction *action) {
  BtechContext *context = action->context;
  const DbRef PLAYER = action->actor;
  const DbRef KEY = action->object;
  void *new;
  const BtechSpecialObjectDefinition *type_of_object;
  int type;
  char *str;

  str = btech_attribute_read(context->database, KEY, A_XTYPE,
                             (char[LBUF_SIZE]){0});
  if (!(str && *str)) {
    mecha_notify(
        btech_context_evaluation(context), PLAYER,
        "You must first set Xtype using @attribute/set <object>/Xtype=<type>");
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "Valid XTYPEs include: MECH, MECHREP, MAP, DEBUG, "
                 "AUTOPILOT, TURRET.");
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "Resetting XCODE flag.");
    c_xcode(context->database, KEY); /* Reset the flag */
    return;
  }

  /* Find the special objects */
  type = btech_context_which_special_attribute(context, KEY);
  if (type > -1) {
    /* We found the proper special object */
    type_of_object = btech_special_object_definition(type);
    if (btech_special_object_data_size(type_of_object)) {
      new = new_special_object(context, KEY, type);
      if (!new)
        mecha_notify(btech_context_evaluation(context), PLAYER,
                     "Memory allocation failure!");
    }
  } else {
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "That is not a valid XTYPE!");
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "Valid XTYPEs include: MECH, MECHREP, MAP, DEBUG, "
                 "AUTOPILOT, TURRET.");
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "Resetting XCODE flag.");
    c_xcode(context->database, KEY);
  }
}

void btech_special_object_dispose(const BtechSpecialObjectAction *action) {
  BtechContext *context = action->context;
  const DbRef PLAYER = action->actor;
  const DbRef KEY = action->object;
  BtechSpecialObject *xcode_obj;

  int i;
  const BtechSpecialObjectDefinition *type_of_object;

  xcode_obj = red_black_tree_find(context->special_objects, (void *)KEY);

  i = btech_context_which_special_attribute(context, KEY);
  if (i < 0) {
    mecha_notify(
        btech_context_evaluation(context), PLAYER,
        "CRITICAL: Unable to free data, inconsistency somewhere. Please");
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "contact a wizard about this _NOW_!");
    return;
  }
  type_of_object = btech_special_object_definition(i);

  if (btech_special_object_data_size(type_of_object) > 0 &&
      btech_context_which_special(context, KEY) != i) {
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "Semi-critical error has occured. For some reason the "
                 "object's data differs\nfrom the data on the object. Please "
                 "contact a wizard about this.");
  }
  if (xcode_obj) {
    if (type_of_object->lifecycle)
      type_of_object->lifecycle(KEY, (void **)&xcode_obj, SPECIAL_FREE);
    red_black_tree_delete(context->special_objects, (void *)KEY);
    mux_event_remove_data(context->events, xcode_obj);
    free(xcode_obj);
  } else if (btech_special_object_data_size(type_of_object) > 0) {
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "This object is not in the special object DBASE.");
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "Please contact a wizard about this bug. ");
  }
}

static void destroy_special_object(const RedBlackTreeReleaseCall *call) {
  void *key = call->key;
  void *data = call->data;
  void *arg = call->context;
  BtechContext *context = arg;
  BtechSpecialObject *xcode_obj = data;
  const BtechSpecialObjectDefinition *type =
      btech_special_object_definition((int)xcode_obj->type);

  mux_event_remove_data(context->events, xcode_obj);
  if (type->lifecycle)
    type->lifecycle((DbRef)key, (void **)&xcode_obj, SPECIAL_FREE);
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
  return btech_context_which_special_attribute(context, key);
}

int btech_context_which_special_attribute(BtechContext *context, DbRef key) {
  int i;
  int return_value = -1;
  char *str;

  if (!is_xcode(context->database, key))
    return -1;
  str = btech_attribute_read(context->database, key, A_XTYPE,
                             (char[LBUF_SIZE]){0});
  if (str && *str) {
    for (i = 0; i < BTECH_SPECIAL_OBJECT_COUNT; i++) {
      if (!strcmp(btech_special_object_definition(i)->type, str)) {
        return_value = i;
        break;
      }
    }
  }
  return (return_value);
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
  return red_black_tree_find(context->special_objects, (void *)key);
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

void btech_special_object_flag_changed(BtechContext *context, DbRef player,
                                       DbRef obj, bool from, bool to) {
  if (from == to)
    return;
  if (!to) {
    s_xcode(context->database, obj);
    btech_special_object_dispose(&(BtechSpecialObjectAction){
        .context = context, .actor = player, .object = obj});
    c_xcode(context->database, obj);
  } else {
    create_special_object(&(BtechSpecialObjectAction){
        .context = context, .actor = player, .object = obj});
  }
}

bool btech_special_object_type_can_set(BtechContext *context, DbRef object,
                                       const char *type, char *error,
                                       size_t error_size) {
  BtechSpecialObject *registered;
  int requested = -1;

  if (!*type) {
    if (is_xcode(context->database, object)) {
      (void)snprintf(error, error_size,
                     "cannot clear XTYPE while XCODE is set");
      return false;
    }
    return true;
  }
  for (int index = 0; index < BTECH_SPECIAL_OBJECT_COUNT; index++) {
    if (!strcmp(btech_special_object_definition(index)->type, type)) {
      requested = index;
      break;
    }
  }
  if (requested < 0) {
    (void)snprintf(error, error_size, "invalid XTYPE %s", type);
    return false;
  }
  if (!is_xcode(context->database, object))
    return true;

  registered = btech_context_find_object(context, object);
  if (registered && (int)registered->type != requested) {
    (void)snprintf(error, error_size,
                   "cannot change XTYPE while the XCODE object is registered");
    return false;
  }
  return true;
}

void btech_special_object_type_register(
    const BtechSpecialObjectAction *action) {
  BtechContext *context = action->context;
  const DbRef PLAYER [[maybe_unused]] = action->actor;
  const DbRef OBJECT = action->object;
  int type;

  if (!is_xcode(context->database, OBJECT) ||
      btech_context_find_object(context, OBJECT))
    return;
  type = btech_context_which_special_attribute(context, OBJECT);
  if (type >= 0 &&
      btech_special_object_data_size(btech_special_object_definition(type)) > 0)
    new_special_object(context, OBJECT, type);
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

  xcode_obj = red_black_tree_find(context->special_objects, (void *)d);
  if (!xcode_obj)
    return nullptr;
  if (xcode_obj->type != GTYPE_MAP)
    return nullptr;
  return (BattleMap *)xcode_obj;
}

Mech *btech_context_get_mech(BtechContext *context, DbRef d) {
  BtechSpecialObject *xcode_obj;

  if (!(is_good_obj(context->database, d)))
    return nullptr;
  if (!(is_xcode(context->database, d)))
    return nullptr;
  xcode_obj = red_black_tree_find(context->special_objects, (void *)d);
  if (!xcode_obj)
    return nullptr;
  if (xcode_obj->type != GTYPE_MECH)
    return nullptr;
  return (Mech *)xcode_obj;
}
