
/*
 * $Id: btech.c,v 1.1.1.1 2005/01/11 21:18:02 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Thu Sep 19 21:59:09 1996 fingon
 * Last modified: Tue Aug 12 19:39:55 1997 fingon
 *
 */

/*
   Local btech alike stuff for MUX.
   Work's based on MUSE's btechstats.c
 */

#include <stdio.h>
#include <string.h>

#include "btech_api.h"
#include "btechstats_api.h"
#include "btechstats_global.h"
#include "command_handlers_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_invocation.h"
#include "mux/commands/command_queue.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "value_handlers_api.h"

void do_show(CommandInvocation *invocation) {
  CommandContext *command = invocation->context;
  GameDatabase *database = command->world->database;
  DbRef player = invocation->player;
  char *arg1 = invocation->first;
  int i;
  enum { CHAVA, CHVAL, CHSKI, CHADV, CHATT, MECHVALUES };
  const char *const cmds[] = {"allvalues",  "values",     "skills",
                              "advantages", "attributes", "xcodevalues",
                              nullptr};
  const char *const cmds_help[] = {"[char_]allvalues",
                                   "[char_]values",
                                   "[char_]skills",
                                   "[char_]advantages",
                                   "[char_]attributes",
                                   "xcodevalues [scode]",
                                   nullptr};
  char buf[MBUF_SIZE] = {0};

  if (!is_wizard(database, player)) {
    mecha_notify(&command->evaluation, player,
                 "You aren't cleared to know this stuff yet!");
    return;
  }

  if (!arg1 || !*arg1) {
    strcpy(buf, "Valid arguments:");
    const size_t help_count = sizeof(cmds_help) / sizeof(*cmds_help) - 1;
    for (size_t index = 0; index < help_count; index++) {
      const char *const *help = checked_storage_at_const(
          cmds_help, help_count, sizeof(*cmds_help), index);
      char entry[80];
      snprintf(entry, sizeof(entry), "%c %s", index > 0 ? ',' : ' ', *help);
      strncat(buf, entry, sizeof(buf) - strlen(buf) - 1);
    }
    mecha_notify(&command->evaluation, player, buf);
    return;
  }
  i = listmatch(cmds, 6, arg1);
  /* Do da cmd */
  switch (i) {
  case MECHVALUES:
    list_xcodevalues(&command->evaluation, player);
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
  return;
}
