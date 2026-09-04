
/* Registers and implements BattleTech MUX commands. */

/*
   Local btech alike stuff for MUX.
   Work's based on MUSE's btechstats.c
 */

#include <stdio.h>

#include "btech/special_objects.h"
#include "btech_api.h"
#include "btechstats_api.h"
#include "btechstats_global.h"
#include "command_handlers_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_invocation.h"
#include "mux/commands/command_keys.h"
#include "mux/commands/command_queue.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/world/object_set.h"
#include "registry_api.h"
#include "value_handlers_api.h"

void do_btech(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef object = match_controlled(&invocation->context->match,
                                  invocation->player, invocation->first);
  char error[256];

  if (object == NOTHING)
    return;
  if (invocation->key == BTECH_REGISTER) {
    if (!invocation->second || !*invocation->second) {
      mecha_notify(evaluation, invocation->player,
                   "Specify MECH, DEBUG, MECHREP, MAP, AUTOPILOT, or TURRET.");
      return;
    }
    if (!btech_special_object_register(evaluation->btech, invocation->player,
                                       object, invocation->second, error,
                                       sizeof(error))) {
      notify_printf(evaluation, invocation->player, "%s.", error);
      return;
    }
    notify_printf(evaluation, invocation->player,
                  "Registered #%ld as BTech type %s.", object,
                  btech_special_object_type_name(
                      btech_special_object_type(evaluation->btech, object)));
    return;
  }
  if (invocation->key == BTECH_UNREGISTER) {
    if (!btech_special_object_unregister(evaluation->btech, invocation->player,
                                         object, error, sizeof(error))) {
      notify_printf(evaluation, invocation->player, "%s.", error);
      return;
    }
    notify_printf(evaluation, invocation->player,
                  "Unregistered #%ld from BTech.", object);
    return;
  }
  int type = btech_special_object_type(evaluation->btech, object);
  if (type < 0) {
    notify_printf(evaluation, invocation->player,
                  "#%ld is not registered with BTech.", object);
    return;
  }
  notify_printf(evaluation, invocation->player, "#%ld BTech type: %s", object,
                btech_special_object_type_name(type));
}

void do_show(CommandInvocation *invocation) {
  CommandContext *command = invocation->context;
  GameDatabase *database = command->world->database;
  DbRef player = invocation->player;
  char *arg1 = invocation->first;
  int i;
  enum { CHAVA, CHVAL, CHSKI, CHADV, CHATT, MECHVALUES };
  const char *const CMDS[] = {"allvalues",  "values",     "skills",
                              "advantages", "attributes", "btechvalues",
                              nullptr};
  const char *const CMDS_HELP[] = {"[char_]allvalues",
                                   "[char_]values",
                                   "[char_]skills",
                                   "[char_]advantages",
                                   "[char_]attributes",
                                   "btechvalues [scode]",
                                   nullptr};
  char buf[MBUF_SIZE] = {0};

  if (!is_wizard(database, player)) {
    mecha_notify(&command->evaluation, player,
                 "You aren't cleared to know this stuff yet!");
    return;
  }

  if (!arg1 || !*arg1) {
    (void)string_copy_bounded(buf, sizeof(buf), "Valid arguments:");
    const size_t HELP_COUNT = (sizeof(CMDS_HELP) / sizeof(*CMDS_HELP)) - 1;
    for (size_t index = 0; index < HELP_COUNT; index++) {
      const char *const *help = (const char *const *)checked_storage_at_const(
          (const void *)CMDS_HELP, HELP_COUNT, sizeof(*CMDS_HELP), index);
      char entry[80];
      (void)snprintf(entry, sizeof(entry), "%c %s", index > 0 ? ',' : ' ',
                     *help);
      (void)string_append_bounded(buf, sizeof(buf), entry);
    }
    mecha_notify(&command->evaluation, player, buf);
    return;
  }
  i = listmatch(CMDS, 6, arg1);
  /* Do da cmd */
  switch (i) {
  case MECHVALUES:
    list_special_value_names(&command->evaluation, player);
    return;
  case CHAVA:
    list_charvaluestuff(&command->evaluation, player, -1);
    return;
  case CHVAL:
    list_charvaluestuff(&command->evaluation, player, CHAR_VALUE);
    return;
  case CHSKI:
    list_charvaluestuff(&command->evaluation, player, CHAR_SKILL);
    return;
  case CHADV:
    list_charvaluestuff(&command->evaluation, player, CHAR_ADVANTAGE);
    return;
  case CHATT:
    list_charvaluestuff(&command->evaluation, player, CHAR_ATTRIBUTE);
    return;
  }
  mecha_notify(&command->evaluation, player,
               "Invalid arguments to +show command!");
}
