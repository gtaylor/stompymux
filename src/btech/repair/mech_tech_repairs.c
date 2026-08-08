
/*
 * $Id: mech.tech.repairs.c,v 1.1.1.1 2005/01/11 21:18:26 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Sat Aug 23 19:12:32 1997 fingon
 * Last modified: Sat Jun  6 20:45:48 1998 fingon
 *
 */

#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_event.h"
#include "coolmenu.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_events_api.h"
#include "mux/network/mux_event.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mycool.h"
#include "registry_api.h"
#include "repair_job.h"

static void describe_repairs(MuxEvent *e, void *menu_context) {
  CoolMenu **menu = menu_context;
  int type = e->type;
  Mech *mech = (Mech *)e->data;
  long earg = ((long)e->data2) % PLAYERPOS;
  DbRef player = ((long)e->data2) / PLAYERPOS;
  int loc, pos, extra;
  char buf[MBUF_SIZE] = {0};
  char buf2[LBUF_SIZE] = {0};
  int fail = (e->function == mech_event_failure_marker);
  BtechContext *context = mech_context(mech);

  RepairEventPayload payload = repair_event_payload_unpack(earg);
  loc = payload.location;
  pos = payload.position;
  extra = payload.extra;
  snprintf(buf, sizeof(buf), "%s%s",
           armor_section_abbreviation(mech_class(mech),
                                      mech_movement_type(mech), loc % 8)
               .text,
           loc >= 8 ? "(R)" : "");
  snprintf(buf2, sizeof(buf2), "%-5ld ", player);
  snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), "%-4d ",
           game_lag_time(context, (e->tick - e->scheduler->tick) / 60));
  switch (type) {
  case EVENT_REPAIR_REPL:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s:%-2d Replacement of %s", buf, pos + 1,
             pos_part_name(mech, loc, pos).text);
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_REPLG:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s:%-2d Replacement of %s", buf, pos + 1,
             pos_part_name(mech, loc, pos).text);
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_REAT:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s Reattachment", buf);
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_RELO:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s:%-2d %sload of %s", buf, pos + 1, extra ? "Un" : "Re",
             pos_part_name(mech, loc, pos).text);
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_FIX:
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
               "%5s:%-2d Failed armor repair", buf, 0);
    else
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
               "%5s:%-2d Repair of armor - possibly next point", buf, pos);
    break;
  case EVENT_REPAIR_FIXI:
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
               "%5s:%-2d Failed internal repair", buf, 0);
    else
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
               "%5s:%-2d Repair of internals - possibly next point", buf, pos);
    break;
  case EVENT_REPAIR_SCRL:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), "%5s Removal",
             buf);
    break;
  case EVENT_REPAIR_SCRP:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s:%-2d Scrapping of %s", buf, pos + 1,
             pos_part_name(mech, loc, pos).text);
    break;
  case EVENT_REPAIR_SCRG:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s:%-2d Scrapping of %s", buf, pos + 1,
             pos_part_name(mech, loc, pos).text);
    break;
  case EVENT_REPAIR_REPAG:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s:%-2d Repair of %s", buf, pos + 1,
             pos_part_name(mech, loc, pos).text);
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_REPAP:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s:%-2d Repair of %s", buf, pos + 1,
             pos_part_name(mech, loc, pos).text);
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_REPENHCRIT:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s:%-2d Repair of %s", buf, pos + 1,
             pos_part_name(mech, loc, pos).text);
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_MOB:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s:%-2d Mounting of %s", buf, pos + 1,
             pos_part_name(mech, loc, pos).text);
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_UMOB:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s:%-2d Removing of %s", buf, pos + 1,
             pos_part_name(mech, loc, pos).text);
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), " (Failure)");
    break;
  case EVENT_REPAIR_REPSUIT:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2),
             "%5s Replacing suit", buf);
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), " (Failure)");
    break;
  // Added Reseal description
  case EVENT_REPAIR_RESE:
    snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), "%5s Reseal",
             buf);
    if (fail)
      snprintf(buf2 + strlen(buf2), sizeof(buf2) - strlen(buf2), " (Failure)");
    break;
  }

  cool_menu_entry_very_simple(menu, buf2);
}

void tech_repairs(DbRef player, Mech *mech, char *buffer) {
  int i;
  CoolMenu *c = nullptr;
  BtechContext *context = mech_context(mech);
  bool is_wizard_player = is_wizard(btech_context_database(context), player);

  if (mech_event_count(mech, EVENT_STARTUP) && !is_wizard_player) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The mech's starting up! Please stop the sequence first.");
    return;
  }
  if (mech_is_started(mech) && !is_wizard_player) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The mech's started up ; please shut it down first.");
    return;
  }
  if (btech_context_limits_repairs_to_stalls(context) &&
      !mech_is_dropship(mech) && mech_repair_stall_dbref(mech) <= 0 &&
      !is_wizard_player) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The 'mech isn't in a repair stall!");
    return;
  }

  if (!figure_latest_tech_event(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This 'mech has no repairs pending!");
    return;
  }
  cool_menu_add_line(&c);
  cool_menu_add_centered(&c, tprintf("Repairs/Scrapping in progress (%s)",
                                     mech_display_id(mech).text));
  cool_menu_add_text(
      &c, tprintf("%-5s %-4s %s", "Plr", "Time", "Location + Description"));
  cool_menu_add_line(&c);
  for (i = FIRST_TECH_EVENT; i <= LAST_TECH_EVENT; i++)
    mech_event_visit(mech, i, describe_repairs, &c);
  cool_menu_add_line(&c);
  cool_menu_add_text(
      &c, "Note: Time = Time remaining in minutes. Plr = Tech's dbref");
  cool_menu_add_line(&c);
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}
