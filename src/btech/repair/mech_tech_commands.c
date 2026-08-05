/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <string.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "legacy_macros.h"
#include "mech_consistency_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_status_api.h"
#include "mech_tech.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_do_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"

typedef struct TechCheckContext {
  int matches;
  int location;
  int part;
} TechCheckContext;

static void tech_check_locpart(MuxEvent *e, void *data) {
  TechCheckContext *context = data;
  int loc, pos;
  long l = (long)e->data2;

  UNPACK_LOCPOS(l, loc, pos);
  if (loc == context->location && pos == context->part)
    context->matches++;
}

static void tech_check_loc(MuxEvent *e, void *data) {
  TechCheckContext *context = data;
  long loc;

  loc = (((long)e->data2) % 16);
  if (loc == context->location)
    context->matches++;
}

#define CHECK(t, fun)                                                          \
  TechCheckContext check = {.location = loc, .part = part};                    \
  mech_event_visit(mech, t, fun, &check);                                      \
  return check.matches

#define CHECKL(t, fun)                                                         \
  TechCheckContext check = {.location = loc};                                  \
  mech_event_visit(mech, t, fun, &check);                                      \
  return check.matches

#define CHECK2(t, t2, fun)                                                     \
  TechCheckContext check = {.location = loc, .part = part};                    \
  mech_event_visit(mech, t, fun, &check);                                      \
  mech_event_visit(mech, t2, fun, &check);                                     \
  return check.matches

/* Replace/reload */
int SomeoneRepairing_s(Mech *mech, int loc, int part, int t) {
  CHECK(t, tech_check_locpart);
}

#define DAT(t)                                                                 \
  if (SomeoneRepairing_s(mech, loc, part, t))                                  \
  return 1

int SomeoneRepairing(Mech *mech, int loc, int part) {
  DAT(EVENT_REPAIR_RELO);
  DAT(EVENT_REPAIR_REPL);
  DAT(EVENT_REPAIR_REPLG);
  DAT(EVENT_REPAIR_REPAP);
  DAT(EVENT_REPAIR_REPAG);
  DAT(EVENT_REPAIR_MOB);
  DAT(EVENT_REPAIR_REPENHCRIT);
  return 0;
}

/* Fixinternal/armor */
int SomeoneFixingA(Mech *mech, int loc) {
  CHECKL(EVENT_REPAIR_FIX, tech_check_loc);
}

int SomeoneFixingI(Mech *mech, int loc) {
  CHECKL(EVENT_REPAIR_FIXI, tech_check_loc);
}

int SomeoneFixing(Mech *mech, int loc) {
  return SomeoneFixingA(mech, loc) || SomeoneFixingI(mech, loc);
}

/* Reattach */
int SomeoneAttaching(Mech *mech, int loc) {
  CHECKL(EVENT_REPAIR_REAT, tech_check_loc);
}

int SomeoneReplacingSuit(Mech *mech, int loc) {
  CHECKL(EVENT_REPAIR_REPSUIT, tech_check_loc);
}

/* Reseal
 *
 * Added by Kipsta
 * 8/4/99
 */

int SomeoneResealing(Mech *mech, int loc) {
  CHECKL(EVENT_REPAIR_RESE, tech_check_loc);
}

int SomeoneScrappingLoc(Mech *mech, int loc) {
  CHECKL(EVENT_REPAIR_SCRL, tech_check_loc);
}

int SomeoneScrappingPart(Mech *mech, int loc, int part) {
  DAT(EVENT_REPAIR_SCRP);
  DAT(EVENT_REPAIR_SCRG);
  DAT(EVENT_REPAIR_UMOB);
  return 0;
}

#undef CHECK
#undef CHECK2
#undef DAT

int CanScrapLoc(Mech *mech, int loc) {
  TechCheckContext check = {.location = loc % 8};

  mech_event_visit(mech, EVENT_REPAIR_REPL, tech_check_loc, &check);
  mech_event_visit(mech, EVENT_REPAIR_RELO, tech_check_loc, &check);
  return !check.matches && !SomeoneFixing(mech, loc);
}

int CanScrapPart(Mech *mech, int loc, int part) {
  return !(SomeoneRepairing(mech, loc, part));
}

int ValidGunPos(Mech *mech, int loc, int pos) {
  unsigned char weaparray_f[MAX_WEAPS_SECTION];
  unsigned char weapdata_f[MAX_WEAPS_SECTION];
  int critical_f[MAX_WEAPS_SECTION];
  int i, num_weaps_f;

  if ((num_weaps_f = FindWeapons_Advanced(mech, loc, weaparray_f, weapdata_f,
                                          critical_f, 1)) < 0)
    return 0;
  for (i = 0; i < num_weaps_f; i++)
    if (critical_f[i] == pos)
      return 1;
  return 0;
}

void tech_checkstatus(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BtechContext *context = mech_context(mech);
  EvaluationContext *evaluation = btech_context_evaluation(context);
  int i = figure_latest_tech_event(mech);
  UptimeText uptime;

  DOCHECK_CONTEXT(context, !i, "The mech's ready to rock!");
  uptime = uptime_text(game_lag_time(context, i));
  notify_printf(evaluation, player,
                "The 'mech has approximately %s until done.", uptime.text);
}
