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
#include "mech.h"
#include "mech_consistency_api.h"
#include "mech_events.h"
#include "mech_macros.h"
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

#define my_parsepart(loc, part)                                                \
  switch (tech_parsepart(mech, buffer, loc, part, NULL)) {                     \
  case -1:                                                                     \
    notify(evaluation, player, "Invalid section!");                            \
    return;                                                                    \
  case -2:                                                                     \
    notify(evaluation, player, "Invalid part!");                               \
    return;                                                                    \
  }

#define my_parsepart2(loc, part, brand)                                        \
  switch (tech_parsepart(mech, buffer, loc, part, brand)) {                    \
  case -1:                                                                     \
    notify(evaluation, player, "Invalid section!");                            \
    return;                                                                    \
  case -2:                                                                     \
    notify(evaluation, player, "Invalid part!");                               \
    return;                                                                    \
  }

#define my_parsegun(loc, part, brand)                                          \
  switch (tech_parsegun(mech, buffer, loc, part, brand)) {                     \
  case -1:                                                                     \
    notify(evaluation, player, "Invalid gun #!");                              \
    return;                                                                    \
  case -2:                                                                     \
    notify(evaluation, player, "Invalid object to replace with!");             \
    return;                                                                    \
  case -3:                                                                     \
    notify(evaluation, player,                                                 \
           "Invalid object type - not matching with original.");               \
    return;                                                                    \
  case -4:                                                                     \
    notify(evaluation, player,                                                 \
           "Invalid gun location - subscript out of range.");                  \
    return;                                                                    \
  }

#define ClanMod(num)                                                           \
  MAX(1, (((num) / ((MechSpecials(mech) & CLAN_TECH) ? 2 : 1))))

typedef struct TechCheckContext {
  int matches;
  int location;
  int part;
} TechCheckContext;
TECHCOMMANDH(tech_removegun) {
  TECHCOMMANDB;
  TECHCOMMANDC;
  my_parsegun(&loc, &part, NULL);
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That part's blown off! You can assume the gun's gone too!");
  DOCHECK_CONTEXT(mech->xcode.context, !IsWeapon(GetPartType(mech, loc, part)),
                  "That's no gun!");
  DOCHECK_CONTEXT(mech->xcode.context, PartIsDestroyed(mech, loc, part),
                  "That gun's gone already!");
  DOCHECK_CONTEXT(mech->xcode.context, !ValidGunPos(mech, loc, part),
                  "You can't remove middle of a gun!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneScrappingPart(mech, loc, part),
                  "Someone's scrapping it already!");
  DOCHECK_CONTEXT(mech->xcode.context, !CanScrapPart(mech, loc, part),
                  "Someone's tinkering with it already!");
  DOCHECK_CONTEXT(
      mech->xcode.context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no additional removals are "
      "possible!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  /* Ok.. Everything's valid (we hope). */
  if (tech_weapon_roll(player, mech, REMOVEG_DIFFICULTY) < 0) {
    START(
        "Ack! Your attempt is far from perfect, you try to recover the gun..");
    if (tech_weapon_roll(player, mech, REMOVEG_DIFFICULTY) < 0) {
      START("No good. Consider the part gone.");
      FAKEREPAIR(REMOVEG_TIME *
                     ClanMod(GetWeaponCrits(
                         mech, Weapon2I(GetPartType(mech, loc, part)))),
                 EVENT_REPAIR_SCRG, mech, PACK_LOCPOS_E(loc, part, mod));
      mod = 3;
      return;
    }
  }
  START("You start removing the gun..");
  STARTREPAIR(REMOVEG_TIME * ClanMod(GetWeaponCrits(
                                 mech, Weapon2I(GetPartType(mech, loc, part)))),
              mech, PACK_LOCPOS_E(loc, part, mod), mux_event_tickmech_removegun,
              EVENT_REPAIR_SCRG);
}

TECHCOMMANDH(tech_removepart) {
  TECHCOMMANDB;
  TECHCOMMANDC;
  my_parsepart(&loc, &part);
  DOCHECK_CONTEXT(mech->xcode.context,
                  (t = GetPartType(mech, loc, part)) == EMPTY,
                  "That location is empty!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That part's blown off! You can assume the part's gone too!");
  DOCHECK_CONTEXT(mech->xcode.context, IsWeapon(t),
                  "That's a gun - use removegun instead!");
  DOCHECK_CONTEXT(mech->xcode.context, PartIsDestroyed(mech, loc, part),
                  "That part's gone already!");
  DOCHECK_CONTEXT(mech->xcode.context, IsCrap(GetPartType(mech, loc, part)),
                  "That type isn't scrappable!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  t == Special(ENDO_STEEL) || t == Special(FERRO_FIBROUS) ||
                      t == Special(STEALTH_ARMOR) ||
                      t == Special(HVY_FERRO_FIBROUS) ||
                      t == Special(LT_FERRO_FIBROUS),
                  "That type of item can't be removed!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneScrappingPart(mech, loc, part),
                  "Someone's scrapping it already!");
  DOCHECK_CONTEXT(
      mech->xcode.context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no additional removals are "
      "possible!");
  DOCHECK_CONTEXT(mech->xcode.context, !CanScrapPart(mech, loc, part),
                  "Someone's tinkering with it already!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  /* Ok.. Everything's valid (we hope). */
  START("You start removing the part..");
  if (tech_roll(player, mech, REMOVEP_DIFFICULTY) < 0) {
    START(
        "Ack! Your attempt is far from perfect, you try to recover the part..");
    if (tech_roll(player, mech, REMOVEP_DIFFICULTY) < 0) {
      START("No good. Consider the part gone.");
      mod = 3;
      FAKEREPAIR(REMOVEP_TIME, EVENT_REPAIR_SCRP, mech,
                 PACK_LOCPOS_E(loc, part, mod));
      return;
    }
  }
  STARTREPAIR(REMOVEP_TIME, mech, PACK_LOCPOS_E(loc, part, mod),
              mux_event_tickmech_removepart, EVENT_REPAIR_SCRP);
}

#define CHECK_S(nloc)                                                          \
  if (!SectIsDestroyed(mech, nloc))                                            \
    return 1;                                                                  \
  if (Invalid_Scrap_Path(mech, nloc))                                          \
  return 1

#define CHECK(tloc, nloc)                                                      \
  case tloc:                                                                   \
    CHECK_S(nloc)

int Invalid_Scrap_Path(Mech *mech, int loc) {
  if (loc < 0)
    return 0;
  if (MechType(mech) != CLASS_MECH)
    return 0;
  switch (loc) {
    CHECK(CTORSO, HEAD);
    CHECK_S(LTORSO);
    CHECK_S(RTORSO);
    break;
    CHECK(LTORSO, LARM);
    break;
    CHECK(RTORSO, RARM);
    break;
  }
  return 0;
}

#undef CHECK
#undef CHECK_S

TECHCOMMANDH(tech_removesection) {
  TECHCOMMANDB;
  TECHCOMMANDC;
  my_parsepart(&loc, NULL);
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That section's gone already!");
  DOCHECK_CONTEXT(mech->xcode.context, Invalid_Scrap_Path(mech, loc),
                  "You need to remove the outer sections first!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneScrappingLoc(mech, loc),
                  "Someone's scrapping it already!");
  DOCHECK_CONTEXT(mech->xcode.context, !CanScrapLoc(mech, loc),
                  "Someone's tinkering with it already!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  /* Ok.. Everything's valid (we hope). */
  if (tech_roll(player, mech, REMOVES_DIFFICULTY) < 0)
    mod = 3;
  START("You start removing the section..");
  STARTREPAIR(REMOVES_TIME, mech, PACK_LOCPOS_E(loc, 0, mod),
              mux_event_tickmech_removesection, EVENT_REPAIR_SCRL);
}
