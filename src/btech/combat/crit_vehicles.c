/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 */

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "btconfig.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "crit_api.h"
#include "econ_cmds_api.h"
#include "eject_api.h"
#include "failures.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_ammodump_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_pickup_api.h"
#include "mech_sensor.h"
#include "mech_tag_api.h"
#include "mech_tech_commands_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "random.h"
#include "registry_api.h"

void DoCargoInfantryCrit(Mech *objMech, int wLoc) {
  /*
   * If there's infantry in the unit, the infantry takes
   * damage as if the weapon that caused the crit hit them.
   *
   * If there's cargo, damage is to be determined.
   */

  mech_notify(
      objMech, MECHALL,
      "The shot pierces your armor yet fails to hit a critical system!");
}

void DoVehicleEngineHit(Mech *objMech, Mech *objAttacker) {
  /*
   * Vehicle engine is severly damaged.
   * The vehicle may not move or change facing.
   *
   * If the vehicle is a VTOL, it may land successfully.
   * If the VTOL is over a clear, paved, rough or building hex is can
   * make a pskill roll to avoid crashing. If the roll fails or it's
   * above any other terrain, the VTOL crashes.
   */

  if (Fallen(objMech)) {
    mech_notify(objMech, MECHALL,
                "Your destroyed engine takes another direct hit!");
    return;
  }

  mech_notify(objMech, MECHALL,
              "[fg=red bold]Your engine takes a direct hit![reset]");

  if (MechType(objMech) == CLASS_VTOL) {
    if (!Landed(objMech)) {
      if (mech_real_terrain_get(objMech) == GRASSLAND ||
          mech_real_terrain_get(objMech) == ROAD ||
          mech_real_terrain_get(objMech) == BUILDING) {

        if (MadePilotSkillRoll(objMech,
                               MechZ(objMech) - MechElevation(objMech))) {
          mech_notify(objMech, MECHALL, "You land safely!");
          MechStatus(objMech) |= LANDED;
          MechZ(objMech) = MechElevation(objMech);
          MechFZ(objMech) = ZSCALE * MechZ(objMech);
          mech_max_speed_set(objMech, 0.0);
          MechVerticalSpeed(objMech) = 0.0;
        }
      } else {
        mech_notify(objMech, MECHALL, "The ground rushes up to meet you!");
        mech_notify(objAttacker, MECHALL, "You knock the VTOL out of the sky!");
        mech_los_broadcast(objMech, "falls from the sky!");
        mech_fall(objMech, MechsElevation(objMech), 0);
      }
    }
  } else {
    mech_max_speed_set(objMech, 0.0);
    mech_make_fall(objMech);
  }
}

void DoVehicleFuelTankCrit(Mech *objMech, Mech *objAttacker) {
  /*
   * The fuel tank is breached causing the
   * the vehicle to explode. If the unit does not
   * have an ICE engine, treat this as an engine crit.
   */

  if (!(MechSpecials(objMech) & ICE_TECH)) {
    DoVehicleEngineHit(objMech, objAttacker);
    return;
  }

  mech_notify(objMech, MECHALL,
              "[fg=red bold]Your fuel tank explodes in a ball of fire![reset]");

  if (objMech != objAttacker)
    mech_los_broadcast(objMech, "explodes in a ball of fire!");

  MechZ(objMech) = MechElevation(objMech);
  MechFZ(objMech) = ZSCALE * MechZ(objMech);
  MechSpeed(objMech) = 0.0;
  MechVerticalSpeed(objMech) = 0.0;
  DestroyMech(objMech, objAttacker, 1, KILL_TYPE_FUELTANK);
  explode_unit(objMech, objAttacker);
}

void DoVehicleCrewStunnedCrit(Mech *objMech) {
  /*
   * For one turn the crew can not move over cruising
   * speed and not fire weapons/ram/use radio/etc, just turn.
   */
  MechTankCritStatus(objMech) |= CREW_STUNNED;
  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]The shot resonates throughout the crew compartment, "
      "temporarily stunning you![reset]");

  mech_stun_crew(objMech);
  limitSpeedToCruise(objMech);
}

void DoVehicleDriverCrit(Mech *objMech) {
  /*
   * Driver is hit, apply a +2 to all driving skills
   */

  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]Your vehicle's driver takes a piece of shrapnel, making "
      "it harder to control the vehicle![reset]");
  MechPilotSkillBase(objMech) += 2;
}

void DoVehicleSensorCrit(Mech *objMech) {
  /*
   * Add +1 to BTH for each sensor crit.
   */

  mech_notify(objMech, MECHALL,
              "[fg=red bold]Your sensor suite takes a hit![reset]");
  MechBTH(objMech) += 1;
}

void DoVehicleCommanderHit(Mech *objMech) {
  /*
   * Commander is hit. Vehicle suffers a +1 to BTH and driving
   * skills. Also does result of crew stunned.
   */

  mech_notify(objMech, MECHALL,
              "[fg=red bold]Your vehicle's commander takes a piece of "
              "shrapnel![reset]");
  MechPilotSkillBase(objMech) += 1;
  MechBTH(objMech) += 1;

  DoVehicleCrewStunnedCrit(objMech);
}

void DoVehicleCrewKilledCrit(Mech *objMech, Mech *objAttacker) {
  /*
   * The whole crew is instantly killed
   * leaving the tank 'destroyed' but fixable.
   */

  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]The shot ricochets around the crew compartment, instantly "
      "killing everyone![reset]");
  DestroyMech(objMech, objAttacker, 0, KILL_TYPE_PILOT);
  KillMechContentsIfIC(objMech);

  if (MechSpeed(objMech) != 0.0)
    mech_los_broadcast(objMech, "careens out of control and starts to slow!");

  mech_make_fall(objMech);
}

void DoVTOLCoPilotCrit(Mech *objMech) {
  /*
   * +1 BTH for weapons fire
   */

  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]Your VTOL's pilot takes a piece of shrapnel, making it "
      "harder to aim your weapons![reset]");
  MechBTH(objMech) += 1;
}

void DoVTOLPilotHit(Mech *objMech) {
  /*
   * +2 pskill rolls
   * Must make sucessful pskill roll
   * or fall 1 elevation.
   */

  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]Your VTOL's copilot takes a piece of shrapnel, making it "
      "harder to control the VTOL![reset]");
  MechPilotSkillBase(objMech) += 2;

  /* TODO: make vtol drop a level if it fails a pskill roll */
}

void DoVTOLRotorDestroyedCrit(Mech *objMech, Mech *objAttacker, int LOS) {
  /*
   * Rotors are destroyed sending the vehicle crashing to the ground,
   * if it's not already there.
   */

  if (SectIsDestroyed(objMech, ROTOR))
    return;

  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]The shot hits your fragile rotor mechanism![reset]");
  mech_los_broadcast(objMech, "'s rotor snaps into several parts!");
  DestroySection(objMech, objAttacker, LOS, ROTOR);

  if (!objAttacker) {
    mech_notify(objMech, MECHALL, "Your rotor has been destroyed!");
    mech_los_broadcast(objMech, "'s rotor has been destroyed!");
  }
}

void DoVTOLRotorDamagedCrit(Mech *objMech) {
  /*
   * -1MP
   */

  /*
   * KipstaFeature: if we have less than 1 MP left then we don't have
   * enough rotor speed to keep us aloft, logically... so let's blow the sucker
   * off and crash the VTOL.
   */
  if (MMaxSpeed(objMech) <= MP1) {
    DoVTOLRotorDestroyedCrit(objMech, NULL, 1);
    return;
  }

  mech_notify(objMech, MECHALL, "Your rotor is damaged!");

  if (!Fallen(objMech))
    mech_max_speed_lower(objMech, MP1);
}

void DoVTOLTailRotorDamagedCrit(Mech *objMech) {
  /*
   * May not move faster than cruising speed.
   * Turns slower.
   */

  if (MechTankCritStatus(objMech) & TAIL_ROTOR_DESTROYED)
    mech_notify(objMech, MECHALL,
                "Your damaged tail rotor suffers more damage!");
  else {
    MechTankCritStatus(objMech) |= TAIL_ROTOR_DESTROYED;
    mech_notify(
        objMech, MECHALL,
        "[fg=red bold]Your tail rotor is damaged, slowing you down![reset]");

    limitSpeedToCruise(objMech);
  }
}

void StartVTOLCrash(Mech *objMech) {
  if (!Fallen(objMech)) {
    MechSpeed(objMech) = 0.0;
    MechDesiredSpeed(objMech) = 0.0;
    mech_max_speed_set(objMech, 0.0);

    if (!Landed(objMech)) {
      mech_notify(objMech, MECHALL,
                  "You ponder F = ma, S = F/m, S = at^2 => S=agt^2 in relation "
                  "to the ground.");
      MechVerticalSpeed(objMech) = 0;
      mech_notify(objMech, MECHALL, "You start free-fall.. Enjoy the ride!");
      mech_los_broadcast(objMech, "starts to fall to the ground!");
      mech_event_schedule(objMech, EVENT_FALL, mech_fall_event, FALL_TICK, -1);

      /*
         MechVerticalSpeed(objMech) = 0;
         mech_notify(objMech, MECHALL, "You fall rapidly from the sky!");
         mech_los_broadcast(objMech, "plummets from the sky!");
         mech_fall(objMech, MechsElevation(objMech), 0);
       */
    }
  }
}

void HandleAdvFasaVehicleCrit(Mech *wounded, Mech *attacker, int LOS,
                              int hitloc, int num) {
  int wRoll = btech_random_roll(wounded->xcode.context);

  if (MechMove(wounded) == MOVE_NONE)
    return;

  if (wRoll < 6)
    return;

  mech_notify(wounded, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");

  switch (MechType(wounded)) {
  case CLASS_VEH_GROUND:
    switch (hitloc) {
    case FSIDE:
      switch (wRoll) {
      case 6:
        DoVehicleDriverCrit(wounded);
        break;

      case 7:
        DoWeaponJamCrit(wounded, hitloc);
        break;

      case 8:
        DoVehicleStablizerCrit(wounded, hitloc);
        break;

      case 9:
        DoVehicleSensorCrit(wounded);
        break;

      case 10:
        DoVehicleCommanderHit(wounded);
        break;

      case 11:
        DoWeaponDestroyedCrit(attacker, wounded, hitloc, LOS);
        break;

      case 12:
        DoVehicleCrewKilledCrit(wounded, attacker);
        break;
      }
      break;

    case LSIDE:
    case RSIDE:
      switch (wRoll) {
      case 6:
        DoCargoInfantryCrit(wounded, hitloc);
        break;

      case 7:
        DoWeaponJamCrit(wounded, hitloc);
        break;

      case 8:
        DoVehicleCrewStunnedCrit(wounded);
        break;

      case 9:
        DoVehicleStablizerCrit(wounded, hitloc);
        break;

      case 10:
        DoWeaponDestroyedCrit(attacker, wounded, hitloc, LOS);
        break;

      case 11:
        DoVehicleEngineHit(wounded, attacker);
        break;

      case 12:
        DoVehicleFuelTankCrit(wounded, attacker);
        break;
      }
      break;

    case BSIDE:
      switch (wRoll) {
      case 6:
        DoWeaponJamCrit(wounded, hitloc);
        break;

      case 7:
        DoCargoInfantryCrit(wounded, hitloc);
        break;

      case 8:
        DoVehicleStablizerCrit(wounded, hitloc);
        break;

      case 9:
        DoWeaponDestroyedCrit(attacker, wounded, hitloc, LOS);
        break;

      case 10:
        DoVehicleEngineHit(wounded, attacker);
        break;

      case 11:
        DoAmmunitionCrit(wounded, attacker, hitloc, LOS);
        break;

      case 12:
        DoVehicleFuelTankCrit(wounded, attacker);
        break;
      }
      break;

    case TURRET:
      switch (wRoll) {
      case 6:
        DoVehicleStablizerCrit(wounded, hitloc);
        break;

      case 7:
        DoTurretJamCrit(wounded);
        break;

      case 8:
        DoWeaponJamCrit(wounded, hitloc);
        break;

      case 9:
        DoTurretLockCrit(wounded);
        break;

      case 10:
        DoWeaponDestroyedCrit(attacker, wounded, hitloc, LOS);
        break;

      case 11:
        DoTurretBlownOffCrit(wounded, attacker, LOS);
        break;

      case 12:
        DoAmmunitionCrit(wounded, attacker, hitloc, LOS);
        break;
      }
      break;
    }
    break;

  case CLASS_VTOL:
    switch (hitloc) {
    case FSIDE:
      switch (wRoll) {
      case 6:
        DoVTOLCoPilotCrit(wounded);
        break;

      case 7:
        DoWeaponJamCrit(wounded, hitloc);
        break;

      case 8:
        DoVehicleStablizerCrit(wounded, hitloc);
        break;

      case 9:
        DoVehicleSensorCrit(wounded);
        break;

      case 10:
        DoVTOLPilotHit(wounded);
        break;

      case 11:
        DoWeaponDestroyedCrit(attacker, wounded, hitloc, LOS);
        break;

      case 12:
        DoVehicleCrewKilledCrit(wounded, attacker);
        break;
      }
      break;

    case LSIDE:
    case RSIDE:
      switch (wRoll) {
      case 6:
        DoWeaponJamCrit(wounded, hitloc);
        break;

      case 7:
        DoCargoInfantryCrit(wounded, hitloc);
        break;

      case 8:
        DoVehicleStablizerCrit(wounded, hitloc);
        break;

      case 9:
        DoWeaponDestroyedCrit(attacker, wounded, hitloc, LOS);
        break;

      case 10:
        DoVehicleEngineHit(wounded, attacker);
        break;

      case 11:
        DoAmmunitionCrit(wounded, attacker, hitloc, LOS);
        break;

      case 12:
        DoVehicleFuelTankCrit(wounded, attacker);
        break;
      }
      break;

    case BSIDE:
      switch (wRoll) {
      case 6:
        DoCargoInfantryCrit(wounded, hitloc);
        break;

      case 7:
        DoWeaponJamCrit(wounded, hitloc);
        break;

      case 8:
        DoVehicleStablizerCrit(wounded, hitloc);
        break;

      case 9:
        DoWeaponDestroyedCrit(attacker, wounded, hitloc, LOS);
        break;

      case 10:
        DoVehicleSensorCrit(wounded);
        break;

      case 11:
        DoVehicleEngineHit(wounded, attacker);
        break;

      case 12:
        DoVehicleFuelTankCrit(wounded, attacker);
        break;
      }
      break;

    case ROTOR:
      switch (wRoll) {
      case 6:
      case 7:
      case 8:
        DoVTOLRotorDamagedCrit(wounded);
        break;

      case 9:
      case 10:
        DoVTOLTailRotorDamagedCrit(wounded);
        break;

      case 11:
      case 12:
        DoVTOLRotorDestroyedCrit(wounded, attacker, LOS);
        break;
      }
      break;
    }
    break;
  }
}

void HandleVTOLCrit(Mech *wounded, Mech *attacker, int LOS, int hitloc,
                    int num) {
  mech_notify(wounded, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
  switch (btech_random_range(wounded->xcode.context, 0, 5)) {
  case 0:
    /* Crew killed */
    mech_notify(wounded, MECHALL, "Your cockpit is destroyed!");
    if (!Landed(wounded)) {
      mech_notify(attacker, MECHALL, "You knock the VTOL out of the sky!");
      mech_los_broadcast(wounded, "falls down from the sky!");
    }
    DestroyMech(wounded, attacker, 0, KILL_TYPE_COCKPIT);
    KillMechContentsIfIC(wounded);
    break;
  case 1:
    /* Weapon jams, set them recylcling maybe */
    /* hmm. nothing for now, tanks are so weak */
    JamMainWeapon(wounded);
    break;
  case 2:
    /* Engine Hit */
    mech_notify(wounded, MECHALL, "Your engine takes a direct hit!");
    if (!Landed(wounded)) {
      if (mech_real_terrain_get(wounded) == GRASSLAND ||
          mech_real_terrain_get(wounded) == ROAD ||
          mech_real_terrain_get(wounded) == BUILDING) {
        if (MadePilotSkillRoll(wounded,
                               MechZ(wounded) - MechElevation(wounded))) {
          mech_notify(wounded, MECHALL, "You land safely!");
          MechStatus(wounded) |= LANDED;
          MechZ(wounded) = MechElevation(wounded);
          MechFZ(wounded) = ZSCALE * MechZ(wounded);
          mech_max_speed_set(wounded, 0.0);
          MechVerticalSpeed(wounded) = 0.0;
        }
      } else {
        mech_notify(wounded, MECHALL, "The ground rushes up to meet you!");
        mech_notify(attacker, MECHALL, "You knock the VTOL out of the sky!");
        mech_los_broadcast(wounded, "falls from the sky!");
        mech_fall(wounded, MechsElevation(wounded), 0);
      }
    }
    mech_max_speed_set(wounded, 0.0);
    break;
  case 3:
    /* Crew Killed */
    mech_notify(wounded, MECHALL, "Your cockpit is destroyed!");
    if (!(MechStatus(wounded) & LANDED)) {
      mech_notify(attacker, MECHALL, "You knock the VTOL out of the sky!");
      mech_los_broadcast(wounded, "falls from the sky!");
    }

    DestroyMech(wounded, attacker, 0, KILL_TYPE_COCKPIT);

    KillMechContentsIfIC(wounded);
    break;
  case 4:
    /* Fuel Tank Explodes */
    mech_notify(wounded, MECHALL, "Your fuel tank explodes in a ball of fire!");
    if (wounded != attacker)
      mech_los_broadcast(wounded, "'s fuel tank explodes in a ball of fire!");
    MechZ(wounded) = MechElevation(wounded);
    MechFZ(wounded) = ZSCALE * MechZ(wounded);
    MechSpeed(wounded) = 0.0;
    MechVerticalSpeed(wounded) = 0.0;
    DestroyMech(wounded, attacker, 1, KILL_TYPE_FUELTANK);
    explode_unit(wounded, attacker);
    break;
  case 5:
    /* Ammo/Power Plant Explodes */
    mech_notify(wounded, MECHALL, "Your power plant explodes!");
    mech_los_broadcast(wounded, "'s power plant suddenly explodes!");
    MechZ(wounded) = MechElevation(wounded);
    MechFZ(wounded) = ZSCALE * MechZ(wounded);
    MechSpeed(wounded) = 0.0;
    MechVerticalSpeed(wounded) = 0.0;
    DestroyMech(wounded, attacker, 1, KILL_TYPE_POWERPLANT);
    if (!(MechSections(wounded)[BSIDE].config & CASE_TECH))
      explode_unit(wounded, attacker);
    else
      DestroySection(wounded, attacker, LOS, BSIDE);
  }
}
