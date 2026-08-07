
/*
 * $Id: mech.tech.h,v 1.5 2005/06/24 04:39:08 av1-op Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Fri Aug 30 15:22:08 1996 fingon
 * Last modified: Sat Jun  6 20:49:50 1998 fingon
 *
 */

#include "btech_event.h"
#include "legacy_macros.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_parts.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"

#pragma once

/* In minutes */
#if 1
constexpr int TECH_TICK = 60;
constexpr char TECH_UNIT[] = "minute";
#else
constexpr int TECH_TICK = 1;
constexpr char TECH_UNIT[] = "second";
#endif

/* Tech skill modifiers ; + = bad, - = good */
#define PARTTYPE_DIFFICULTY(a) (1)
#define WEAPTYPE_DIFFICULTY(a)                                                 \
  ((int)(sqrt((MechWeapons[Weapon2I(a)].criticals * 1.5) - 1.1)))
constexpr int REPAIR_DIFFICULTY = 0;
constexpr int REPLACE_DIFFICULTY = 1;
constexpr int RELOAD_DIFFICULTY = 1;
constexpr int FIXARMOR_DIFFICULTY = 1;
constexpr int FIXINTERNAL_DIFFICULTY = 2;
constexpr int REATTACH_DIFFICULTY = 3;
constexpr int REMOVEG_DIFFICULTY = 1;
constexpr int REMOVEP_DIFFICULTY = 0;
constexpr int REMOVES_DIFFICULTY = 2;
constexpr int RESEAL_DIFFICULTY = 0; /* Added 8/4/99. Kipsta. */
constexpr int REPLACESUIT_DIFFICULTY = 3;
constexpr int ENHCRIT_DIFFICULTY = 0;

/* Times are in minutes */
constexpr int MOUNT_BOMB_TIME = 5;
constexpr int UMOUNT_BOMB_TIME = 5;
constexpr int REPLACEGUN_TIME = 60;
constexpr int REPLACEPART_TIME = 45;
constexpr int REPAIRGUN_TIME = 20;
constexpr int REPAIRENHCRIT_TIME = 15;
constexpr int REPAIRPART_TIME = 15;
constexpr int RELOAD_TIME = 10;
constexpr int FIXARMOR_TIME = 3;
constexpr int FIXINTERNAL_TIME = 9;
constexpr int REATTACH_TIME = 240;
constexpr int REMOVEP_TIME = 40;
constexpr int REMOVEG_TIME = 40;
constexpr int REMOVES_TIME = 120;
constexpr int RESEAL_TIME = 60; /* Added 8/4/99. Kipsta. */
constexpr int REPLACESUIT_TIME = 120;

#define TECHCOMMANDH(a) void a(DbRef player, void *data, char *buffer)
#define TECHCOMMANDB                                                           \
  Mech *mech = (Mech *)data;                                                   \
  BtechContext *context = mech_context(mech);                                  \
  [[maybe_unused]] EvaluationContext *evaluation =                             \
      btech_context_evaluation(context);                                       \
  [[maybe_unused]] int loc, part, t, full, now, from, to, change, mod = 2,     \
                                                                  isds = 0;    \
  [[maybe_unused]] char *c;

#define TECHCOMMANDC                                                           \
  do {                                                                         \
    BtechContext *tech_context = mech ? mech_context(mech) : nullptr;          \
    DOCHECK_CONTEXT(                                                           \
        tech_context, !mech,                                                   \
        "Error has occured in techcommand ; please contact a wiz");            \
    isds = mech_is_dropship(mech);                                             \
    DOCHECK_CONTEXT(                                                           \
        tech_context,                                                          \
        mech_event_count(mech, EVENT_STARTUP) &&                               \
            !is_wizard(btech_context_database(tech_context), player),          \
        "The mech's starting up! Please stop the sequence first.");            \
    DOCHECK_CONTEXT(                                                           \
        tech_context,                                                          \
        mech_is_started(mech) &&                                               \
            !is_wizard(btech_context_database(tech_context), player),          \
        "The mech's started up ; please shut it down first.");                 \
    DOCHECK_CONTEXT(                                                           \
        tech_context,                                                          \
        !isds && mech_repair_stall_dbref(mech) <= 0 &&                         \
            !is_wizard(btech_context_database(tech_context), player),          \
        "The 'mech isn't in a repair stall!");                                 \
  } while (0)

#define TECHCOMMANDD                                                           \
  do {                                                                         \
    BtechContext *tech_context = mech ? mech_context(mech) : nullptr;          \
    DOCHECK_CONTEXT(                                                           \
        tech_context, !mech,                                                   \
        "Error has occured in techcommand ; please contact a wiz");            \
    isds = mech_is_dropship(mech);                                             \
    DOCHECK_CONTEXT(                                                           \
        tech_context,                                                          \
        mech_event_count(mech, EVENT_STARTUP) &&                               \
            !is_wizard(btech_context_database(tech_context), player),          \
        "The mech's starting up! Please stop the sequence first.");            \
    DOCHECK_CONTEXT(                                                           \
        tech_context,                                                          \
        mech_is_started(mech) &&                                               \
            !is_wizard(btech_context_database(tech_context), player),          \
        "The mech's started up ; please shut it down first.");                 \
    DOCHECK_CONTEXT(                                                           \
        tech_context,                                                          \
        btech_context_limits_repairs_to_stalls(tech_context) && !isds &&       \
            mech_repair_stall_dbref(mech) <= 0 &&                              \
            !is_wizard(btech_context_database(tech_context), player),          \
        "The 'mech isn't in a repair stall!");                                 \
  } while (0)

#define ETECHCOMMAND(a) void a(DbRef player, void *data, char *buffer)

constexpr int LOCMAX = 16;
constexpr int POSMAX = 16;
constexpr int EXTMAX = 256;
#define PLAYERPOS (LOCMAX * POSMAX * EXTMAX)

#define TECHEVENT(a)                                                           \
  void a(MuxEvent *e) {                                                        \
    Mech *mech = (Mech *)e->data;                                              \
    int earg = (int)(e->data2) % PLAYERPOS;

#define ETECHEVENT(a) extern void a(MuxEvent *e)

#define START(a) notify(evaluation, player, a)
#ifndef BT_FREETECHTIME
#define FIXEVENT(time, d1, d2, fu, type)                                       \
  btech_context_event_schedule(mech_context((Mech *)(d1)), (void *)(d1), type, \
                               fu, MAX(1, time),                               \
                               (intptr_t)((d2) + player * PLAYERPOS))
#else
#define FIXEVENT(time, d1, d2, fu, type)                                       \
  btech_context_event_schedule(                                                \
      mech_context((Mech *)(d1)), (void *)(d1), type, fu,                      \
      (btech_context_uses_free_technology_time(mech_context((Mech *)(d1)))     \
           ? 2                                                                 \
           : MAX(2, time)),                                                    \
      (intptr_t)((d2) + player * PLAYERPOS))
#endif
#define REPAIREVENT(time, d1, d2, fu, type)                                    \
  FIXEVENT((time) * TECH_TICK, d1, d2, fu, type)
#define STARTREPAIR(time, d1, d2, fu, type)                                    \
  FIXEVENT(                                                                    \
      tech_addtechtime(mech_context((Mech *)(d1)), player, (time * mod) / 2),  \
      d1, d2, fu, type)
#define STARTIREPAIR(time, d1, d2, fu, type, amount)                           \
  FIXEVENT((tech_addtechtime(mech_context((Mech *)(d1)), player,               \
                             (time * mod) / 2) -                               \
            (amount > 0 ? TECH_TICK * (time * (amount - 1) / (amount)) : 0)),  \
           d1, d2, fu, type)
#define FAKEREPAIR(time, type, d1, d2)                                         \
  FIXEVENT(                                                                    \
      tech_addtechtime(mech_context((Mech *)(d1)), player, (time * mod) / 2),  \
      d1, d2, very_fake_func, type)

/* replace gun/part, repair gun/part (loc/pos) */
#define DOTECH_LOCPOS(diff, flunkfunc, succfunc, resourcefunc, time, d1, d2,   \
                      fu, type, msg, isgun)                                    \
  if (resourcefunc(player, mech, loc, part) >= 0) {                            \
    START(msg);                                                                \
    if ((!isgun && tech_roll(player, mech, diff) < 0) ||                       \
        (isgun && tech_weapon_roll(player, mech, diff) < 0)) {                 \
      mod = 3;                                                                 \
      if (flunkfunc(player, mech, loc, part) < 0) {                            \
        FAKEREPAIR(time, type, d1, d2);                                        \
        return;                                                                \
      }                                                                        \
    } else {                                                                   \
      if (succfunc(player, mech, loc, part) < 0)                               \
        return;                                                                \
    }                                                                          \
    STARTREPAIR(time, d1, d2, fu, type);                                       \
  }

/* reload (loc/pos/amount) */
#define DOTECH_LOCPOS_VAL(diff, flunkfunc, succfunc, resourcefunc, amo, time,  \
                          d1, d2, fu, type, msg)                               \
  if (resourcefunc(player, mech, loc, part, amo) < 0)                          \
    return;                                                                    \
  START(msg);                                                                  \
  if (tech_roll(player, mech, diff) < 0) {                                     \
    mod = 3;                                                                   \
    if (flunkfunc(player, mech, loc, part, amo) < 0) {                         \
      FAKEREPAIR(time, type, d1, d2);                                          \
      return;                                                                  \
    }                                                                          \
  } else {                                                                     \
    if (succfunc(player, mech, loc, part, amo) < 0)                            \
      return;                                                                  \
  }                                                                            \
  STARTREPAIR(time, d1, d2, fu, type)

/* fixarmor/internal (loc/amount) */
#define DOTECH_LOC_VAL_S(diff, flunkfunc, succfunc, resourcefunc, amo, time,   \
                         type, d1, d2, msg)                                    \
  if (resourcefunc(player, mech, loc, amo) < 0)                                \
    return;                                                                    \
  START(msg);                                                                  \
  if (tech_roll(player, mech, diff) < 0) {                                     \
    mod = 3;                                                                   \
    if (flunkfunc(player, mech, loc, amo) < 0) {                               \
      FAKEREPAIR(time, type, d1, d2);                                          \
      return;                                                                  \
    }                                                                          \
  } else {                                                                     \
    if (succfunc(player, mech, loc, amo) < 0)                                  \
      return;                                                                  \
  }

#define DOTECH_LOC_VAL(diff, flunkfunc, succfunc, resourcefunc, amo, time, d1, \
                       d2, fu, type, msg)                                      \
  if (resourcefunc(player, mech, loc, amo) < 0)                                \
    return;                                                                    \
  START(msg);                                                                  \
  if (tech_roll(player, mech, diff) < 0) {                                     \
    mod = 3;                                                                   \
    if (flunkfunc(player, mech, loc, amo) < 0) {                               \
      FAKEREPAIR(time, type, d1, d2);                                          \
      return;                                                                  \
    }                                                                          \
  } else {                                                                     \
    if (succfunc(player, mech, loc, amo) < 0)                                  \
      return;                                                                  \
  }                                                                            \
  STARTREPAIR(time, d1, d2, fu, type)

/* reattach and reseal (loc) */
#define DOTECH_LOC(diff, flunkfunc, succfunc, resourcefunc, time, d1, d2, fu,  \
                   type, msg)                                                  \
  if (resourcefunc(player, mech, loc) < 0)                                     \
    return;                                                                    \
  START(msg);                                                                  \
  if (tech_roll(player, mech, diff) < 0) {                                     \
    mod = 3;                                                                   \
    if (flunkfunc(player, mech, loc) < 0) {                                    \
      FAKEREPAIR(time, type, d1, d2);                                          \
      return;                                                                  \
    }                                                                          \
  } else {                                                                     \
    if (succfunc(player, mech, loc) < 0)                                       \
      return;                                                                  \
  }                                                                            \
  STARTREPAIR(time, d1, d2, fu, type)

#define TFUNC_LOCPOS_VAL(name)                                                 \
  int name(DbRef player, Mech *mech, int loc, int part, int *val)
#define TFUNC_LOC_VAL(name)                                                    \
  int name(DbRef player, Mech *mech, int loc, int *val)
#define TFUNC_LOCPOS(name) int name(DbRef player, Mech *mech, int loc, int part)
#define TFUNC_LOC(name) int name(DbRef player, Mech *mech, int loc)
#define TFUNC_LOC_RESEAL(name) int name(DbRef player, Mech *mech, int loc)
#define NFUNC(a)                                                               \
  a { return 0; }

ETECHCOMMAND(tech_removegun);
ETECHCOMMAND(tech_removepart);
ETECHCOMMAND(tech_removesection);
ETECHCOMMAND(tech_replacegun);
ETECHCOMMAND(tech_repairgun);
ETECHCOMMAND(tech_fixenhcrit);
ETECHCOMMAND(tech_replacepart);
ETECHCOMMAND(tech_repairpart);
ETECHCOMMAND(tech_toggletype);
ETECHCOMMAND(tech_reload);
ETECHCOMMAND(tech_unload);
ETECHCOMMAND(tech_fixarmor);
ETECHCOMMAND(tech_fixinternal);
ETECHCOMMAND(tech_reattach);
ETECHCOMMAND(tech_checkstatus);
ETECHCOMMAND(tech_reseal);
ETECHCOMMAND(tech_replacesuit);
ECMD(show_mechs_damage);
ECMD(tech_fix);

#define PACK_LOCPOS(loc, pos) ((loc) + (pos) * LOCMAX)
#define PACK_LOCPOS_E(loc, pos, extra)                                         \
  ((loc) + (pos) * LOCMAX + (extra) * LOCMAX * POSMAX)

#define UNPACK_LOCPOS(var, loc, pos)                                           \
  loc = (var % LOCMAX);                                                        \
  pos = (var / LOCMAX) % POSMAX
#define UNPACK_LOCPOS_E(var, loc, pos, extra)                                  \
  UNPACK_LOCPOS(var, loc, pos);                                                \
  extra = var / (LOCMAX * POSMAX)

#ifndef BT_COMPLEXREPAIRS
int tech_proper_armor_part(const Mech *mech);
int tech_proper_internal_part(const Mech *mech);
#define ProperArmor(mech) tech_proper_armor_part(mech)
#define ProperInternal(mech) tech_proper_internal_part(mech)
#endif

ETECHEVENT(mux_event_tickmech_reattach);
ETECHEVENT(mux_event_tickmech_reseal);
ETECHEVENT(mux_event_tickmech_reload);
ETECHEVENT(mux_event_tickmech_removegun);
ETECHEVENT(mux_event_tickmech_removepart);
ETECHEVENT(mux_event_tickmech_removesection);
ETECHEVENT(mux_event_tickmech_repairarmor);
ETECHEVENT(mux_event_tickmech_repairgun);
ETECHEVENT(mux_event_tickmech_repairenhcrit);
ETECHEVENT(mux_event_tickmech_repairinternal);
ETECHEVENT(mux_event_tickmech_repairpart);
ETECHEVENT(mux_event_tickmech_replacegun);
ETECHEVENT(mux_event_tickmech_mountbomb);
ETECHEVENT(mux_event_tickmech_umountbomb);
ETECHEVENT(mux_event_tickmech_replacesuit);
ETECHEVENT(very_fake_func);

int valid_ammo_mode(Mech *mech, int loc, int part, int let);
