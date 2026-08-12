#include "equipment_types.h"
#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/* Implements BattleTech combat mechanics for unit damage. */

#include <stdio.h>
#include <string.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "environment_damage_api.h"
#include "map.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_ammodump_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_damage_history_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_ood_api.h"
#include "mech_pickup_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_stagger.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "pcombat_api.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
void mech_damage_apply(const MechDamageRequest *request) {
  Mech *wounded = request->target;
  Mech *attacker = request->attacker;
  const bool LOS = request->line_of_sight;
  const DbRef ATTACK_PILOT = request->attack_pilot;
  int hitloc = request->hit_location;
  int isrear = request->rear;
  int iscritical = request->critical;
  int damage = request->armor_damage;
  int int_damage = request->internal_damage;
  const int CAUSE = request->cause;
  const int BTH = request->base_to_hit;
  const int W_WEAP_INDX = request->weapon_index;
  const int W_AMMO_MODE = request->ammunition_mode;
  const bool T_IGNORE_SWARMERS = request->ignore_swarmers;
  char location_buff[20];
  char notification_buff[80];
  char rear_message[10];
  int transfer = request->transfer != MECH_DAMAGE_NORMAL;
  int was_transfer = request->transfer == MECH_DAMAGE_TRANSFER_CONTINUATION;
  int kill = 0;
  BattleMap *map;
  int crits = 0;
  int t_blow_dumping_ammo = 0;
  int w_swarmer_hit_chance = 0;
  int w_roll = btech_random_roll(mech_context(wounded));
  Mech *mech_swarmer;
  int t_snap_tow_lines = 0;
  Mech *tow_target;

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
  if ((bsuit_swarmer_count(wounded) > 0) && (!T_IGNORE_SWARMERS)) {
    mech_swarmer = bsuit_swarmer_find(wounded);
    if (mech_swarmer) {
      if (!attacker || (mech_dbref(attacker) != mech_dbref(mech_swarmer))) {
        w_swarmer_hit_chance = 20 * bsuit_member_count(mech_swarmer);
        if (isrear) {
          if ((hitloc != CTORSO) && (hitloc != RTORSO) && (hitloc != LTORSO))
            w_swarmer_hit_chance = 0;
        } else {
          if ((hitloc != RTORSO) && (hitloc != LTORSO))
            w_swarmer_hit_chance = 0;
        }

        if ((w_swarmer_hit_chance >= w_roll) &&
            mech_section_armor(wounded, hitloc)) {
          if (attacker && (mech_dbref(attacker) != mech_dbref(wounded))) {
            mech_notify(attacker, MECHALL,
                        "The battlesuits crawling all over your target absorb "
                        "the damage!");
          }

          mech_notify(
              wounded, MECHALL,
              "The battlesuits crawling all over you absorb the damage!");
          mech_notify(mech_swarmer, MECHALL, "You absorb the damage!");
          hitloc = mech_hit_location(mech_swarmer, 0, &iscritical, &isrear);
          mech_damage_apply(&(MechDamageRequest){
              .target = mech_swarmer,
              .attacker = attacker,
              .line_of_sight = LOS,
              .attack_pilot = ATTACK_PILOT,
              .hit_location = hitloc,
              .armor_damage = damage,
              .cause = CAUSE,
              .base_to_hit = BTH,
              .weapon_index = W_WEAP_INDX,
              .ammunition_mode = W_AMMO_MODE,
          });
          return;
        }
      }
    }
  }

  if (mech_class(wounded) == CLASS_MW || mech_class(wounded) == CLASS_MECH)
    transfer = 1;
#ifdef BT_MOVEMENT_MODES
  if ((damage > 0 || int_damage > 0) &&
      mech_condition_summary(wounded).sprinting) {
    mech_sprinting_set(wounded, false);
    mech_los_broadcast(wounded, "breaks out of its sprint as it takes damage!");
    mech_notify(wounded, MECHALL,
                "You lose your sprinting momentum as you take damage!");
    if (!mech_event_count(wounded, EVENT_MOVEMODE))
      mech_event_schedule(wounded, EVENT_MOVEMODE, mech_movemode_event, TURN,
                          MODE_OFF | MODE_SPRINT);
  }

  if ((damage > 0 || int_damage > 0) &&
      mech_condition_summary(wounded).hidden) {
    mech_hidden_set(wounded, false);
    mech_los_broadcast(wounded, "loses its cover as it takes damage!");
    mech_notify(wounded, MECHALL, "Your cover is ruined as you take damage!");
    if (!mech_event_count(wounded, EVENT_MOVEMODE))
      mech_hidden_set(wounded, false);
  }

  if ((damage > 0 || int_damage > 0) &&
      (mech_move_mode_locked(wounded) &&
       !(mech_event_first_delay(wounded, EVENT_MOVEMODE) &
         (MODE_EVADE | MODE_DODGE | MODE_OFF)))) {
    mech_event_cancel(wounded, EVENT_MOVEMODE);
    mech_notify(wounded, MECHALL,
                "Your movement mode changes are cancelled as you take damage!");
  }
#endif
  if (damage > 0 && int_damage == 0) {
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
      gunnery_experience_award(&(GunneryExperienceAward){
          .pilot = ATTACK_PILOT,
          .attacker = attacker,
          .target = wounded,
          .damage = damage,
          .multiplier = 1.0,
          .weapon_index = CAUSE,
          .base_to_hit = BTH,
      });
    else if (btech_context_damage_experience_mode(mech_context(wounded)) ==
             BTECH_DAMAGE_XP_PILOTING)
      if (!mech_is_destroyed(wounded) &&
          is_in_character(btech_context_database(mech_context(wounded)),
                          mech_dbref(wounded)) &&
          mech_team(wounded) != mech_team(attacker))
        if (mech_class(wounded) != CLASS_MW || mech_class(attacker) == CLASS_MW)
          piloting_experience_award(&(PilotingExperienceAward){
              .pilot = ATTACK_PILOT,
              .mech = attacker,
              .reason = damage / 3,
              .unconditional = true,
          });
    damage = unit_damage_to_personal_combat(&(PersonalCombatDamageConversion){
        .target = wounded,
        .weapon_index = CAUSE,
        .damage = damage,
    });
  }
  if (isrear) {
    if (!(mech_technology_flags(wounded) & SALVAGE_TECH) &&
        (btech_random_roll(mech_context(wounded)) <= 5) &&
        (hitloc == CTORSO || hitloc == LTORSO || hitloc == RTORSO))
      t_snap_tow_lines = 1;

    if (mech_class(wounded) == CLASS_MECH) {
      strcpy(rear_message, "(Rear)");
      if (mech_event_count(wounded, EVENT_DUMP) &&
          ((hitloc == CTORSO) || (hitloc == LTORSO) || (hitloc == RTORSO)) &&
          (CAUSE >= 0))
        t_blow_dumping_ammo = 1;
    } else {
      if (hitloc == FSIDE)
        hitloc = BSIDE;
      *rear_message = '\0';
      isrear = 0;
    }
  } else {
    *rear_message = '\0';
  }
  /*   while (mech_section_is_destroyed(wounded, hitloc) && !kill) */
  while (((!mech_is_aerospace_unit(wounded) &&
           !mech_section_internal(wounded, hitloc)) ||
          (mech_is_aerospace_unit(wounded) &&
           !mech_section_armor(wounded, hitloc))) &&
         !kill) {
    bool transfer_succeeded = false;
    if (transfer) {
      hitloc = mech_hit_location_transfer(wounded, hitloc);
      transfer_succeeded = hitloc >= 0 && (mech_class(wounded) == CLASS_MECH ||
                                           mech_class(wounded) == CLASS_MW ||
                                           mech_class(wounded) == CLASS_BSUIT ||
                                           mech_is_aerospace_unit(wounded));
    }
    if (transfer_succeeded) {
      mech_damage_apply(&(MechDamageRequest){
          .target = wounded,
          .attacker = attacker,
          .line_of_sight = LOS,
          .attack_pilot = ATTACK_PILOT,
          .hit_location = hitloc,
          .rear = isrear != 0,
          .critical = iscritical != 0,
          .armor_damage = damage,
          .transfer = MECH_DAMAGE_TRANSFER_CONTINUATION,
          .cause = CAUSE,
          .base_to_hit = BTH,
          .weapon_index = W_WEAP_INDX,
          .ammunition_mode = W_AMMO_MODE,
          .ignore_swarmers = T_IGNORE_SWARMERS,
      });
      return;
    }
    bool secondary_transfer_succeeded = false;
    if (mech_class(wounded) == CLASS_MECH || mech_class(wounded) == CLASS_MW ||
        mech_class(wounded) == CLASS_BSUIT || mech_is_aerospace_unit(wounded)) {
      hitloc = mech_hit_location_transfer(wounded, hitloc);
      secondary_transfer_succeeded = hitloc >= 0;
    }
    if (!secondary_transfer_succeeded) {
      if (mech_is_aerospace_unit(wounded) && !mech_is_destroyed(wounded)) {
        /* Hurt SI instead. */
        if (mech_structural_integrity(wounded) <= damage) {
          kill = 1;
        } else {
          mech_structural_integrity_set(
              wounded, mech_structural_integrity(wounded) - damage);
          kill = -1;
        }
      } else {
        return;
      }
    }
    /* Nyah. Damage transferred to waste, shooting a dead mech? */
  }
  if (mech_cocoon_integrity(wounded) > 0 &&
      btech_random_roll(mech_context(wounded)) > 8) {
    mech_ood_damage(wounded, attacker, damage + int_damage);
    return;
  }

  if (hitloc != -1) {
    armor_string_from_index(hitloc, location_buff, mech_class(wounded),
                            mech_movement_type(wounded));
    (void)snprintf(notification_buff, sizeof(notification_buff),
                   "for %d points of damage in the %s %s", damage + int_damage,
                   location_buff, rear_message);
  } else {
    (void)snprintf(notification_buff, sizeof(notification_buff),
                   "for %d points of damage in the structure.",
                   damage + int_damage);
  }

  /* Only count initial damage. Transfer is just gonna do that, transfer, not
   * damage again */
  if (!was_transfer) {
    if (attacker != wounded)
      mech_damage_inflicted_add(attacker, damage + int_damage);
    mech_damage_taken_add(wounded, damage + int_damage);
  }

  /*  if (LOS && attack_pilot != -1) */
  if (LOS) {
    if (!was_transfer)
      mech_printf(attacker, MECHALL, "[fg=green]You hit %s[reset]",
                  notification_buff);
    else
      mech_printf(attacker, MECHALL, "[fg=green]Damage transfer.. %s[reset]",
                  notification_buff);
  }
  if (mech_class(wounded) == CLASS_MW && !was_transfer) {
    if (damage > 0) {
      damage = personal_armor_reduce_damage(&(PersonalArmorDamageRequest){
          .wounded = wounded,
          .cause = CAUSE,
          .hit_location = hitloc,
          .damage = damage,
          .damage_identifier = int_damage,
      });
      if (!damage)
        return;
    }
  }
  mech_printf(wounded, MECHALL, "[fg=yellow bold]You have been hit %s%s[reset]",
              notification_buff, was_transfer ? "(transfer)" : "");
  /* Always a good policy :-> */
  if (damage > 0 && int_damage <= 0 && !was_transfer &&
      !mech_condition_summary(wounded).fallen) {
    if (btech_context_stagger_mode(mech_context(wounded)) &&
        mech_class(wounded) == CLASS_MECH) {

      mech_stagger_damage_append(&(StaggerDamageApplication){
          .mech = wounded,
          .amount = damage,
          .occurred_at = btech_context_now(mech_context(wounded)),
          .attacker = mech_dbref(attacker)});
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
    int_damage += cause_armordamage(
        &(ArmorDamageRequest){.wounded = wounded,
                              .attacker = attacker,
                              .line_of_sight = LOS,
                              .rear = isrear != 0,
                              .critical = iscritical != 0,
                              .section = hitloc,
                              .damage = damage,
                              .critical_hits = &crits,
                              .weapon_index = W_WEAP_INDX,
                              .ammunition_mode = W_AMMO_MODE});
    /* for Stat Engine */
    /* STATHIT|MAP|ATTACKER PILOT DBREF|WOUNDED PILOT DBREF|ATTACKER
     * MECHREF|WOUNDED MECHREF|ATTACKER MECH DBREF|WOUNDED MECH DBREF|BTH OF
     * SHOT|HITLOC|WEAPON NAME|Armor Damage|Internal Damage */
    /* The last part in the function is how we're handling transfer damage.
     * We're going to check how much internal is left, do some math, and only
     * count the applied damage */
    /* As the transfer damage will come back and send to another section via
     * mech_damage_apply calls */
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
        W_WEAP_INDX != -1)
      notify_checked(
          btech_context_evaluation(mech_context(wounded)),
          btech_context_stat_engine_dbref(mech_context(wounded)), GOD,
          tprintf("STATHIT|#%ld|#%ld|#%ld|%s|%s|#%ld|#%ld|%d|%s%s|%s|%d|%d",
                  mech_map_dbref(attacker), mech_pilot_dbref(attacker),
                  mech_pilot_dbref(wounded), mech_model_reference(attacker),
                  mech_model_reference(wounded), mech_dbref(attacker),
                  mech_dbref(wounded), BTH, isrear ? "Rear " : "",
                  hitloc != -1 ? location_buff : "NONE",
                  weapon_catalogue_name(W_WEAP_INDX), damage - int_damage,
                  mech_section_internal(wounded, hitloc) < int_damage
                      ? int_damage - (int_damage -
                                      mech_section_internal(wounded, hitloc))
                      : int_damage),
          MSG_ME_ALL | MSG_F_DOWN);

    if (int_damage >= 0)
      mech_flood_section(wounded, hitloc, mech_position_z(wounded));
    if (int_damage > 0 && !mech_is_aerospace_unit(wounded)) {
      int_damage = cause_internaldamage(
          &(InternalDamageRequest){.wounded = wounded,
                                   .attacker = attacker,
                                   .line_of_sight = LOS,
                                   .section = hitloc,
                                   .damage = int_damage,
                                   .critical_hits = &crits});
      if (!int_damage && !mech_section_is_destroyed(wounded, hitloc))
        mech_location_breach(attacker, wounded, hitloc);
    } else {
      mech_location_maybe_breach(attacker, wounded, hitloc);
    }
    if (int_damage > 0 && transfer && (mech_class(wounded) != CLASS_BSUIT)) {
      hitloc = mech_hit_location_transfer(wounded, hitloc);
      if (hitloc >= 0) {
        mech_damage_apply(&(MechDamageRequest){
            .target = wounded,
            .attacker = attacker,
            .line_of_sight = LOS,
            .attack_pilot = ATTACK_PILOT,
            .hit_location = hitloc,
            .rear = isrear != 0,
            .critical = iscritical != 0,
            .armor_damage = int_damage,
            .transfer = MECH_DAMAGE_TRANSFER_CONTINUATION,
            .cause = CAUSE,
            .base_to_hit = BTH,
            .weapon_index = W_WEAP_INDX,
            .ammunition_mode = W_AMMO_MODE,
            .ignore_swarmers = T_IGNORE_SWARMERS,
        });
      } else {
        mech_destroy(wounded, attacker, 1, KILL_TYPE_NORMAL);
        return;
      }
    }
  } else
  /* Cause _INTERNAL_ HAVOC! :-) */
  /* Non-CASE things get _really_ hurt */
  {
    if (int_damage > 0) {
      if (mech_is_aerospace_unit(wounded))
        int_damage = cause_armordamage(
            &(ArmorDamageRequest){.wounded = wounded,
                                  .attacker = attacker,
                                  .line_of_sight = LOS,
                                  .rear = isrear != 0,
                                  .critical = iscritical != 0,
                                  .section = hitloc,
                                  .damage = int_damage,
                                  .critical_hits = &crits,
                                  .weapon_index = W_WEAP_INDX,
                                  .ammunition_mode = W_AMMO_MODE});
      else
        int_damage = cause_internaldamage(
            &(InternalDamageRequest){.wounded = wounded,
                                     .attacker = attacker,
                                     .line_of_sight = LOS,
                                     .section = hitloc,
                                     .damage = int_damage,
                                     .critical_hits = &crits});
      if (!mech_section_is_destroyed(wounded, hitloc))
        mech_location_maybe_breach(attacker, wounded, hitloc);
      if (int_damage > 0 && transfer &&
          !(mech_section_configuration_has(wounded, hitloc, CASE_TECH) ||
            (mech_technology_flags(wounded) & CLAN_TECH))) {
        hitloc = mech_hit_location_transfer(wounded, hitloc);
        if (hitloc >= 0) {
          if (!mech_is_aerospace_unit(wounded))
            mech_damage_apply(&(MechDamageRequest){
                .target = wounded,
                .attacker = attacker,
                .line_of_sight = LOS,
                .attack_pilot = ATTACK_PILOT,
                .hit_location = hitloc,
                .rear = isrear != 0,
                .critical = iscritical != 0,
                .internal_damage = int_damage,
                .transfer = MECH_DAMAGE_TRANSFER_CONTINUATION,
                .cause = CAUSE,
                .base_to_hit = BTH,
                .weapon_index = W_WEAP_INDX,
                .ammunition_mode = W_AMMO_MODE,
                .ignore_swarmers = T_IGNORE_SWARMERS,
            });
          else
            mech_damage_apply(&(MechDamageRequest){
                .target = wounded,
                .attacker = attacker,
                .line_of_sight = LOS,
                .attack_pilot = ATTACK_PILOT,
                .hit_location = hitloc,
                .rear = isrear != 0,
                .critical = iscritical != 0,
                .armor_damage = int_damage,
                .transfer = MECH_DAMAGE_TRANSFER_CONTINUATION,
                .cause = CAUSE,
                .base_to_hit = BTH,
                .weapon_index = W_WEAP_INDX,
                .ammunition_mode = W_AMMO_MODE,
                .ignore_swarmers = T_IGNORE_SWARMERS,
            });
        } else {
          mech_destroy(wounded, attacker, 1, KILL_TYPE_NORMAL);
          return;
        }
      }
    }
  }

  /* Check to see if the tow lines should snap */
  if (t_snap_tow_lines && (mech_carried_dbref(wounded) > 0)) {
    tow_target = btech_context_get_mech(mech_context(wounded),
                                        mech_carried_dbref(wounded));
    if (tow_target) {
      mech_notify(wounded, MECHALL, "The hit causes your tow line to let go!");
      mech_notify(tow_target, MECHALL, "Your tow lines go suddenly slack!");
      mech_los_broadcast(wounded,
                         "'s tow lines release and flap freely behind it!");

      mech_dropoff(GOD, wounded, "");
    }
  }

  /* For now, only check IS PlasmaRifles. Can use this for Clan PlasmaCannon
   * later */
  if (W_WEAP_INDX > 0) {
    if (strstr(weapon_catalogue_name(W_WEAP_INDX), "IS.PlasmaRifle")) {
      if (mech_class(wounded) == CLASS_MECH)
        mech_plasma_hit(wounded);
    }
  }
  /* Check to see if we blow up ammo that's dumping. */
  if (t_blow_dumping_ammo) {
    mech_ammunition_dump_explode(wounded, attacker, hitloc);
  }
}

/* this takes care of setting all the criticals to CRIT_DESTROYED */
