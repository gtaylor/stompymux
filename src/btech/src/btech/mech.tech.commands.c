/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech.h"
#include "failures.h"
#include "mech.events.h"
#include "mech.tech.h"
#include "mux/network/mux_event.h"
#include "p.bsuit.h"
#include "p.btechstats.h"
#include "p.econ.h"
#include "p.mech.consistency.h"
#include "p.mech.status.h"
#include "p.mech.tech.do.h"
#include "p.mech.tech.h"
#include "p.mech.utils.h"
#include <math.h>
#include <string.h>

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
  mux_event_visit_type_data(mech->xcode.context->events, t, (void *)mech, fun, \
                            &check);                                           \
  return check.matches

#define CHECKL(t, fun)                                                         \
  TechCheckContext check = {.location = loc};                                  \
  mux_event_visit_type_data(mech->xcode.context->events, t, (void *)mech, fun, \
                            &check);                                           \
  return check.matches

#define CHECK2(t, t2, fun)                                                     \
  TechCheckContext check = {.location = loc, .part = part};                    \
  mux_event_visit_type_data(mech->xcode.context->events, t, (void *)mech, fun, \
                            &check);                                           \
  mux_event_visit_type_data(mech->xcode.context->events, t2, (void *)mech,     \
                            fun, &check);                                      \
  return check.matches

/* Replace/reload */
int SomeoneRepairing_s(MECH *mech, int loc, int part, int t) {
  CHECK(t, tech_check_locpart);
}

#define DAT(t)                                                                 \
  if (SomeoneRepairing_s(mech, loc, part, t))                                  \
  return 1

int SomeoneRepairing(MECH *mech, int loc, int part) {
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
int SomeoneFixingA(MECH *mech, int loc) {
  CHECKL(EVENT_REPAIR_FIX, tech_check_loc);
}

int SomeoneFixingI(MECH *mech, int loc) {
  CHECKL(EVENT_REPAIR_FIXI, tech_check_loc);
}

int SomeoneFixing(MECH *mech, int loc) {
  return SomeoneFixingA(mech, loc) || SomeoneFixingI(mech, loc);
}

/* Reattach */
int SomeoneAttaching(MECH *mech, int loc) {
  CHECKL(EVENT_REPAIR_REAT, tech_check_loc);
}

int SomeoneReplacingSuit(MECH *mech, int loc) {
  CHECKL(EVENT_REPAIR_REPSUIT, tech_check_loc);
}

/* Reseal
 *
 * Added by Kipsta
 * 8/4/99
 */

int SomeoneResealing(MECH *mech, int loc) {
  CHECKL(EVENT_REPAIR_RESE, tech_check_loc);
}

int SomeoneScrappingLoc(MECH *mech, int loc) {
  CHECKL(EVENT_REPAIR_SCRL, tech_check_loc);
}

int SomeoneScrappingPart(MECH *mech, int loc, int part) {
  DAT(EVENT_REPAIR_SCRP);
  DAT(EVENT_REPAIR_SCRG);
  DAT(EVENT_REPAIR_UMOB);
  return 0;
}

#undef CHECK
#undef CHECK2
#undef DAT

int CanScrapLoc(MECH *mech, int loc) {
  TechCheckContext check = {.location = loc % 8};

  mux_event_visit_type_data(mech->xcode.context->events, EVENT_REPAIR_REPL,
                            (void *)mech, tech_check_loc, &check);
  mux_event_visit_type_data(mech->xcode.context->events, EVENT_REPAIR_RELO,
                            (void *)mech, tech_check_loc, &check);
  return !check.matches && !SomeoneFixing(mech, loc);
}

int CanScrapPart(MECH *mech, int loc, int part) {
  return !(SomeoneRepairing(mech, loc, part));
}

#define tech_gun_is_ok(a, b, c) !PartIsNonfunctional(a, b, c)

int ValidGunPos(MECH *mech, int loc, int pos) {
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
  MECH *mech = (MECH *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  int i = figure_latest_tech_event(mech);
  UptimeText uptime;

  DOCHECK_CONTEXT(mech->xcode.context, !i, "The mech's ready to rock!");
  uptime = uptime_text(game_lag_time(mech->xcode.context, i));
  notify_printf(evaluation, player,
                "The 'mech has approximately %s until done.", uptime.text);
}

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

int Invalid_Scrap_Path(MECH *mech, int loc) {
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

TECHCOMMANDH(tech_replacegun) {
  int brand = 0, ob = 0;

  int roll, rollmod, fixtime, base_fixtime, parttype, oparttype, fail_fixtime;

  TECHCOMMANDB;
  TECHCOMMANDC;
  my_parsegun(&loc, &part, &brand);
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsFlooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneRepairing(mech, loc, part),
                  "Someone's repairing that part already!");
  DOCHECK_CONTEXT(mech->xcode.context, !IsWeapon(GetPartType(mech, loc, part)),
                  "That's no gun!");
  DOCHECK_CONTEXT(mech->xcode.context, !ValidGunPos(mech, loc, part),
                  "You can't replace middle of a gun!");
  DOCHECK_CONTEXT(mech->xcode.context, !PartIsNonfunctional(mech, loc, part),
                  "That gun isn't hurtin'!");
  DOCHECK_CONTEXT(
      mech->xcode.context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  if (brand) {
    ob = GetPartBrand(mech, loc, part);
    SetPartBrand(mech, loc, part, brand);
  }

  /*        oparttype=GetPartType(mech,loc,part);
          parttype =   (IsActuator(oparttype) ? Cargo(S_ACTUATOR) :
             (oparttype == Special(ENGINE) ?
                 ((MechSpecials(mech) & XL_TECH) ? Cargo(XL_ENGINE) :
                      (MechSpecials(mech) & ICE_TECH) ? Cargo(IC_ENGINE) :
                           (MechSpecials(mech) & XXL_TECH) ? Cargo(XXL_ENGINE) :
                                (MechSpecials(mech) & CE_TECH) ?
     Cargo(COMP_ENGINE) : (MechSpecials(mech) & LE_TECH) ? Cargo(LIGHT_ENGINE) :
     oparttype) : (oparttype == Special(HEAT_SINK) && MechHasDHS(mech) ?
     Cargo(DOUBLE_HEAT_SINK) : oparttype)));
  */
  parttype = oparttype = GetPartType(mech, loc, part);

  DOCHECK_CONTEXT(
      mech->xcode.context,
      IsAmmo(GetPartType(mech, loc, part))
          ? 0
          : econ_find_items(
                mech->xcode.context,
                IsDS(mech) ? AeroBay(mech, 0)
                           : game_object_location(mech->xcode.context->database,
                                                  mech->mynum),
                parttype, GetPartBrand(mech, loc, part)) < 1,
      tprintf("Not enough units of %s in store.",
              part_name(mech->xcode.context, parttype,
                        GetPartBrand(mech, loc, part))
                  .text));

  notify_printf(evaluation, player, "You start replacing the gun...");
  rollmod =
      REPLACE_DIFFICULTY + WEAPTYPE_DIFFICULTY(GetPartType(mech, loc, part));
  roll = tech_weapon_roll(player, mech, rollmod);
  base_fixtime =
      REPLACEGUN_TIME *
      ClanMod(GetWeaponCrits(mech, Weapon2I(GetPartType(mech, loc, part))));
  fail_fixtime = (base_fixtime * 3) / 2;

  if (roll < 0) {
    notify_printf(
        evaluation, player,
        "Your attempt is unsuccessful, but you try to save the gun...");
    rollmod = REPLACE_DIFFICULTY;
    roll = tech_roll(player, mech, rollmod);
    if (roll < 0) {
      fixtime = fail_fixtime;
      notify_printf(evaluation, player,
                    "You muck around, wasting the gun for good...");
      /* part goes , 1.5 * techtime*/
      if (!(IsAmmo(GetPartType(mech, loc, part))))
        econ_change_items(mech->xcode.context,
                          IsDS(mech)
                              ? AeroBay(mech, 0)
                              : game_object_location(
                                    mech->xcode.context->database, mech->mynum),
                          parttype, GetPartBrand(mech, loc, part), -1);
      tech_addtechtime(mech->xcode.context, player, fixtime);
      mux_event_add(
          mech->xcode.context->events,
          MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
          EVENT_REPAIR_REPLG, very_fake_func, (void *)mech,
          (void *)(PACK_LOCPOS_E(loc, part, brand) + player * PLAYERPOS));

    } else {
      notify_printf(evaluation, player, "You manage to save the gun...");
      /* part doesn't go. 1.5 * techtime, but lets mod the fix time if
       * applicable*/
      /* We should really MIN(100,mod * roll) for the subtract to cap this out
       */
      if (roll == 0)
        fixtime = fail_fixtime;
      else
        fixtime =
            mech->xcode.context->configuration->btech_variable_techtime
                ? (fail_fixtime * 10) /
                      (1000 / (100 - (roll ? mech->xcode.context->configuration
                                                     ->btech_techtime_mod *
                                                 roll
                                           : 0)))
                : fail_fixtime;
      if (fail_fixtime - fixtime)
        notify_printf(
            evaluation, player, "Your skill manages to save %d minute%s",
            fail_fixtime - fixtime, fail_fixtime - fixtime == 1 ? "!" : "s!");
      tech_addtechtime(mech->xcode.context, player, fixtime);
      mux_event_add(
          mech->xcode.context->events,
          MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
          EVENT_REPAIR_REPLG, very_fake_func, (void *)mech,
          (void *)(PACK_LOCPOS_E(loc, part, brand) + player * PLAYERPOS));
    }

  } else {
    if (roll == 0)
      fixtime = base_fixtime;
    else
      fixtime =
          mech->xcode.context->configuration->btech_variable_techtime
              ? (base_fixtime * 10) /
                    (1000 / (100 - (roll ? mech->xcode.context->configuration
                                                   ->btech_techtime_mod *
                                               roll
                                         : 0)))
              : base_fixtime;
    if (base_fixtime - fixtime)
      notify_printf(
          evaluation, player, "Your skill manages to save %d minute%s",
          base_fixtime - fixtime, base_fixtime - fixtime == 1 ? "!" : "s!");
    if (!(IsAmmo(GetPartType(mech, loc, part))))
      econ_change_items(mech->xcode.context,
                        IsDS(mech)
                            ? AeroBay(mech, 0)
                            : game_object_location(
                                  mech->xcode.context->database, mech->mynum),
                        parttype, GetPartBrand(mech, loc, part), -1);
    tech_addtechtime(mech->xcode.context, player, fixtime);
    mux_event_add(
        mech->xcode.context->events,
        MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
        EVENT_REPAIR_REPLG, mux_event_tickmech_replacegun, (void *)mech,
        (void *)(PACK_LOCPOS_E(loc, part, brand) + player * PLAYERPOS));
  }

  /*
          DOTECH_LOCPOS(REPLACE_DIFFICULTY +
                                    WEAPTYPE_DIFFICULTY(GetPartType(mech, loc,
     part)), replaceg_fail, replaceg_succ, replace_econ, REPLACEGUN_TIME *
                                    ClanMod(GetWeaponCrits
                                                    (mech,
     Weapon2I(GetPartType(mech, loc, part)))), mech, PACK_LOCPOS_E(loc, part,
     brand), mux_event_tickmech_replacegun, EVENT_REPAIR_REPLG, "You start
     replacing the gun..", 1);

  */
  if (brand)
    SetPartBrand(mech, loc, part, ob);
}

TECHCOMMANDH(tech_repairgun) {
  int extra_hard = 0;

  TECHCOMMANDB;
  TECHCOMMANDC;
  /* Find the gun for us */
  my_parsegun(&loc, &part, NULL);
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsFlooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneRepairing(mech, loc, part),
                  "Someone's repairing that part already!");
  DOCHECK_CONTEXT(mech->xcode.context, !IsWeapon(GetPartType(mech, loc, part)),
                  "That's no gun!");
  DOCHECK_CONTEXT(mech->xcode.context, !ValidGunPos(mech, loc, part),
                  "You can't repair middle of a gun!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneScrappingPart(mech, loc, part),
                  "Someone's scrapping it already!");
  DOCHECK_CONTEXT(
      mech->xcode.context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  DOCHECK_CONTEXT(mech->xcode.context, PartIsDisabled(mech, loc, part),
                  "That gun can't be fixed yet!");

  if (PartIsDestroyed(mech, loc, part)) {
    if (GetWeaponCrits(mech, Weapon2I(GetPartType(mech, loc, part))) < 5 ||
        PartIsDestroyed(mech, loc, part + 1)) {
      notify(evaluation, player, "That gun is gone for good!");
      return;
    }
    extra_hard = 1;
  } else if (!PartTempNuke(mech, loc, part)) {
    notify(evaluation, player, "That gun isn't hurtin'!");
    return;
  }

  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  DOTECH_LOCPOS(REPAIR_DIFFICULTY +
                    WEAPTYPE_DIFFICULTY(GetPartType(mech, loc, part)) +
                    extra_hard,
                repairg_fail, repairg_succ, repair_econ, REPAIRGUN_TIME, mech,
                PACK_LOCPOS(loc, part), mux_event_tickmech_repairgun,
                EVENT_REPAIR_REPAP, "You start repairing the weapon..", 1);
}

TECHCOMMANDH(tech_fixenhcrit) {
  TECHCOMMANDB;
  TECHCOMMANDC;
  /* Find the gun for us */
  my_parsegun(&loc, &part, NULL);
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsFlooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneRepairing(mech, loc, part),
                  "Someone's repairing that part already!");
  DOCHECK_CONTEXT(mech->xcode.context, !IsWeapon(GetPartType(mech, loc, part)),
                  "That's no gun!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneScrappingPart(mech, loc, part),
                  "Someone's scrapping it already!");
  DOCHECK_CONTEXT(
      mech->xcode.context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  DOCHECK_CONTEXT(mech->xcode.context, PartIsDisabled(mech, loc, part),
                  "That gun can't be fixed yet!");

  if (!PartIsDamaged(mech, loc, part)) {
    notify(evaluation, player, "That gun isn't damaged!");
    return;
  }

  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  DOTECH_LOCPOS(ENHCRIT_DIFFICULTY, repairenhcrit_fail, repairenhcrit_succ,
                repairenhcrit_econ, REPAIRENHCRIT_TIME, mech,
                PACK_LOCPOS(loc, part), mux_event_tickmech_repairenhcrit,
                EVENT_REPAIR_REPENHCRIT, "You start repairing the weapon...",
                1);
}

TECHCOMMANDH(tech_replacepart) {
  TECHCOMMANDB;

  TECHCOMMANDC;

  int roll, rollmod, fixtime, base_fixtime, parttype, oparttype, fail_fixtime;

  my_parsepart(&loc, &part);
  DOCHECK_CONTEXT(mech->xcode.context,
                  (t = GetPartType(mech, loc, part)) == EMPTY,
                  "That location is empty!");
  DOCHECK_CONTEXT(mech->xcode.context, !PartIsNonfunctional(mech, loc, part),
                  "That part looks ok to me..");
  DOCHECK_CONTEXT(mech->xcode.context, IsCrap(GetPartType(mech, loc, part)),
                  "That part isn't hurtin'!");
  DOCHECK_CONTEXT(mech->xcode.context, IsWeapon(t),
                  "That's a weapon! Use replacegun instead.");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsFlooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneRepairing(mech, loc, part),
                  "Someone's repairing that part already!");
  DOCHECK_CONTEXT(
      mech->xcode.context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  /* little cheating here to get the proper part, since we aren't doing complex
   * repairs */
  oparttype = GetPartType(mech, loc, part);
  parttype =
      (IsActuator(oparttype)
           ? Cargo(S_ACTUATOR)
           : (oparttype == Special(ENGINE)
                  ? ((MechSpecials(mech) & XL_TECH)    ? Cargo(XL_ENGINE)
                     : (MechSpecials(mech) & ICE_TECH) ? Cargo(IC_ENGINE)
                     : (MechSpecials(mech) & XXL_TECH) ? Cargo(XXL_ENGINE)
                     : (MechSpecials(mech) & CE_TECH)  ? Cargo(COMP_ENGINE)
                     : (MechSpecials(mech) & LE_TECH)  ? Cargo(LIGHT_ENGINE)
                                                       : oparttype)
                  : (oparttype == Special(HEAT_SINK) && MechHasDHS(mech)
                         ? Cargo(DOUBLE_HEAT_SINK)
                         : oparttype)));

  DOCHECK_CONTEXT(
      mech->xcode.context,
      IsAmmo(GetPartType(mech, loc, part))
          ? 0
          : econ_find_items(
                mech->xcode.context,
                IsDS(mech) ? AeroBay(mech, 0)
                           : game_object_location(mech->xcode.context->database,
                                                  mech->mynum),
                parttype, GetPartBrand(mech, loc, part)) < 1,
      tprintf("Not enough units of %s in store.",
              part_name(mech->xcode.context, parttype,
                        GetPartBrand(mech, loc, part))
                  .text));

  notify_printf(evaluation, player, "You start replacing the part...");
  rollmod =
      REPLACE_DIFFICULTY + PARTTYPE_DIFFICULTY(GetPartType(mech, loc, part));
  roll = tech_roll(player, mech, rollmod);
  base_fixtime = REPLACEPART_TIME;
  fail_fixtime = (REPLACEPART_TIME * 3) / 2;

  if (roll < 0) {
    notify_printf(
        evaluation, player,
        "Your attempt is unsuccessful, but you try to save the part...");
    rollmod = rollmod + 1;
    roll = tech_roll(player, mech, rollmod);
    if (roll < 0) {
      fixtime = fail_fixtime;
      notify_printf(evaluation, player,
                    "You muck around, wasting the part for good...");
      /* part goes , 1.5 * techtime*/
      econ_change_items(mech->xcode.context,
                        IsDS(mech)
                            ? AeroBay(mech, 0)
                            : game_object_location(
                                  mech->xcode.context->database, mech->mynum),
                        parttype, GetPartBrand(mech, loc, part), -1);
      tech_addtechtime(mech->xcode.context, player, fixtime);
      mux_event_add(
          mech->xcode.context->events,
          MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
          EVENT_REPAIR_REPL, very_fake_func, (void *)mech,
          (void *)(PACK_LOCPOS(loc, part) + player * PLAYERPOS));

    } else {
      notify_printf(evaluation, player, "You manage to save the part...");
      /* part doesn't go. 1.5 * techtime, but lets mod the fix time if
       * applicable*/
      /* We should really MIN(100,mod * roll) for the subtract to cap this out
       */
      if (roll == 0)
        fixtime = fail_fixtime;
      else
        fixtime =
            mech->xcode.context->configuration->btech_variable_techtime
                ? (fail_fixtime * 10) /
                      (1000 / (100 - (roll ? mech->xcode.context->configuration
                                                     ->btech_techtime_mod *
                                                 roll
                                           : 0)))
                : fail_fixtime;
      if (fail_fixtime - fixtime)
        notify_printf(
            evaluation, player, "Your skill manages to save %d minute%s",
            fail_fixtime - fixtime, fail_fixtime - fixtime == 1 ? "!" : "s!");
      tech_addtechtime(mech->xcode.context, player, fixtime);
      mux_event_add(
          mech->xcode.context->events,
          MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
          EVENT_REPAIR_REPL, very_fake_func, (void *)mech,
          (void *)(PACK_LOCPOS(loc, part) + player * PLAYERPOS));
    }

  } else {
    if (roll == 0)
      fixtime = base_fixtime;
    else
      fixtime =
          mech->xcode.context->configuration->btech_variable_techtime
              ? (base_fixtime * 10) /
                    (1000 / (100 - (roll ? mech->xcode.context->configuration
                                                   ->btech_techtime_mod *
                                               roll
                                         : 0)))
              : base_fixtime;
    if (base_fixtime - fixtime)
      notify_printf(
          evaluation, player, "Your skill manages to save %d minute%s",
          base_fixtime - fixtime, base_fixtime - fixtime == 1 ? "!" : "s!");

    econ_change_items(
        mech->xcode.context,
        IsDS(mech)
            ? AeroBay(mech, 0)
            : game_object_location(mech->xcode.context->database, mech->mynum),
        parttype, GetPartBrand(mech, loc, part), -1);
    tech_addtechtime(mech->xcode.context, player, fixtime);
    mux_event_add(
        mech->xcode.context->events,
        MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
        EVENT_REPAIR_REPL, mux_event_tickmech_repairpart, (void *)mech,
        (void *)(PACK_LOCPOS(loc, part) + player * PLAYERPOS));
  }
  /*
          DOTECH_LOCPOS(REPLACE_DIFFICULTY +
                                    PARTTYPE_DIFFICULTY(GetPartType(mech, loc,
     part)), replacep_fail, replacep_succ, replace_econ, REPLACEPART_TIME, mech,
     PACK_LOCPOS(loc, part), mux_event_tickmech_repairpart, EVENT_REPAIR_REPL,
                                    "You start replacing the part..", 0);
  */
}

TECHCOMMANDH(tech_repairpart) {
  TECHCOMMANDB;

  TECHCOMMANDC;
  my_parsepart(&loc, &part);
  DOCHECK_CONTEXT(mech->xcode.context,
                  (t = GetPartType(mech, loc, part)) == EMPTY,
                  "That location is empty!");
  DOCHECK_CONTEXT(mech->xcode.context, PartIsDestroyed(mech, loc, part),
                  "That part is gone for good!");
  DOCHECK_CONTEXT(mech->xcode.context, PartIsDisabled(mech, loc, part),
                  "That part can't be repaired yet!");
  DOCHECK_CONTEXT(mech->xcode.context, !PartTempNuke(mech, loc, part),
                  "That part isn't hurtin'!");
  DOCHECK_CONTEXT(mech->xcode.context, IsCrap(GetPartType(mech, loc, part)),
                  "That part isn't hurtin'!");
  DOCHECK_CONTEXT(mech->xcode.context, IsWeapon(t),
                  "That's a weapon! Use repairgun instead.");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsFlooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneRepairing(mech, loc, part),
                  "Someone's repairing that part already!");
  DOCHECK_CONTEXT(
      mech->xcode.context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  DOTECH_LOCPOS(REPAIR_DIFFICULTY +
                    PARTTYPE_DIFFICULTY(GetPartType(mech, loc, part)),
                repairp_fail, repairp_succ, repair_econ, REPAIRPART_TIME, mech,
                PACK_LOCPOS(loc, part), mux_event_tickmech_repairpart,
                EVENT_REPAIR_REPAP, "You start repairing the part..", 0);
}

TECHCOMMANDH(tech_toggletype) {
  int atype;

  TECHCOMMANDB;

  DOCHECK_CONTEXT(
      mech->xcode.context,
      (!is_wizard(mech->xcode.context->database, player)) &&
          is_in_character(mech->xcode.context->database, mech->mynum),
      "This command only works in simpods!");
  my_parsepart2(&loc, &part, &atype);
  DOCHECK_CONTEXT(mech->xcode.context,
                  !IsAmmo((t = GetPartType(mech, loc, part))),
                  "That's no ammo!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  PartIsNonfunctional(mech, loc, part) ||
                      PartIsDisabled(mech, loc, part),
                  "The ammo compartment is nonfunctional!");
  DOCHECK_CONTEXT(mech->xcode.context, !atype,
                  "You need to give a type to toggle to (use - for normal)");
  DOCHECK_CONTEXT(mech->xcode.context,
                  (t = (valid_ammo_mode(mech, loc, part, atype))) < 0,
                  "That is invalid ammo type for this weapon!");
  GetPartAmmoMode(mech, loc, part) &= ~(AMMO_MODES);
  GetPartAmmoMode(mech, loc, part) |= t;
  SetPartData(mech, loc, part, FullAmmo(mech, loc, part));
  mech_notify(mech, MECHALL, "Ammo toggled.");
}

TECHCOMMANDH(tech_reload) {
  int atype;

  TECHCOMMANDB;
  TECHCOMMANDD;
  my_parsepart2(&loc, &part, &atype);
  DOCHECK_CONTEXT(mech->xcode.context,
                  !IsAmmo((t = GetPartType(mech, loc, part))),
                  "That's no ammo!");
  DOCHECK_CONTEXT(
      mech->xcode.context, PartIsNonfunctional(mech, loc, part),
      "The ammo compartment is destroyed ; repair/replacepart it first.");
  DOCHECK_CONTEXT(
      mech->xcode.context, PartIsDisabled(mech, loc, part),
      "The ammo compartment is disabled ; repair/replacepart it first.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  (now = GetPartData(mech, loc, part)) ==
                      (full = FullAmmo(mech, loc, part)),
                  "That particular ammo compartment doesn't need reloading.");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneRepairing(mech, loc, part),
                  "Someone's playing with that part already!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsFlooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(
      mech->xcode.context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  if (atype) {
    DOCHECK_CONTEXT(mech->xcode.context,
                    (t = (valid_ammo_mode(mech, loc, part, atype))) < 0,
                    "That is invalid ammo type for this weapon!");
    SetPartData(mech, loc, part, 0);
    GetPartAmmoMode(mech, loc, part) &= ~(AMMO_MODES);
    GetPartAmmoMode(mech, loc, part) |= t;
  }
  change = 0;

  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  DOTECH_LOCPOS_VAL(RELOAD_DIFFICULTY, reload_fail, reload_succ, reload_econ,
                    &change, RELOAD_TIME, mech,
                    PACK_LOCPOS_E(loc, part, change), mux_event_tickmech_reload,
                    EVENT_REPAIR_RELO,
                    "You start reloading the ammo compartment..");
}

TECHCOMMANDH(tech_unload) {
  TECHCOMMANDB;

  TECHCOMMANDD;
  my_parsepart(&loc, &part);
  DOCHECK_CONTEXT(mech->xcode.context,
                  !IsAmmo((t = GetPartType(mech, loc, part))),
                  "That's no ammo!");
  DOCHECK_CONTEXT(
      mech->xcode.context, PartIsNonfunctional(mech, loc, part),
      "The ammo compartment is destroyed ; repair/replacepart it first.");
  DOCHECK_CONTEXT(
      mech->xcode.context, PartIsDisabled(mech, loc, part),
      "The ammo compartment is disabled ; repair/replacepart it first.");
  DOCHECK_CONTEXT(mech->xcode.context, !(now = GetPartData(mech, loc, part)),
                  "That particular ammo compartment is empty already.");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneRepairing(mech, loc, part),
                  "Someone's playing with that part already!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsFlooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(
      mech->xcode.context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  if ((full = FullAmmo(mech, loc, part)) == now)
    change = 2;
  else
    change = 1;
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  if (tech_roll(player, mech, REMOVES_DIFFICULTY) < 0)
    mod = 3;
  START("You start unloading the ammo compartment..");
  STARTREPAIR(RELOAD_TIME, mech, PACK_LOCPOS_E(loc, part, change),
              mux_event_tickmech_reload, EVENT_REPAIR_RELO);
}

TECHCOMMANDH(tech_fixarmor) {
  int ochange;

  TECHCOMMANDB;

  TECHCOMMANDD;
  DOCHECK_CONTEXT(mech->xcode.context,
                  tech_parsepart_advanced(mech, buffer, &loc, NULL, NULL, 1) <
                      0,
                  "Invalid section!");
  if (loc >= 8) {
    from = GetSectRArmor(mech, loc % 8);
    to = GetSectORArmor(mech, loc % 8);
  } else {
    from = GetSectArmor(mech, loc);
    to = GetSectOArmor(mech, loc);
  }
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc % 8),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsFlooded(mech, loc % 8),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  SomeoneFixingA(mech, loc) || SomeoneFixingI(mech, loc % 8),
                  "Someone's repairing that section already!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  GetSectInt(mech, loc % 8) != GetSectOInt(mech, loc % 8),
                  "The internals need to be fixed first!");
  DOCHECK_CONTEXT(
      mech->xcode.context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  from = MIN(to, from);
  DOCHECK_CONTEXT(mech->xcode.context, from == to,
                  "The location doesn't need armor repair!");
  change = to - from;
  ochange = change;
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");
  DOTECH_LOC_VAL_S(FIXARMOR_DIFFICULTY, fixarmor_fail, fixarmor_succ,
                   fixarmor_econ, &change, FIXARMOR_TIME * ochange, loc,
                   EVENT_REPAIR_FIX, mech, "You start fixing the armor..");
  STARTIREPAIR(FIXARMOR_TIME * change, mech, (change * 16 + loc),
               mux_event_tickmech_repairarmor, EVENT_REPAIR_FIX, change);
}

TECHCOMMANDH(tech_fixinternal) {
  TECHCOMMANDB int ochange;

  TECHCOMMANDC;
  my_parsepart(&loc, NULL);
  from = GetSectInt(mech, loc);
  to = GetSectOInt(mech, loc);
  DOCHECK_CONTEXT(mech->xcode.context, from == to,
                  "The location doesn't need internals' repair!");
  change = to - from;
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(mech->xcode.context, SectIsFlooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneFixing(mech, loc),
                  "Someone's repairing that section already!");
  DOCHECK_CONTEXT(
      mech->xcode.context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  ochange = change;
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  DOTECH_LOC_VAL_S(FIXINTERNAL_DIFFICULTY, fixinternal_fail, fixinternal_succ,
                   fixinternal_econ, &change, FIXINTERNAL_TIME * ochange, loc,
                   EVENT_REPAIR_FIX, mech, "You start fixing the internals..");
  STARTIREPAIR(FIXINTERNAL_TIME * change, mech, (change * 16 + loc),
               mux_event_tickmech_repairinternal, EVENT_REPAIR_FIXI, change);
}

#define CHECK(tloc, nloc)                                                      \
  case tloc:                                                                   \
    if (SectIsDestroyed(mech, nloc))                                           \
      return 1;                                                                \
    break;

int Invalid_Repair_Path(MECH *mech, int loc) {
  if (MechType(mech) != CLASS_MECH)
    return 0;
  switch (loc) {
    CHECK(HEAD, CTORSO);
    CHECK(LTORSO, CTORSO);
    CHECK(RTORSO, CTORSO);
    CHECK(LARM, LTORSO);
    CHECK(RARM, RTORSO);
    CHECK(LLEG, CTORSO);
    CHECK(RLEG, CTORSO);
  }
  return 0;
}

int unit_is_fixable(MECH *mech) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (!GetSectOInt(mech, i))
      continue;
    if (!SectIsDestroyed(mech, i))
      continue;
    if (MechType(mech) == CLASS_MECH)
      if (i == CTORSO)
        return 0;
    if (MechType(mech) == CLASS_VTOL)
      if (i != ROTOR)
        return 0;
    if (MechType(mech) == CLASS_VEH_GROUND)
      if (i != TURRET)
        return 0;
  }
  return 1;
};

TECHCOMMANDH(tech_reattach) {
  TECHCOMMANDB;

  TECHCOMMANDC;

  int internal_stock = 0;
  int electric_stock = 0;
  int roll, rollmod, fixtime, base_fixtime, fail_fixtime;

  my_parsepart(&loc, NULL);
  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) == CLASS_BSUIT,
                  "You can't reattach a Battlesuit! Use 'replacesuit'!");
  DOCHECK_CONTEXT(mech->xcode.context, !SectIsDestroyed(mech, loc),
                  "That section isn't destroyed!");
  DOCHECK_CONTEXT(mech->xcode.context, Invalid_Repair_Path(mech, loc),
                  "You need to reattach adjacent locations first!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneAttaching(mech, loc),
                  "Someone's attaching that section already!");
  DOCHECK_CONTEXT(mech->xcode.context, !unit_is_fixable(mech),
                  "You see nothing to reattach it to (read:unit is cored).");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  internal_stock = econ_find_items(
      mech->xcode.context,
      IsDS(mech)
          ? AeroBay(mech, 0)
          : game_object_location(mech->xcode.context->database, mech->mynum),
      ProperInternal(mech), 0);
  electric_stock = econ_find_items(
      mech->xcode.context,
      IsDS(mech)
          ? AeroBay(mech, 0)
          : game_object_location(mech->xcode.context->database, mech->mynum),
      Cargo(S_ELECTRONIC), 0);

  DOCHECK_CONTEXT(
      mech->xcode.context, internal_stock < GetSectOInt(mech, loc),
      tprintf("Not enough %ss in stock. You need %d more.",
              part_name(mech->xcode.context, ProperInternal(mech), 0).text,
              GetSectOInt(mech, loc) - internal_stock));
  DOCHECK_CONTEXT(mech->xcode.context, electric_stock < GetSectOInt(mech, loc),
                  tprintf("Not enough Electrics in stock. You need %d more.",
                          GetSectOInt(mech, loc) - electric_stock));

  notify_printf(evaluation, player, "You start replacing the section...");
  rollmod = REATTACH_DIFFICULTY;
  roll = tech_roll(player, mech, rollmod);
  base_fixtime = REATTACH_TIME;
  fail_fixtime = (base_fixtime * 3) / 2;

  if (roll < 0) {
    notify_printf(
        evaluation, player,
        "Your attempt is unsuccessful, but you try to save the section...");
    rollmod = REATTACH_DIFFICULTY;
    roll = tech_roll(player, mech, rollmod);
    if (roll < 0) {
      fixtime = fail_fixtime;
      notify_printf(evaluation, player,
                    "You muck around, wasting the section for good...");
      /* TODO: maybe save X% of materials like before? */
      econ_change_items(mech->xcode.context,
                        IsDS(mech)
                            ? AeroBay(mech, 0)
                            : game_object_location(
                                  mech->xcode.context->database, mech->mynum),
                        ProperInternal(mech), 0, 0 - (GetSectOInt(mech, loc)));
      econ_change_items(mech->xcode.context,
                        IsDS(mech)
                            ? AeroBay(mech, 0)
                            : game_object_location(
                                  mech->xcode.context->database, mech->mynum),
                        Cargo(S_ELECTRONIC), 0, 0 - (GetSectOInt(mech, loc)));
      tech_addtechtime(mech->xcode.context, player, fixtime);
      mux_event_add(
          mech->xcode.context->events,
          MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
          EVENT_REPAIR_REAT, very_fake_func, (void *)mech,
          (void *)(loc + player * PLAYERPOS));

    } else {
      notify_printf(evaluation, player, "You manage to replace the section...");
      /* it's a saving roll, so it is what it is */
      if (roll == 0)
        fixtime = fail_fixtime;
      else
        fixtime =
            mech->xcode.context->configuration->btech_variable_techtime
                ? (fail_fixtime * 10) /
                      (1000 / (100 - (roll ? mech->xcode.context->configuration
                                                     ->btech_techtime_mod *
                                                 roll
                                           : 0)))
                : fail_fixtime;
      if (fail_fixtime - fixtime)
        notify_printf(
            evaluation, player, "Your skill manages to save %d minute%s",
            fail_fixtime - fixtime, fail_fixtime - fixtime == 1 ? "!" : "s!");
      econ_change_items(mech->xcode.context,
                        IsDS(mech)
                            ? AeroBay(mech, 0)
                            : game_object_location(
                                  mech->xcode.context->database, mech->mynum),
                        ProperInternal(mech), 0, 0 - (GetSectOInt(mech, loc)));
      econ_change_items(mech->xcode.context,
                        IsDS(mech)
                            ? AeroBay(mech, 0)
                            : game_object_location(
                                  mech->xcode.context->database, mech->mynum),
                        Cargo(S_ELECTRONIC), 0, 0 - (GetSectOInt(mech, loc)));
      tech_addtechtime(mech->xcode.context, player, fixtime);
      mux_event_add(
          mech->xcode.context->events,
          MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
          EVENT_REPAIR_REAT, mux_event_tickmech_reattach, (void *)mech,
          (void *)(loc + player * PLAYERPOS));
    }
  } else {
    if (roll == 0)
      fixtime = base_fixtime;
    else
      fixtime =
          mech->xcode.context->configuration->btech_variable_techtime
              ? (base_fixtime * 10) /
                    (1000 / (100 - (roll ? mech->xcode.context->configuration
                                                   ->btech_techtime_mod *
                                               roll
                                         : 0)))
              : base_fixtime;
    if (base_fixtime - fixtime)
      notify_printf(
          evaluation, player, "Your skill manages to save %d minute%s",
          base_fixtime - fixtime, base_fixtime - fixtime == 1 ? "!" : "s!");
    econ_change_items(
        mech->xcode.context,
        IsDS(mech)
            ? AeroBay(mech, 0)
            : game_object_location(mech->xcode.context->database, mech->mynum),
        ProperInternal(mech), 0, 0 - (GetSectOInt(mech, loc)));
    econ_change_items(
        mech->xcode.context,
        IsDS(mech)
            ? AeroBay(mech, 0)
            : game_object_location(mech->xcode.context->database, mech->mynum),
        Cargo(S_ELECTRONIC), 0, 0 - (GetSectOInt(mech, loc)));
    tech_addtechtime(mech->xcode.context, player, fixtime);
    mux_event_add(
        mech->xcode.context->events,
        MAX(1, player_techtime(mech->xcode.context, player) * TECH_TICK), 0,
        EVENT_REPAIR_REAT, mux_event_tickmech_reattach, (void *)mech,
        (void *)(loc + player * PLAYERPOS));
  }

  //	DOTECH_LOC(REATTACH_DIFFICULTY, reattach_fail, reattach_succ,
  //			   reattach_econ, REATTACH_TIME, mech, loc,
  //			   mux_event_tickmech_reattach, EVENT_REPAIR_REAT,
  //			   "You start replacing the section..");
}

TECHCOMMANDH(tech_replacesuit) {
  int wSuits = 0;

  TECHCOMMANDB;

  TECHCOMMANDC;
  my_parsepart(&loc, NULL);
  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) != CLASS_BSUIT,
                  "You can only use 'replacesuit' on a battlesuit unit!");

  wSuits = CountBSuitMembers(mech);

  DOCHECK_CONTEXT(
      mech->xcode.context, MechMaxSuits(mech) <= wSuits,
      tprintf("This %s is already full! This %s only consists of %d suits!",
              GetLCaseBSuitName(mech), GetLCaseBSuitName(mech),
              MechMaxSuits(mech)));
  DOCHECK_CONTEXT(mech->xcode.context, (loc >= MechMaxSuits(mech)) || (loc < 0),
                  tprintf("Invalid suit! This %s only consists of %d suits!",
                          GetLCaseBSuitName(mech), MechMaxSuits(mech)));

  DOCHECK_CONTEXT(mech->xcode.context, !SectIsDestroyed(mech, loc),
                  "That suit isn't destroyed!");

  DOCHECK_CONTEXT(mech->xcode.context, SomeoneReplacingSuit(mech, loc),
                  "Someone's already rebuilding that suit!");
  DOCHECK_CONTEXT(mech->xcode.context, wSuits <= 0,
                  "You are unable to replace the suits here! None of the "
                  "buggers are still alive!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  DOTECH_LOC(REPLACESUIT_DIFFICULTY, replacesuit_fail, replacesuit_succ,
             replacesuit_econ, REPLACESUIT_TIME, mech, loc,
             mux_event_tickmech_replacesuit, EVENT_REPAIR_REPSUIT,
             "You start replacing the missing suit.");
}

/*
 * Reseal
 * Added by Kipsta
 * 8/4/99
 */

TECHCOMMANDH(tech_reseal) {
  TECHCOMMANDB;

  TECHCOMMANDC;
  my_parsepart(&loc, NULL);
  DOCHECK_CONTEXT(mech->xcode.context, SectIsDestroyed(mech, loc),
                  "That section is destroyed!");
  DOCHECK_CONTEXT(mech->xcode.context, !SectIsFlooded(mech, loc),
                  "That has not been flooded!");
  DOCHECK_CONTEXT(mech->xcode.context, Invalid_Repair_Path(mech, loc),
                  "You need to reattach adjacent locations first!");
  DOCHECK_CONTEXT(mech->xcode.context, SomeoneResealing(mech, loc),
                  "Someone's sealing that section already!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  player_techtime(mech->xcode.context, player) >=
                      mech->xcode.context->configuration->btech_maxtechtime,
                  "You're too tired to do that!");

  DOTECH_LOC(RESEAL_DIFFICULTY, reseal_fail, reseal_succ, reseal_econ,
             RESEAL_TIME, mech, loc, mux_event_tickmech_reseal,
             EVENT_REPAIR_RESE, "You start resealing the section.");
}

TECHCOMMANDH(tech_fixextra) {
  TECHCOMMANDB;

  TECHCOMMANDC;
  notify(evaluation, player, "Fixed extra stuff - reseals, ammo feeds, etc.");
  do_fixextra(mech);
}

TECHCOMMANDH(tech_magic) {
  TECHCOMMANDB;

  TECHCOMMANDC;
  notify(evaluation, player, "Doing the magic..");
  do_magic(mech);
  mech_int_check(mech, 1);
  notify(evaluation, player, "Done!");
}
