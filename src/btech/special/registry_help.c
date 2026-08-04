#include "btech_event.h" // IWYU pragma: keep
#include "map.h"         // IWYU pragma: keep
#include "map_api.h"
#include "map_terrain.h"
#include "mech_parts.h"               // IWYU pragma: keep
#include "mech_scan_api.h"            // IWYU pragma: keep
#include "mech_status_api.h"          // IWYU pragma: keep
#include "mux/server/runtime_clock.h" // IWYU pragma: keep

/*
 * $Id: registry.c,v 1.4 2005/08/08 09:43:09 murrayma Exp $
 *
 * Original author: unknown
 *
 * Copyright (c) 1996-2002 Markus Stenberg
 * Copyright (c) 1998-2002 Thomas Wouters
 * Copyright (c) 2000-2002 Cord Awtry
 *
 * Last modified: Thu Jul  9 02:40:16 1998 fingon
 *
 * This includes the basic code to allow objects to have hardcoded
 * commands / properties.
 *
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "ds_turret_api.h"
#include "legacy_macros.h"
#include "map_dynamic_api.h"
#include "mech_lifecycle.h"
#include "mech_restrict_api.h"
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
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/formatting.h"
#include "mux/support/hash_table.h"
#include "registry_api.h"
#include "special_object.h"
#include "weapon_settings.h"

#define FAST_WHICHSPECIAL

#define _GLUE_C

/*** #include all the prototype here! ****/
#include "autopilot.h"
#include "btech/persistence.h"
#include "coolmenu.h"
#include "mech.h"
#include "mech_events.h"
#include "mech_partnames_api.h"
#include "mech_stat_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "mux/objects/powers.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"
#include "mycool.h"
#include "registry_internal.h"
#include "turret.h"
void center_string(char *destination, size_t destination_size,
                   const char *source, int width) {
  if (destination == nullptr || destination_size == 0)
    return;

  size_t source_length = strlen(source);
  size_t padding = 0;
  if (width > 0 && (size_t)width > source_length)
    padding = ((size_t)width - source_length) / 2;
  padding = MIN(padding, destination_size - 1);
  memset(destination, ' ', padding);
  snprintf(destination + padding, destination_size - padding, "%s", source);
}

static void help_color_initialize(const char *from, char *to) {
  int i;
  char buf[LBUF_SIZE];
  char *tp = to;

  for (i = 0; from[i] && from[i] != ' '; i++)
    ;
  if (from[i]) {

    /*      from[i]=0; */
    strncpy(buf, from, i);
    buf[i] = 0;
    safe_str("[fg=blue bold]", to, &tp);
    safe_str(buf, to, &tp);
    safe_str("[reset] ", to, &tp);
    safe_str((char *)&from[i + 1], to, &tp);

    /*      from[i]=' '; */
  } else {
    safe_str("[fg=cyan]", to, &tp);
    safe_str((char *)from, to, &tp);
    safe_str("[reset]", to, &tp);
  }
  *tp = '\0';
}

#define ONE_LINE_TEXTS

#ifdef ONE_LINE_TEXTS
#define MLen CM_ONE
#else
#define MLen CM_TWO
#endif

static char *do_ugly_things(CoolMenu **d, char *msg, int len, int initial) {
  CoolMenu *c = *d;
  size_t msg_len;
  char *e;
  char buf[LBUF_SIZE];

  /* XXX: Not entirely sure what this is for.  */
#ifndef ONE_LINE_TEXTS
  if (!msg) {
    sim(" ", MLen);
    *d = c;
    return NULL;
  }
#endif

  /*
   * Split off at last space on a line, taking into account initial
   * indentation, etc.  Help messages are strings of words, separated by
   * at most one space, with no word longer than len.
   *
   * All of these assumptions are necessary for this code to be safe.
   * Basically, the code needs to find the breaking space.
   *
   * FIXME: All of this code really needs more cleanup and fixing.
   */
  msg_len = strlen(msg);

  if (msg_len <= (size_t)len) {
    /* Line fits, don't split anything.  */
    e = msg + msg_len;
  } else {
    /* Split at last space on line.  */
    for (e = msg + len - 1; *e != ' '; e--)
      ;
  }

  if (initial > 0) {
    /* Colorize header line.  */
    help_color_initialize(msg, buf);
  } else if (initial < 0) {
    /* Write indented line.  */
    memset(buf, ' ', -initial);
    memcpy(buf - initial, msg, e - msg);
    buf[(e - msg) - initial] = '\0';
  } else {
    /* Write unindented line.  */
    memcpy(buf, msg, e - msg);
    buf[e - msg] = '\0';
  }

  sim(buf, MLen);

  /* Move pointer to start of next line.  */
  if (*e == ' ')
    e++;

  *d = c;
  return *e ? e : NULL;
}

#define Len(s) ((!s || !*s) ? 0 : strlen(s))

#define TAB 3

static void cut_apart_helpmsgs(CoolMenu **d, char *msg1, char *msg2, int len,
                               int initial) {
  int l1 = Len(msg1);
  int l2 = Len(msg2);
  int nl1, nl2;

#ifndef ONE_LINE_TEXTS

  msg1 = do_ugly_things(d, msg1, len, initial);
  msg2 =
      do_ugly_things(d, msg2, initial ? len : len - TAB, initial ? 0 : 0 - TAB);
  if (!msg1 && !msg2)
    return;
  nl1 = Len(msg1);
  nl2 = Len(msg2);
  if (nl1 != l1 || nl2 != l2) /* To prevent infinite loops */
    cut_apart_helpmsgs(d, msg1, msg2, len, 0);
#else
  int first = 1;

  while (msg1 && *msg1) {
    msg1 = do_ugly_things(d, msg1, len * 2 - 1, first);
    nl1 = Len(msg1);
    if (nl1 == l1)
      break;
    l1 = nl1;
    first = 0;
  }
  while (msg2 && *msg2) {
    msg2 = do_ugly_things(d, msg2, len * 2 - TAB, 0 - TAB);
    nl2 = Len(msg2);
    if (nl2 == l2)
      break;
    l2 = nl2;
  }

#endif
}

void btech_special_object_help(BtechContext *context, DbRef player, char *type,
                               int id, int loc, PowerId powerneeded, int objid,
                               char *arg) {
  int i, j;
  Mech *mech = NULL;
  int pos[100][2];
  int count = 0, csho = 0;
  CoolMenu *c = NULL;
  char buf[LBUF_SIZE];
  char *d;
  int dc;

  if (id == GTYPE_MECH)
    mech = btech_context_get_mech(context, loc);
  bzero(pos, sizeof(pos));
  for (i = 0; SpecialObjects[id].commands[i].name; i++) {
    if (!btech_command_definition_has_handler(
            &SpecialObjects[id].commands[i]) &&
        (SpecialObjects[id].commands[i].helpmsg[0] != '@' ||
         btech_special_command_access(context, player, powerneeded)))
      if (id != GTYPE_MECH || btech_command_allowed_for_mech(
                                  mech, SpecialObjects[id].commands[i].flag)) {
        if (count)
          pos[count - 1][1] = i - pos[count - 1][0];
        pos[count][0] = i;
        count++;
      }
  }
  if (count)
    pos[count - 1][1] = i - pos[count - 1][0];
  else {
    pos[0][0] = 0;
    pos[0][1] = i;
    count = 1;
  }
  sim(NULL, CM_ONE | CM_LINE);
  if (!arg || !*arg) {
#define HELPMSG(a)                                                             \
  &SpecialObjects[id]                                                          \
       .commands[a]                                                            \
       .helpmsg[SpecialObjects[id].commands[a].helpmsg[0] == '@']
    for (i = 0; i < count; i++) {
      if (count > 1) {
        center_string(buf, sizeof(buf), HELPMSG(pos[i][0]), 70);
        d = buf;
        sim(tprintf("%s%s%s", "[fg=green]", d, "[reset]"), CM_ONE);
      } else
        sim(tprintf("%s command listing: ", type), CM_ONE | CM_CENTER);
      for (j = pos[i][0] + (count == 1 ? 0 : 1); j < pos[i][0] + pos[i][1]; j++)
        if (SpecialObjects[id].commands[j].helpmsg[0] != '@' ||
            btech_special_command_access(context, player, powerneeded))
          if (id != GTYPE_MECH ||
              btech_command_allowed_for_mech(
                  mech, SpecialObjects[id].commands[j].flag)) {
            strcpy(buf, SpecialObjects[id].commands[j].name);
            d = buf;
            while (*d && *d != ' ')
              d++;
            if (*d == ' ')
              *d = 0;
            sim(buf, CM_FOUR);
            csho++;
          }
    }
    if (!csho)
      vsi(tprintf("There are no commands you are authorized to use here."));
    else {
      sim(NULL, CM_ONE | CM_LINE);
      if (count > 1)
        vsi("Additional info available with 'HELP SUBTOPIC'");
      else
        vsi("Additional info available with 'HELP ALL'");
    }
  } else {
    /* Try to find matching subtopic, or ALL */
    if (!strcasecmp(arg, "all")) {
      if (count > 1) {
        vsi("ALL not available for objects with subcategories.");
        dc = -2;
      } else
        dc = -1;
    } else {
      if (count == 1) {
        vsi("This object doesn't have any other detailed help than 'HELP ALL'");
        dc = -2;
      } else {
        for (i = 0; i < count; i++)
          if (!strcasecmp(arg, HELPMSG(pos[i][0])))
            break;
        if (i == count) {
          vsi("Subcategory not found.");
          dc = -2;
        } else
          dc = i;
      }
    }
    if (dc > -2) {
      for (i = 0; i < count; i++)
        if (dc == -1 || i == dc) {
          if (count > 1) {
            center_string(buf, sizeof(buf), HELPMSG(pos[i][0]), 70);
            vsi(tprintf("%s%s%s", "[fg=green]", buf, "[reset]"));
          }
          for (j = pos[i][0] + (count == 1 ? 0 : 1); j < pos[i][0] + pos[i][1];
               j++)
            if (SpecialObjects[id].commands[j].helpmsg[0] != '@' ||
                btech_special_command_access(context, player, powerneeded))
              if (id != GTYPE_MECH ||
                  btech_command_allowed_for_mech(
                      mech, SpecialObjects[id].commands[j].flag))
                cut_apart_helpmsgs(&c, SpecialObjects[id].commands[j].name,
                                   HELPMSG(j), 37, 1);
        }
    }
  }
  sim(NULL, CM_ONE | CM_LINE);
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}
