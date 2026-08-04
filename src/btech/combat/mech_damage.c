#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2000 Thomas Wouters
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "eject_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_ammodump_api.h"
#include "mech_build_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_ood_api.h"
#include "mech_pickup_api.h"
#include "mech_stagger.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "pcombat_api.h"
#include "registry_api.h"
void DamageMech(Mech *wounded, Mech *attacker, int LOS, int attackPilot,
                int hitloc, int isrear, int iscritical, int damage,
                int intDamage, int cause, int bth, int wWeapIndx, int wAmmoMode,
                int tIgnoreSwarmers) {
  char locationBuff[20];
  char notificationBuff[80];
  char rearMessage[10];
  int transfer = 0;
  int was_transfer = 0;
  int kill = 0;
  BattleMap *map;
  int crits = 0;
  int tBlowDumpingAmmo = 0;
  int wSwarmerHitChance = 0;
  int wRoll = btech_random_roll(wounded->xcode.context);
  Mech *mechSwarmer;
  int tSnapTowLines = 0;
  Mech *towTarget;

  /* if:
     damage = -1 && intDamage>0
     - ammo expl
     damage = -2 && intDamage>0
     - transferred ammo expl
     damage = n && intDamage = 0
     - usual damage
     damage = n && intDamage = -1/-2
     - usual damage + transfer/+red enable */
  /* if damage>0 && !intDamage usual dam. */
  map = btech_context_get_map(attacker->xcode.context, attacker->mapindex);
  if ((map && MapIsCS(map)) || (MechStatus(wounded) & COMBAT_SAFE)) {
    if (wounded != attacker)
      mech_notify(attacker, MECHALL, "Your efforts only scratch the paint!");
    return;
  }

  /* Rare case something passes through. We're in WEAPONS_HOLD. Don't even allow
   * it */
  if (MechStatus2(attacker) & WEAPONS_HOLD) {
    if (wounded != attacker)
      mech_notify(attacker, MECHALL, "You are currently in weapons hold!");
  }

  /* See if we have suits on us. If we get hit in any rear torso or the
   * left/right front torsos, there's a chance the bsuits on us will suck up the
   * damage. In fasa rules, there's no roll, but that's foolish if there's only
   * one suits. 3030 rules are there's a 20 percent chance per suit on you that
   * the suits will eat up the damage.
   */
  if ((CountSwarmers(wounded) > 0) && (!tIgnoreSwarmers)) {
    if ((mechSwarmer = findSwarmers(wounded))) {
      if (!attacker || (attacker->mynum != mechSwarmer->mynum)) {
        wSwarmerHitChance = 20 * CountBSuitMembers(mechSwarmer);
        if (isrear) {
          if ((hitloc != CTORSO) && (hitloc != RTORSO) && (hitloc != LTORSO))
            wSwarmerHitChance = 0;
        } else {
          if ((hitloc != RTORSO) && (hitloc != LTORSO))
            wSwarmerHitChance = 0;
        }

        if ((wSwarmerHitChance >= wRoll) && (GetSectArmor(wounded, hitloc))) {
          if (attacker && (attacker->mynum != wounded->mynum)) {
            mech_notify(attacker, MECHALL,
                        "The battlesuits crawling all over your target absorb "
                        "the damage!");
          }

          mech_notify(
              wounded, MECHALL,
              "The battlesuits crawling all over you absorb the damage!");
          mech_notify(mechSwarmer, MECHALL, "You absorb the damage!");
          hitloc = FindHitLocation(mechSwarmer, 0, &iscritical, &isrear);
          DamageMech(mechSwarmer, attacker, LOS, attackPilot, hitloc, 0, 0,
                     damage, 0, cause, bth, wWeapIndx, wAmmoMode, 0);
          return;
        }
      }
    }
  }

  if (MechType(wounded) == CLASS_MW || MechType(wounded) == CLASS_MECH)
    transfer = 1;
#ifdef BT_MOVEMENT_MODES
  if ((damage > 0 || intDamage > 0) && MechStatus2(wounded) & SPRINTING) {
    MechStatus2(wounded) &= ~SPRINTING;
    MechLOSBroadcast(wounded, "breaks out of its sprint as it takes damage!");
    mech_notify(wounded, MECHALL,
                "You lose your sprinting momentum as you take damage!");
    if (!mech_event_count(wounded, EVENT_MOVEMODE))
      mech_event_schedule(wounded, EVENT_MOVEMODE, mech_movemode_event, TURN,
                          MODE_OFF | MODE_SPRINT);
  }

  if ((damage > 0 || intDamage > 0) && MechCritStatus(wounded) & HIDDEN) {
    MechCritStatus(wounded) &= ~HIDDEN;
    MechLOSBroadcast(wounded, "loses its cover as it takes damage!");
    mech_notify(wounded, MECHALL, "Your cover is ruined as you take damage!");
    if (!mech_event_count(wounded, EVENT_MOVEMODE))
      MechCritStatus(wounded) &= ~HIDDEN;
  }

  if ((damage > 0 || intDamage > 0) &&
      (mech_move_mode_locked(wounded) &&
       !(mech_event_first_delay(wounded, EVENT_MOVEMODE) &
         (MODE_EVADE | MODE_DODGE | MODE_OFF)))) {
    mech_event_cancel(wounded, EVENT_MOVEMODE);
    mech_notify(wounded, MECHALL,
                "Your movement mode changes are cancelled as you take damage!");
  }
#endif
  if (damage > 0 && intDamage == 0) {
    /* If we're a VTOL and the hitloc is the rotor,
       we'll cut the damage by some value */
    if ((MechType(wounded) == CLASS_VTOL) && (hitloc == ROTOR)) {
      if (wounded->xcode.context->configuration->btech_divrotordamage > 0)
        damage = damage /
                 wounded->xcode.context->configuration->btech_divrotordamage;
      if (damage < 1)
        damage = 1;
    }

    if (MechCritStatus(wounded) & HIDDEN) {
      mech_notify(wounded, MECHALL, "Your cover is ruined as you take damage!");
      MechLOSBroadcast(wounded, "loses its cover as it takes damage.");
      MechCritStatus(wounded) &= ~HIDDEN;
    }

    if (wounded->xcode.context->combat_overrides.damage_experience ==
        BTECH_DAMAGE_XP_GUNNERY)
      AccumulateGunXP(attackPilot, attacker, wounded, damage, 1, cause, bth);
    else if (wounded->xcode.context->combat_overrides.damage_experience ==
             BTECH_DAMAGE_XP_PILOTING)
      if (!Destroyed(wounded) &&
          is_in_character(wounded->xcode.context->database, wounded->mynum) &&
          MechTeam(wounded) != MechTeam(attacker))
        if (MechType(wounded) != CLASS_MW || MechType(attacker) == CLASS_MW)
          AccumulatePilXP(attackPilot, attacker, damage / 3, 1);
    damage = dam_to_pc_conversion(wounded, cause, damage);
  }
  if (isrear) {
    if (!(MechSpecials(wounded) & SALVAGE_TECH) &&
        (btech_random_roll(wounded->xcode.context) <= 5) &&
        (hitloc == CTORSO || hitloc == LTORSO || hitloc == RTORSO))
      tSnapTowLines = 1;

    if (MechType(wounded) == CLASS_MECH) {
      strcpy(rearMessage, "(Rear)");
      if (mech_event_count(wounded, EVENT_DUMP) &&
          ((hitloc == CTORSO) || (hitloc == LTORSO) || (hitloc == RTORSO)) &&
          (cause >= 0))
        tBlowDumpingAmmo = 1;
    } else {
      if (hitloc == FSIDE)
        hitloc = BSIDE;
      *rearMessage = '\0';
      isrear = 0;
    }
  } else
    *rearMessage = '\0';
  /* Damage something else, ok? */
  if (damage < 0) {
    switch (damage) {
    case -2:
      was_transfer = 1;
      [[fallthrough]];
    case -1:
      transfer = 1;
      break;
    }
    damage = 0;
  } else if (intDamage < 0) {
    switch (intDamage) {
    case -2:
      was_transfer = 1;
      [[fallthrough]];
    case -1:
      transfer = 1;
      break;
    }
    intDamage = 0;
  }

  /*   while (SectIsDestroyed(wounded, hitloc) && !kill) */
  while (((!is_aero(wounded) && !GetSectInt(wounded, hitloc)) ||
          (is_aero(wounded) && !GetSectArmor(wounded, hitloc))) &&
         !kill) {
    if (transfer && (hitloc = TransferTarget(wounded, hitloc)) >= 0 &&
        (MechType(wounded) == CLASS_MECH || MechType(wounded) == CLASS_MW ||
         MechType(wounded) == CLASS_BSUIT || is_aero(wounded))) {
      DamageMech(wounded, attacker, LOS, attackPilot, hitloc, isrear,
                 iscritical, damage == -1 ? -2 : damage,
                 transfer == 1 ? -2 : damage, cause, bth, wWeapIndx, wAmmoMode,
                 tIgnoreSwarmers);
      return;
    } else {
      if (!((MechType(wounded) == CLASS_MECH || MechType(wounded) == CLASS_MW ||
             MechType(wounded) == CLASS_BSUIT || is_aero(wounded)) &&
            (hitloc = TransferTarget(wounded, hitloc)) >= 0)) {
        if (is_aero(wounded) && !Destroyed(wounded)) {
          /* Hurt SI instead. */
          if (AeroSI(wounded) <= damage)
            kill = 1;
          else {
            AeroSI(wounded) -= damage;
            kill = -1;
          }
        } else
          return;
      }
      /* Nyah. Damage transferred to waste, shooting a dead mech? */
    }
  }
  if (C_OODing(wounded) && btech_random_roll(wounded->xcode.context) > 8) {
    mech_ood_damage(wounded, attacker,
                    damage + (intDamage < 0 ? 0 : intDamage));
    return;
  }

  if (hitloc != -1) {
    ArmorStringFromIndex(hitloc, locationBuff, MechType(wounded),
                         MechMove(wounded));
    snprintf(notificationBuff, sizeof(notificationBuff),
             "for %d points of damage in the %s %s",
             damage + (intDamage < 0 ? 0 : intDamage), locationBuff,
             rearMessage);
  } else
    snprintf(notificationBuff, sizeof(notificationBuff),
             "for %d points of damage in the structure.",
             damage + (intDamage < 0 ? 0 : intDamage));

  /* Only count initial damage. Transfer is just gonna do that, transfer, not
   * damage again */
  if (!was_transfer) {
    if (attacker != wounded)
      MechDamageInflicted(attacker) = MechDamageInflicted(attacker) + damage +
                                      (intDamage < 0 ? 0 : intDamage);
    MechDamageTaken(wounded) =
        MechDamageTaken(wounded) + damage + (intDamage < 0 ? 0 : intDamage);
  }

  /*  if (LOS && attackPilot != -1) */
  if (LOS) {
    if (!was_transfer)
      mech_printf(attacker, MECHALL, "[fg=green]You hit %s[reset]",
                  notificationBuff);
    else
      mech_printf(attacker, MECHALL, "[fg=green]Damage transfer.. %s[reset]",
                  notificationBuff);
  }
  if (MechType(wounded) == CLASS_MW && !was_transfer)
    if (damage > 0)
      if (!(damage = armor_effect(wounded, cause, hitloc, damage, intDamage)))
        return;
  mech_printf(wounded, MECHALL, "[fg=yellow bold]You have been hit %s%s[reset]",
              notificationBuff, was_transfer ? "(transfer)" : "");
  /* Always a good policy :-> */
  if (damage > 0 && intDamage <= 0 && !was_transfer && !Fallen(wounded)) {
    if (wounded->xcode.context->configuration->btech_newstagger &&
        MechType(wounded) == CLASS_MECH) {

      mech_stagger_damage_append(wounded, damage,
                                 wounded->xcode.context->clock->now,
                                 attacker->mynum, false);
    } else {
      MechTurnDamage(wounded) += damage;
    }
  }

  if (hitloc == HEAD && MechType(wounded) == CLASS_MECH) {

    /*      mech_notify (wounded, MECHALL,
       "You take 10 points of Lethal damage!!"); */

    /* Rule Reference: BMR Revised, Page 16 (Head Hit = 1 Bruise) */
    /* Rule Reference: Total Warfare, Page 41 (Head Hit = 1 Bruise) */

    headhitmwdamage(wounded, attacker, 1);
  }
  if (kill) {
    if (kill == 1) {
      mech_notify(wounded, MECHALL,
                  "The blast causes the last of your craft's structure to "
                  "disintegrate, blowing");
      mech_notify(wounded, MECHALL, "its pieces all over the sky!");
      if (!Landed(wounded) && Started(wounded)) {
        mech_notify(attacker, MECHALL, "You shoot the craft from the sky!");
        MechLOSBroadcasti(attacker, wounded, "shoots %s from the sky!");
      }
      DestroyMech(wounded, attacker, !(!Landed(wounded) && Started(wounded)),
                  KILL_TYPE_NORMAL);
    }
    return;
  }
  if (damage > 0) {
    if (MechType(wounded) == CLASS_MECH) {
      if (!isrear && (MechSpecials(wounded) & SLITE_TECH) &&
          !(MechCritStatus(wounded) & SLITE_DEST) &&
          (hitloc == LTORSO || hitloc == CTORSO || hitloc == RTORSO)) {
        /* Possibly destroy the light */
        if (btech_random_roll(wounded->xcode.context) > 6) {
          if ((MechStatus2(wounded) & SLITE_ON) ||
              (btech_random_roll(wounded->xcode.context) > 5)) {
            MechCritStatus(wounded) |= SLITE_DEST;
            MechStatus2(wounded) &= ~SLITE_ON;
            MechLOSBroadcast(wounded, "'s searchlight is blown apart!");
            mech_notify(
                wounded, MECHALL,
                "[fg=yellow bold]Your searchlight is destroyed![reset]");
          }
        }
      }
    }
    if (MechType(wounded) == CLASS_VEH_GROUND) {
      if (!isrear && (MechSpecials(wounded) & SLITE_TECH) &&
          !(MechCritStatus(wounded) & SLITE_DEST) && (hitloc == FSIDE)) {
        /* Possibly destroy the light */
        if (btech_random_roll(wounded->xcode.context) > 6) {
          if ((MechStatus2(wounded) & SLITE_ON) ||
              (btech_random_roll(wounded->xcode.context) > 5)) {
            MechCritStatus(wounded) |= SLITE_DEST;
            MechStatus2(wounded) &= ~SLITE_ON;
            MechLOSBroadcast(wounded, "'s searchlight is blown apart!");
            mech_notify(
                wounded, MECHALL,
                "[fg=yellow bold]Your searchlight is destroyed![reset]");
          }
        }
      }
    }
    intDamage += cause_armordamage(wounded, attacker, LOS, attackPilot, isrear,
                                   iscritical, hitloc, damage, &crits,
                                   wWeapIndx, wAmmoMode);
    /* for Stat Engine */
    /* STATHIT|MAP|ATTACKER PILOT DBREF|WOUNDED PILOT DBREF|ATTACKER
     * MECHREF|WOUNDED MECHREF|ATTACKER MECH DBREF|WOUNDED MECH DBREF|BTH OF
     * SHOT|HITLOC|WEAPON NAME|Armor Damage|Internal Damage */
    /* The last part in the function is how we're handling transfer damage.
     * We're going to check how much internal is left, do some math, and only
     * count the applied damage */
    /* As the transfer damage will come back and send to another section via
     * DamageMech calls */
    /* We're going to skip wWeapindx = -1 for now as well. Those are physicals
     * (currently) and self inflicted */
    /* May make physicals -2, -3, -4, etc, but I'd rather not do all that logic.
     * Maybe change to -2 and just add a 'PHYSICAL'...Though kick vs punch would
     * be neat */
    /* LIGHTBULB:
     * Make a 'special/physical' weapons table. We'll send the wWeapindx as a
     * negative num. If its negative, (lower then -1 which will stay as
     * selfdamage) we'll check a 'Physical Weapons Table' and abs() the value
     * and pick the name out from there */
    if (wounded->xcode.context->configuration->btech_statengine_obj > 0 &&
        wWeapIndx != -1)
      notify_checked(
          btech_context_evaluation(wounded->xcode.context),
          wounded->xcode.context->configuration->btech_statengine_obj, GOD,
          tprintf("STATHIT|#%ld|#%ld|#%ld|%s|%s|#%ld|#%ld|%d|%s%s|%s|%d|%d",
                  attacker->mapindex, MechPilot(attacker), MechPilot(wounded),
                  MechType_Ref(attacker), MechType_Ref(wounded),
                  attacker->mynum, wounded->mynum, bth, isrear ? "Rear " : "",
                  hitloc != -1 ? locationBuff : "NONE",
                  &MechWeapons[wWeapIndx].name[0], damage - intDamage,
                  GetSectInt(wounded, hitloc) < intDamage
                      ? intDamage - (intDamage - GetSectInt(wounded, hitloc))
                      : intDamage),
          MSG_ME_ALL | MSG_F_DOWN);

    if (intDamage >= 0)
      MechFloodsLoc(wounded, hitloc, MechZ(wounded));
    if (intDamage > 0 && !is_aero(wounded)) {
      intDamage =
          cause_internaldamage(wounded, attacker, LOS, attackPilot, isrear,
                               hitloc, intDamage, cause, &crits);
      if (!intDamage && !SectIsDestroyed(wounded, hitloc))
        BreachLoc(attacker, wounded, hitloc);
    } else
      PossiblyBreach(attacker, wounded, hitloc);
    if (intDamage > 0 && transfer && (MechType(wounded) != CLASS_BSUIT)) {
      if ((hitloc = TransferTarget(wounded, hitloc)) >= 0)
        DamageMech(wounded, attacker, LOS, attackPilot, hitloc, isrear,
                   iscritical, intDamage, -2, cause, bth, wWeapIndx, wAmmoMode,
                   tIgnoreSwarmers);
      else {
        DestroyMech(wounded, attacker, 1, KILL_TYPE_NORMAL);
        return;
      }
    }
  } else
  /* Cause _INTERNAL_ HAVOC! :-) */
  /* Non-CASE things get _really_ hurt */
  {
    if (intDamage > 0) {
      if (is_aero(wounded))
        intDamage = cause_armordamage(wounded, attacker, LOS, attackPilot,
                                      isrear, iscritical, hitloc, intDamage,
                                      &crits, wWeapIndx, wAmmoMode);
      else
        intDamage =
            cause_internaldamage(wounded, attacker, LOS, attackPilot, isrear,
                                 hitloc, intDamage, cause, &crits);
      if (!SectIsDestroyed(wounded, hitloc))
        PossiblyBreach(attacker, wounded, hitloc);
      if (intDamage > 0 && transfer &&
          !((MechSections(wounded)[hitloc].config & CASE_TECH) ||
            (MechSpecials(wounded) & CLAN_TECH))) {
        if ((hitloc = TransferTarget(wounded, hitloc)) >= 0) {
          if (!is_aero(wounded))
            DamageMech(wounded, attacker, LOS, attackPilot, hitloc, isrear,
                       iscritical, -2, intDamage, cause, bth, wWeapIndx,
                       wAmmoMode, tIgnoreSwarmers);
          else
            DamageMech(wounded, attacker, LOS, attackPilot, hitloc, isrear,
                       iscritical, intDamage, -2, cause, bth, wWeapIndx,
                       wAmmoMode, tIgnoreSwarmers);
        } else {
          DestroyMech(wounded, attacker, 1, KILL_TYPE_NORMAL);
          return;
        }
      }
    }
  }

  /* Check to see if the tow lines should snap */
  if (tSnapTowLines && (MechCarrying(wounded) > 0)) {
    if ((towTarget = btech_context_get_mech(wounded->xcode.context,
                                            MechCarrying(wounded)))) {
      mech_notify(wounded, MECHALL, "The hit causes your tow line to let go!");
      mech_notify(towTarget, MECHALL, "Your tow lines go suddenly slack!");
      MechLOSBroadcast(wounded,
                       "'s tow lines release and flap freely behind it!");

      mech_dropoff(GOD, wounded, "");
    }
  }

  /* For now, only check IS PlasmaRifles. Can use this for Clan PlasmaCannon
   * later */
  if (wWeapIndx > 0) {
    if (strstr(MechWeapons[wWeapIndx].name, "IS.PlasmaRifle")) {
      if (MechType(wounded) == CLASS_MECH)
        Plasma_Hit(attacker, wounded, LOS);
    }
  }
  /* Check to see if we blow up ammo that's dumping. */
  if (tBlowDumpingAmmo) {
    BlowDumpingAmmo(wounded, attacker, hitloc);
  }
}

/* this takes care of setting all the criticals to CRIT_DESTROYED */
