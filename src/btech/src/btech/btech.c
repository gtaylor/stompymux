
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

#include "btechstats_global.h"
#include "coolmenu.h"
#include "mech.h"
#include "mux/commands/command_invocation.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/world/world_context.h"
#include "mycool.h"
#include "p.glue.scode.h"
#include "p.mech.utils.h"

void do_show(CommandInvocation *invocation) {
  CommandContext *command = invocation->context;
  GameDatabase *database = command->world->database;
  DbRef player = invocation->player;
  char *arg1 = invocation->first;
  int i;
  enum { CHAVA, CHVAL, CHSKI, CHADV, CHATT, MECHVALUES };
  char *cmds[] = {"allvalues",  "values",      "skills", "advantages",
                  "attributes", "xcodevalues", NULL};
  char *cmds_help[] = {"[char_]allvalues",
                       "[char_]values",
                       "[char_]skills",
                       "[char_]advantages",
                       "[char_]attributes",
                       "xcodevalues [scode]",
                       NULL};
  char buf[MBUF_SIZE] = {0};

  if (!WizP(database, player)) {
    notify(&command->evaluation, player,
           "You aren't cleared to know this stuff yet!");
    return;
  }

  if (!arg1 || !*arg1) {
    strcpy(buf, "Valid arguments:");
    for (i = 0; cmds_help[i]; i++)
      snprintf(buf + strlen(buf), MBUF_SIZE - strlen(buf), "%c %s",
               i > 0 ? ',' : ' ', cmds_help[i]);
    notify(&command->evaluation, player, buf);
    return;
  }
  i = listmatch(cmds, arg1);
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
  notify(&command->evaluation, player, "Invalid arguments to +show command!");
  return;
}
