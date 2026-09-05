
/* Registers and implements BattleTech MUX commands. */

/*
   Local btech alike stuff for MUX.
   Work's based on MUSE's btechstats.c
 */

#include <stdio.h>

#include "btech/repair/mechrep_api.h"
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

typedef void (*MechAdminHandler)(DbRef player, void *mech, char *arguments);

static void mech_admin_help(EvaluationContext *evaluation, DbRef player) {
  static const char *const HELP[] = {
      "@mech command switches:",
      "  /loadnew <typename>                  Load a unit template.",
      "  /restore                             Completely restore the unit.",
      "  /savenew <typename>                  Save the unit as a template.",
      "  /setarmor <loc> [front] [int] [rear] Set section armor values.",
      "  /addweap <name> <loc> <crits> [R|T|O] Add a weapon.",
      "  /resetcrits                          Reset critical slots.",
      "  /repair <loc> <type> [value]         Repair unit damage.",
      "  /reload <name> <loc> <slot> [mode]   Configure ammunition.",
      "  /restock <loc> <slot>                Refill an ammunition bin.",
      "  /firemode <weapon> <mode>            Change a weapon fire mode.",
      "  /addsp <item> <loc> <slot> [data]    Add special equipment.",
      "  /display <loc>                       Display section criticals.",
      "  /showtech                            Show unit technology.",
      "  /addtech <type>                      Add unit technology.",
      "  /deltech <all|type>                  Remove unit technology.",
      "  /addinftech <type>                   Add infantry technology.",
      "  /delinftech                          Remove infantry technology.",
      "  /settons <tons>                      Set unit tonnage.",
      "  /settype <type>                      Set the unit type.",
      "  /setmove <movement>                  Set the movement type.",
      "  /setmaxspeed <mp>                    Set maximum speed.",
      "  /setheatsinks <count>                Set heat sinks.",
      "  /setjumpspeed <mp>                   Set jump speed.",
      "  /setlrsrange <hexes>                 Set long-range scan range.",
      "  /settacrange <hexes>                 Set tactical range.",
      "  /setscanrange <hexes>                Set scan range.",
      "  /setradio <level>                    Set radio level.",
      "  /setradiorange <hexes>               Set radio range.",
      "  /setcargospace <space> <max tons>    Set cargo capacity.",
  };
  const size_t HELP_COUNT = sizeof(HELP) / sizeof(*HELP);
  for (size_t index = 0; index < HELP_COUNT; ++index) {
    const char *const *line = (const char *const *)checked_storage_at_const(
        (const void *)HELP, HELP_COUNT, sizeof(*HELP), index);
    raw_notify(evaluation, player, *line);
  }
}

static MechAdminHandler mech_admin_handler(int key) {
  switch (key) {
  case MECH_ADMIN_LOADNEW:
    return mechrep_rloadnew;
  case MECH_ADMIN_RESTORE:
    return mechrep_rrestore;
  case MECH_ADMIN_SAVENEW:
    return mechrep_rsavetemp2;
  case MECH_ADMIN_SETARMOR:
    return mechrep_rsetarmor;
  case MECH_ADMIN_ADDWEAP:
    return mechrep_raddweap;
  case MECH_ADMIN_RESETCRITS:
    return mechrep_rresetcrits;
  case MECH_ADMIN_REPAIR:
    return mechrep_rrepair;
  case MECH_ADMIN_RELOAD:
    return mechrep_rreload;
  case MECH_ADMIN_RESTOCK:
    return mechrep_rrestock;
  case MECH_ADMIN_FIREMODE:
    return mechrep_rfiremode;
  case MECH_ADMIN_ADDSP:
    return mechrep_raddspecial;
  case MECH_ADMIN_DISPLAY:
    return mechrep_rdisplaysection;
  case MECH_ADMIN_SHOWTECH:
    return mechrep_rshowtech;
  case MECH_ADMIN_ADDTECH:
    return mechrep_raddtech;
  case MECH_ADMIN_DELTECH:
    return mechrep_rdeltech;
  case MECH_ADMIN_ADDINFTECH:
    return mechrep_raddinftech;
  case MECH_ADMIN_DELINFTECH:
    return mechrep_rdelinftech;
  case MECH_ADMIN_SETTONS:
    return mechrep_rsettons;
  case MECH_ADMIN_SETTYPE:
    return mechrep_rsettype;
  case MECH_ADMIN_SETMOVE:
    return mechrep_rsetmove;
  case MECH_ADMIN_SETMAXSPEED:
    return mechrep_rsetspeed;
  case MECH_ADMIN_SETHEATSINKS:
    return mechrep_rsetheatsinks;
  case MECH_ADMIN_SETJUMPSPEED:
    return mechrep_rsetjumpspeed;
  case MECH_ADMIN_SETLRSRANGE:
    return mechrep_rsetlrsrange;
  case MECH_ADMIN_SETTACRANGE:
    return mechrep_rsettacrange;
  case MECH_ADMIN_SETSCANRANGE:
    return mechrep_rsetscanrange;
  case MECH_ADMIN_SETRADIO:
    return mechrep_rsetradio;
  case MECH_ADMIN_SETRADIORANGE:
    return mechrep_rsetradiorange;
  case MECH_ADMIN_SETCARGOSPACE:
    return mechrep_setcargospace;
  default:
    return nullptr;
  }
}

void do_mech_admin(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  if (invocation->key == 0) {
    mech_admin_help(evaluation, invocation->player);
    return;
  }
  const DbRef LOCATION = game_object_location(
      invocation->context->world->database, invocation->player);
  Mech *mech = btech_context_get_mech(evaluation->btech, LOCATION);
  if (mech == nullptr) {
    raw_notify(evaluation, invocation->player,
               "You must be inside a BattleTech unit to use @mech.");
    return;
  }
  MechAdminHandler handler = mech_admin_handler(invocation->key);
  if (handler == nullptr) {
    raw_notify(evaluation, invocation->player, "Unknown @mech switch.");
    return;
  }
  handler(invocation->player, mech, invocation->first);
}

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
                   "Specify MECH, DEBUG, MAP, AUTOPILOT, or TURRET.");
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
