#include "mech_advanced_internal.h"

static void mech_mascr_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (MechMASCCounter(mech) > 0) {
    MechMASCCounter(mech)--;
    mech_event_schedule(mech, EVENT_MASC_REGEN, mech_mascr_event, MASC_TICK, 0);
  }
}

static void mech_masc_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
#ifndef BT_MOVEMENT_MODES
  int needed = 2 * (1 + (MechMASCCounter(mech)++));
#else
  int needed = (2 * (1 + (MechMASCCounter(mech)++))) +
               ((MechStatus2(mech) & SCHARGE_ENABLED) ? 1 : 0) +
               (MechStatus2(mech) & SPRINTING ? 2 : 0);
#endif
  int roll = btech_random_roll(mech->xcode.context);

  if (!Started(mech))
    return;
  if (!(MechSpecials(mech) & MASC_TECH))
    return;
  if (MechStatus(mech) & SCHARGE_ENABLED)
    roll--;
  if (needed < 10 &&
      is_good_obj(mech->xcode.context->database, MechPilot(mech)) &&
      is_wizard(mech->xcode.context->database, MechPilot(mech)))
    roll = btech_random_range(mech->xcode.context, needed + 1, 12);
  mech_printf(mech, MECHALL, "MASC: BTH %d+, Roll: %d", needed + 1, roll);
  if (roll > needed) {
    mech_event_schedule(mech, EVENT_MASC_FAIL, mech_masc_event, MASC_TICK, 0);
    return;
  }
  MechSpecials(mech) &= ~MASC_TECH;
  MechStatus(mech) &= ~MASC_ENABLED;
  if (fabs(MechSpeed(mech)) > MP1) {
    mech_notify(mech, MECHALL,
                "Your leg actuators freeze suddenly, and you fall!");
    mech_los_broadcast(mech, "stops and falls in mid-step!");
    mech_fall(mech, 1, 0);
  } else {
    mech_notify(mech, MECHALL, "Your leg actuators freeze suddenly!");
    if (MechSpeed(mech) > 0.0)
      mech_los_broadcast(mech, "stops suddenly!");
  }

  /* Break the Hips - FASA canon rule about MASC */
  DestroyPart(mech, RLEG, 0);
  DestroyPart(mech, LLEG, 0);
  /* Don't forget to add in Hipped penalties (for landing, etc) */
  MechSections(mech)[RLEG].basetohit += 2;
  MechSections(mech)[LLEG].basetohit += 2;
  if (MechMove(mech) == MOVE_QUAD) {
    DestroyPart(mech, RARM, 0);
    DestroyPart(mech, LARM, 0);
  }

  /* Let the MUX know both hips gone */
  MechCritStatus(mech) |= HIP_DAMAGED;
  MechCritStatus(mech) |= HIP_DESTROYED;

  /* Reset the Speeds, this sets all 3 of them */
  mech_max_speed_set(mech, 0.0);
}

void mech_masc(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  DOCHECK_CONTEXT(mech->xcode.context, !(MechSpecials(mech) & MASC_TECH),
                  "Your toy ain't prepared for what you're askin' it!");
  if (MechStatus(mech) & MASC_ENABLED) {
    mech_notify(mech, MECHALL, "MASC has been turned off.");
    MechStatus(mech) &= ~MASC_ENABLED;
    MechDesiredSpeed(mech) = MechDesiredSpeed(mech) * 3. / 4.;
    mech_event_cancel(mech, EVENT_MASC_FAIL);
    mech_event_schedule(mech, EVENT_MASC_REGEN, mech_mascr_event, MASC_TICK, 0);
    return;
  }
  DOCHECK_CONTEXT(mech->xcode.context, MMaxSpeed(mech) < MP1,
                  "You can't move. How is MASC going to work?");
  mech_notify(mech, MECHALL, "MASC has been turned on.");
  MechStatus(mech) |= MASC_ENABLED;
  mech_event_cancel(mech, EVENT_MASC_REGEN);
  MechDesiredSpeed(mech) = MechDesiredSpeed(mech) * 4. / 3.;
  mech_event_schedule(mech, EVENT_MASC_FAIL, mech_masc_event, 1, 0);
}

static void mech_scharger_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (MechSChargeCounter(mech) > 0) {
    MechSChargeCounter(mech)--;
    mech_event_schedule(mech, EVENT_SCHARGE_REGEN, mech_scharger_event,
                        SCHARGE_TICK, 0);
  }
}

static void mech_scharge_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
#ifndef BT_MOVEMENT_MODES
  int needed = 2 * (1 + (MechSChargeCounter(mech)++));
#else
  int needed = (2 * (1 + (MechMASCCounter(mech)++))) +
               ((MechStatus(mech) & MASC_ENABLED) ? 1 : 0) +
               (MechStatus2(mech) & SPRINTING ? 2 : 0);
#endif
  int roll = btech_random_roll(mech->xcode.context);
  int j, count = 0;
  int maxspeed, newmaxspeed = 0;
  int critType;
  char msgbuf[MBUF_SIZE] = {0};

  if (!Started(mech))
    return;
  if (!(MechSpecials2(mech) & SUPERCHARGER_TECH))
    return;
  if (MechStatus(mech) & MASC_ENABLED)
    roll = roll - 1;
  if (needed < 10 &&
      is_good_obj(mech->xcode.context->database, MechPilot(mech)) &&
      is_wizard(mech->xcode.context->database, MechPilot(mech)))
    roll = btech_random_range(mech->xcode.context, needed + 1, 12);
  mech_printf(mech, MECHALL, "Supercharger: BTH %d, Roll: %d", needed + 1,
              roll);
  if (roll > needed) {
    mech_event_schedule(mech, EVENT_SCHARGE_FAIL, mech_scharge_event,
                        SCHARGE_TICK, 0);
    return;
  }

  MechSpecials2(mech) &= ~SUPERCHARGER_TECH;
  MechStatus(mech) &= ~SCHARGE_ENABLED;

  mech_notify(mech, MECHALL, "Your supercharger overloads and explodes!");

  if (MechType(mech) == CLASS_MECH) {
    for (j = 0; j < CritsInLoc(mech, CTORSO); j++) {
      critType = GetPartType(mech, CTORSO, j);
      if (critType == Special(SUPERCHARGER)) {
        if (!PartIsDestroyed(mech, CTORSO, j))
          DestroyPart(mech, CTORSO, j);
      }
    }

    count = btech_random_range(mech->xcode.context, 1, 4);

    for (j = 0; count && j < CritsInLoc(mech, CTORSO); j++) {
      critType = GetPartType(mech, CTORSO, j);
      if (critType == Special(ENGINE) && !PartIsDestroyed(mech, CTORSO, j)) {
        DestroyPart(mech, CTORSO, j);
        if (!Destroyed(mech) && Started(mech)) {
          snprintf(msgbuf, MBUF_SIZE, "'s center torso spews black smoke!");
          mech_los_broadcast(mech, msgbuf);
        }
        if (MechEngineHeat(mech) < 10) {
          MechEngineHeat(mech) += 5;
          mech_notify(mech, MECHALL,
                      "Your engine shielding takes a hit!  It's getting hotter "
                      "in here!!");
        } else if (MechEngineHeat(mech) < 15) {
          MechEngineHeat(mech) = 15;
          mech_notify(mech, MECHALL, "Your engine is destroyed!!");
          DestroyMech(mech, mech, 1, KILL_TYPE_SCHARGE);
        }
        count--;
      }
    }
  }

  if ((MechType(mech) == CLASS_VTOL) || (MechType(mech) == CLASS_VEH_GROUND)) {
    snprintf(msgbuf, MBUF_SIZE, " coughs thick black smoke from its exhaust.");
    mech_los_broadcast(mech, msgbuf);
    maxspeed = MechMaxSpeed(mech);
    newmaxspeed = (maxspeed * .5);
    mech_max_speed_set(mech, newmaxspeed);
  }
}

void mech_scharge(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  DOCHECK_CONTEXT(mech->xcode.context,
                  !(MechSpecials2(mech) & SUPERCHARGER_TECH),
                  "Your toy ain't prepared for what you're askin' it!");
  if (MechStatus(mech) & SCHARGE_ENABLED) {
    mech_notify(mech, MECHALL, "Supercharger has been turned off.");
    MechStatus(mech) &= ~SCHARGE_ENABLED;
    MechDesiredSpeed(mech) = MechDesiredSpeed(mech) * 3. / 4.;
    mech_event_cancel(mech, EVENT_SCHARGE_FAIL);
    mech_event_schedule(mech, EVENT_SCHARGE_REGEN, mech_scharger_event,
                        SCHARGE_TICK, 0);
    return;
  }
  DOCHECK_CONTEXT(mech->xcode.context, MMaxSpeed(mech) < MP1,
                  "How much can you Supercharge if you can't move?");
  mech_notify(mech, MECHALL, "Supercharger has been turned on.");
  MechStatus(mech) |= SCHARGE_ENABLED;
  mech_event_cancel(mech, EVENT_SCHARGE_REGEN);
  MechDesiredSpeed(mech) = MechDesiredSpeed(mech) * 4. / 3.;
  mech_event_schedule(mech, EVENT_SCHARGE_FAIL, mech_scharge_event, 1, 0);
}

static void mech_dig_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (!Digging(mech))
    return;

  if (!Started(mech))
    return;

  MechTankCritStatus(mech) &= ~DIGGING_IN;
  MechTankCritStatus(mech) |= DUG_IN;
  mech_notify(mech, MECHALL,
              "You finish burrowing for cover - only turret weapons are "
              "available now.");
}

void mech_dig(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context, Fortified(mech),
                  "You are already fortified, there's no need to dig.");
  DOCHECK_CONTEXT(mech->xcode.context, fabs(MechSpeed(mech)) > 0.0,
                  "You are moving!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechFacing(mech) != MechDesiredFacing(mech),
                  "You are turning!");
  DOCHECK_CONTEXT(mech->xcode.context, MechMove(mech) == MOVE_NONE,
                  "You are stationary!");
  DOCHECK_CONTEXT(mech->xcode.context, MechDugIn(mech),
                  "You are already dug in!");
  DOCHECK_CONTEXT(mech->xcode.context, Digging(mech),
                  "You are already digging in!");
  DOCHECK_CONTEXT(mech->xcode.context, OODing(mech),
                  "While dropping? I think not.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_real_terrain_get(mech) == ROAD ||
                      mech_real_terrain_get(mech) == BRIDGE ||
                      mech_real_terrain_get(mech) == BUILDING ||
                      mech_real_terrain_get(mech) == WALL,
                  "The surface is slightly too hard for you to dig in.");
  DOCHECK_CONTEXT(mech->xcode.context, mech_real_terrain_get(mech) == WATER,
                  "In water? Who are you kidding?");

  MechTankCritStatus(mech) |= DIGGING_IN;
  mech_event_schedule(mech, EVENT_DIG, mech_dig_event, 20, 0);
  mech_notify(mech, MECHALL, "You start digging yourself in a nice hole..");
}

static void mech_unjam_turret_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (Destroyed(mech))
    return;

  if (Uncon(mech))
    return;

  if (!GetSectInt(mech, TURRET))
    return;

  if (!Started(mech))
    return;

  if (MechTankCritStatus(mech) & TURRET_LOCKED) {
    mech_notify(mech, MECHALL, "You are unable to unjam the turret!");
    return;
  }

  MechTankCritStatus(mech) &= ~TURRET_JAMMED;
  mech_notify(mech, MECHALL, "You manage to unjam your turret!");
}

void mech_fixturret(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context, MechTankCritStatus(mech) & TURRET_LOCKED,
                  "Your turret is locked! You need a repairbay to fix it!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  !(MechTankCritStatus(mech) & TURRET_JAMMED),
                  "Your turret is not jammed!");
  mech_event_schedule(mech, EVENT_UNJAM_TURRET, mech_unjam_turret_event, 60, 0);
  mech_notify(mech, MECHALL, "You start to repair your jammed turret.");
}
