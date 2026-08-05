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
#include "environment_damage_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_ammodump_api.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_damage_history_api.h"
#include "mech_ecm_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_ood_api.h"
#include "mech_pickup_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_stagger.h"
#include "mech_status_types.h"
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
#include "section_types.h"
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
  int wRoll = btech_random_roll(mech_context(wounded));
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
  map = btech_context_get_map(mech_context(attacker), mech_map_dbref(attacker));
  if ((map && battle_map_is_combat_safe(map)) ||
      mech_condition_summary(wounded).combat_safe) {
    if (wounded != attacker)
      mech_notify(attacker, MECHALL, "Your efforts only scratch the paint!");
    return;
  }

  /* Rare case something passes through. We're in WEAPONS_HOLD. Don't even allow
   * it */
  if (mech_condition_summary(attacker).weapons_hold) {
    if (wounded != attacker)
      mech_notify(attacker, MECHALL, "You are currently in weapons hold!");
  }

  /* See if we have suits on us. If we get hit in any rear torso or the
   * left/right front torsos, there's a chance the bsuits on us will suck up the
   * damage. In fasa rules, there's no roll, but that's foolish if there's only
   * one suits. 3030 rules are there's a 20 percent chance per suit on you that
   * the suits will eat up the damage.
   */
  if ((bsuit_swarmer_count(wounded) > 0) && (!tIgnoreSwarmers)) {
    if ((mechSwarmer = bsuit_swarmer_find(wounded))) {
      if (!attacker || (mech_dbref(attacker) != mech_dbref(mechSwarmer))) {
        wSwarmerHitChance = 20 * bsuit_member_count(mechSwarmer);
        if (isrear) {
          if ((hitloc != CTORSO) && (hitloc != RTORSO) && (hitloc != LTORSO))
            wSwarmerHitChance = 0;
        } else {
          if ((hitloc != RTORSO) && (hitloc != LTORSO))
            wSwarmerHitChance = 0;
        }

        if ((wSwarmerHitChance >= wRoll) &&
            mech_section_armor(wounded, hitloc)) {
          if (attacker && (mech_dbref(attacker) != mech_dbref(wounded))) {
            mech_notify(attacker, MECHALL,
                        "The battlesuits crawling all over your target absorb "
                        "the damage!");
          }

          mech_notify(
              wounded, MECHALL,
              "The battlesuits crawling all over you absorb the damage!");
          mech_notify(mechSwarmer, MECHALL, "You absorb the damage!");
          hitloc = mech_hit_location(mechSwarmer, 0, &iscritical, &isrear);
          DamageMech(mechSwarmer, attacker, LOS, attackPilot, hitloc, 0, 0,
                     damage, 0, cause, bth, wWeapIndx, wAmmoMode, 0);
          return;
        }
      }
    }
  }

  if (mech_class(wounded) == CLASS_MW || mech_class(wounded) == CLASS_MECH)
    transfer = 1;
#ifdef BT_MOVEMENT_MODES
  if ((damage > 0 || intDamage > 0) &&
      mech_condition_summary(wounded).sprinting) {
    mech_sprinting_set(wounded, false);
    mech_los_broadcast(wounded, "breaks out of its sprint as it takes damage!");
    mech_notify(wounded, MECHALL,
                "You lose your sprinting momentum as you take damage!");
    if (!mech_event_count(wounded, EVENT_MOVEMODE))
      mech_event_schedule(wounded, EVENT_MOVEMODE, mech_movemode_event, TURN,
                          MODE_OFF | MODE_SPRINT);
  }

  if ((damage > 0 || intDamage > 0) && mech_condition_summary(wounded).hidden) {
    mech_hidden_set(wounded, false);
    mech_los_broadcast(wounded, "loses its cover as it takes damage!");
    mech_notify(wounded, MECHALL, "Your cover is ruined as you take damage!");
    if (!mech_event_count(wounded, EVENT_MOVEMODE))
      mech_hidden_set(wounded, false);
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
    if ((mech_class(wounded) == CLASS_VTOL) && (hitloc == ROTOR)) {
      if (btech_context_rotor_damage_divisor(mech_context(wounded)) > 0)
        damage =
            damage / btech_context_rotor_damage_divisor(mech_context(wounded));
      if (damage < 1)
        damage = 1;
    }

    if (mech_condition_summary(wounded).hidden) {
      mech_notify(wounded, MECHALL, "Your cover is ruined as you take damage!");
      mech_los_broadcast(wounded, "loses its cover as it takes damage.");
      mech_hidden_set(wounded, false);
    }

    if (btech_context_damage_experience_mode(mech_context(wounded)) ==
        BTECH_DAMAGE_XP_GUNNERY)
      AccumulateGunXP(attackPilot, attacker, wounded, damage, 1, cause, bth);
    else if (btech_context_damage_experience_mode(mech_context(wounded)) ==
             BTECH_DAMAGE_XP_PILOTING)
      if (!mech_is_destroyed(wounded) &&
          is_in_character(btech_context_database(mech_context(wounded)),
                          mech_dbref(wounded)) &&
          mech_team(wounded) != mech_team(attacker))
        if (mech_class(wounded) != CLASS_MW || mech_class(attacker) == CLASS_MW)
          AccumulatePilXP(attackPilot, attacker, damage / 3, 1);
    damage = unit_damage_to_personal_combat(wounded, cause, damage);
  }
  if (isrear) {
    if (!(mech_technology_flags(wounded) & SALVAGE_TECH) &&
        (btech_random_roll(mech_context(wounded)) <= 5) &&
        (hitloc == CTORSO || hitloc == LTORSO || hitloc == RTORSO))
      tSnapTowLines = 1;

    if (mech_class(wounded) == CLASS_MECH) {
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
  while (((!mech_is_aerospace_unit(wounded) &&
           !mech_section_internal(wounded, hitloc)) ||
          (mech_is_aerospace_unit(wounded) &&
           !mech_section_armor(wounded, hitloc))) &&
         !kill) {
    if (transfer &&
        (hitloc = mech_hit_location_transfer(wounded, hitloc)) >= 0 &&
        (mech_class(wounded) == CLASS_MECH || mech_class(wounded) == CLASS_MW ||
         mech_class(wounded) == CLASS_BSUIT ||
         mech_is_aerospace_unit(wounded))) {
      DamageMech(wounded, attacker, LOS, attackPilot, hitloc, isrear,
                 iscritical, damage == -1 ? -2 : damage,
                 transfer == 1 ? -2 : damage, cause, bth, wWeapIndx, wAmmoMode,
                 tIgnoreSwarmers);
      return;
    } else {
      if (!((mech_class(wounded) == CLASS_MECH ||
             mech_class(wounded) == CLASS_MW ||
             mech_class(wounded) == CLASS_BSUIT ||
             mech_is_aerospace_unit(wounded)) &&
            (hitloc = mech_hit_location_transfer(wounded, hitloc)) >= 0)) {
        if (mech_is_aerospace_unit(wounded) && !mech_is_destroyed(wounded)) {
          /* Hurt SI instead. */
          if (mech_structural_integrity(wounded) <= damage)
            kill = 1;
          else {
            mech_structural_integrity_set(
                wounded, mech_structural_integrity(wounded) - damage);
            kill = -1;
          }
        } else
          return;
      }
      /* Nyah. Damage transferred to waste, shooting a dead mech? */
    }
  }
  if (mech_cocoon_integrity(wounded) > 0 &&
      btech_random_roll(mech_context(wounded)) > 8) {
    mech_ood_damage(wounded, attacker,
                    damage + (intDamage < 0 ? 0 : intDamage));
    return;
  }

  if (hitloc != -1) {
    ArmorStringFromIndex(hitloc, locationBuff, mech_class(wounded),
                         mech_movement_type(wounded));
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
      mech_damage_inflicted_add(attacker,
                                damage + (intDamage < 0 ? 0 : intDamage));
    mech_damage_taken_add(wounded, damage + (intDamage < 0 ? 0 : intDamage));
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
  if (mech_class(wounded) == CLASS_MW && !was_transfer)
    if (damage > 0)
      if (!(damage = personal_armor_reduce_damage(wounded, cause, hitloc,
                                                  damage, intDamage)))
        return;
  mech_printf(wounded, MECHALL, "[fg=yellow bold]You have been hit %s%s[reset]",
              notificationBuff, was_transfer ? "(transfer)" : "");
  /* Always a good policy :-> */
  if (damage > 0 && intDamage <= 0 && !was_transfer &&
      !mech_condition_summary(wounded).fallen) {
    if (btech_context_stagger_mode(mech_context(wounded)) &&
        mech_class(wounded) == CLASS_MECH) {

      mech_stagger_damage_append(wounded, damage,
                                 btech_context_now(mech_context(wounded)),
                                 mech_dbref(attacker), false);
    } else {
      mech_turn_damage_add(wounded, damage);
    }
  }

  if (hitloc == HEAD && mech_class(wounded) == CLASS_MECH) {

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
      if (!mech_is_landed(wounded) && mech_is_started(wounded)) {
        mech_notify(attacker, MECHALL, "You shoot the craft from the sky!");
        mech_los_broadcast_unit(attacker, wounded, "shoots %s from the sky!");
      }
      mech_destroy(wounded, attacker,
                   !(!mech_is_landed(wounded) && mech_is_started(wounded)),
                   KILL_TYPE_NORMAL);
    }
    return;
  }
  if (damage > 0) {
    if (mech_class(wounded) == CLASS_MECH) {
      if (!isrear && (mech_technology_flags(wounded) & SLITE_TECH) &&
          !mech_condition_summary(wounded).searchlight_destroyed &&
          (hitloc == LTORSO || hitloc == CTORSO || hitloc == RTORSO)) {
        /* Possibly destroy the light */
        if (btech_random_roll(mech_context(wounded)) > 6) {
          if (mech_condition_summary(wounded).searchlight_on ||
              (btech_random_roll(mech_context(wounded)) > 5)) {
            mech_searchlight_destroy(wounded);
            mech_los_broadcast(wounded, "'s searchlight is blown apart!");
            mech_notify(
                wounded, MECHALL,
                "[fg=yellow bold]Your searchlight is destroyed![reset]");
          }
        }
      }
    }
    if (mech_class(wounded) == CLASS_VEH_GROUND) {
      if (!isrear && (mech_technology_flags(wounded) & SLITE_TECH) &&
          !mech_condition_summary(wounded).searchlight_destroyed &&
          (hitloc == FSIDE)) {
        /* Possibly destroy the light */
        if (btech_random_roll(mech_context(wounded)) > 6) {
          if (mech_condition_summary(wounded).searchlight_on ||
              (btech_random_roll(mech_context(wounded)) > 5)) {
            mech_searchlight_destroy(wounded);
            mech_los_broadcast(wounded, "'s searchlight is blown apart!");
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
    if (btech_context_stat_engine_dbref(mech_context(wounded)) > 0 &&
        wWeapIndx != -1)
      notify_checked(
          btech_context_evaluation(mech_context(wounded)),
          btech_context_stat_engine_dbref(mech_context(wounded)), GOD,
          tprintf("STATHIT|#%ld|#%ld|#%ld|%s|%s|#%ld|#%ld|%d|%s%s|%s|%d|%d",
                  mech_map_dbref(attacker), mech_pilot_dbref(attacker),
                  mech_pilot_dbref(wounded), mech_model_reference(attacker),
                  mech_model_reference(wounded), mech_dbref(attacker),
                  mech_dbref(wounded), bth, isrear ? "Rear " : "",
                  hitloc != -1 ? locationBuff : "NONE",
                  &MechWeapons[wWeapIndx].name[0], damage - intDamage,
                  mech_section_internal(wounded, hitloc) < intDamage
                      ? intDamage -
                            (intDamage - mech_section_internal(wounded, hitloc))
                      : intDamage),
          MSG_ME_ALL | MSG_F_DOWN);

    if (intDamage >= 0)
      mech_flood_section(wounded, hitloc, mech_position_z(wounded));
    if (intDamage > 0 && !mech_is_aerospace_unit(wounded)) {
      intDamage =
          cause_internaldamage(wounded, attacker, LOS, attackPilot, isrear,
                               hitloc, intDamage, cause, &crits);
      if (!intDamage && !mech_section_is_destroyed(wounded, hitloc))
        mech_location_breach(attacker, wounded, hitloc);
    } else
      mech_location_maybe_breach(attacker, wounded, hitloc);
    if (intDamage > 0 && transfer && (mech_class(wounded) != CLASS_BSUIT)) {
      if ((hitloc = mech_hit_location_transfer(wounded, hitloc)) >= 0)
        DamageMech(wounded, attacker, LOS, attackPilot, hitloc, isrear,
                   iscritical, intDamage, -2, cause, bth, wWeapIndx, wAmmoMode,
                   tIgnoreSwarmers);
      else {
        mech_destroy(wounded, attacker, 1, KILL_TYPE_NORMAL);
        return;
      }
    }
  } else
  /* Cause _INTERNAL_ HAVOC! :-) */
  /* Non-CASE things get _really_ hurt */
  {
    if (intDamage > 0) {
      if (mech_is_aerospace_unit(wounded))
        intDamage = cause_armordamage(wounded, attacker, LOS, attackPilot,
                                      isrear, iscritical, hitloc, intDamage,
                                      &crits, wWeapIndx, wAmmoMode);
      else
        intDamage =
            cause_internaldamage(wounded, attacker, LOS, attackPilot, isrear,
                                 hitloc, intDamage, cause, &crits);
      if (!mech_section_is_destroyed(wounded, hitloc))
        mech_location_maybe_breach(attacker, wounded, hitloc);
      if (intDamage > 0 && transfer &&
          !(mech_section_configuration_has(wounded, hitloc, CASE_TECH) ||
            (mech_technology_flags(wounded) & CLAN_TECH))) {
        if ((hitloc = mech_hit_location_transfer(wounded, hitloc)) >= 0) {
          if (!mech_is_aerospace_unit(wounded))
            DamageMech(wounded, attacker, LOS, attackPilot, hitloc, isrear,
                       iscritical, -2, intDamage, cause, bth, wWeapIndx,
                       wAmmoMode, tIgnoreSwarmers);
          else
            DamageMech(wounded, attacker, LOS, attackPilot, hitloc, isrear,
                       iscritical, intDamage, -2, cause, bth, wWeapIndx,
                       wAmmoMode, tIgnoreSwarmers);
        } else {
          mech_destroy(wounded, attacker, 1, KILL_TYPE_NORMAL);
          return;
        }
      }
    }
  }

  /* Check to see if the tow lines should snap */
  if (tSnapTowLines && (mech_carried_dbref(wounded) > 0)) {
    if ((towTarget = btech_context_get_mech(mech_context(wounded),
                                            mech_carried_dbref(wounded)))) {
      mech_notify(wounded, MECHALL, "The hit causes your tow line to let go!");
      mech_notify(towTarget, MECHALL, "Your tow lines go suddenly slack!");
      mech_los_broadcast(wounded,
                         "'s tow lines release and flap freely behind it!");

      mech_dropoff(GOD, wounded, "");
    }
  }

  /* For now, only check IS PlasmaRifles. Can use this for Clan PlasmaCannon
   * later */
  if (wWeapIndx > 0) {
    if (strstr(MechWeapons[wWeapIndx].name, "IS.PlasmaRifle")) {
      if (mech_class(wounded) == CLASS_MECH)
        mech_plasma_hit(attacker, wounded, LOS);
    }
  }
  /* Check to see if we blow up ammo that's dumping. */
  if (tBlowDumpingAmmo) {
    mech_ammunition_dump_explode(wounded, attacker, hitloc);
  }
}

/* this takes care of setting all the criticals to CRIT_DESTROYED */
