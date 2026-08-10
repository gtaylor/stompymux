/* Implements BattleTech combat mechanics for battle armor. */

#include <string.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "crit_api.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_bth_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"

/*! \todo {The Bsuit code needs an overhaul} */

/* 2 battlesuit-specific attacks:
   - attackleg
   - swarm
 */

/* Stops everyone who's swarming this poor guy */

constexpr int RECYCLE_SWARM = PHYSICAL_RECYCLE_TIME / 3;
constexpr int RECYCLE_ATTACKLEG = PHYSICAL_RECYCLE_TIME / 2;

void bsuit_swarm(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  Mech *target;
  int baseToHit = 4;
  int tIsMount = 0;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (buffer != nullptr)
    buffer = checked_storage_at(buffer, strlen(buffer) + 1, sizeof(char),
                                strspn(buffer, " \t\r\n\f\v"));
  if (!buffer) {
    static char empty_buffer[] = "";
    buffer = empty_buffer;
  }

  /* Stop swarming... */
  if (!strcmp(buffer, "-")) {
    if (mech_swarm_target(mech) > 0) {
      bsuit_swarm_stop(mech, 1);
      return;
    }
  }

  if (bsuit_action_validate(mech, player))
    return;

  if (bsuit_target_find(player, mech, &target, buffer))
    return;

  /* See if we're 'swarming' or 'mounting' */
  if (mech_team(target) == mech_team(mech)) {
    /* Make sure this type of bsuit has the ability to mount */
    if (!(mech_infantry_technology_flags(mech) & INF_MOUNT_TECH)) {
      mech_notify(mech, MECHALL,
                  "These battlesuits are not capable of mounting mechs!");
      return;
    }

    tIsMount = 1;
  } else {
    if (bsuit_jettison_validate(mech))
      return;

    /* Make sure this type of bsuit has the ability to swarm */
    if (!(mech_infantry_technology_flags(mech) & INF_SWARM_TECH)) {
      mech_notify(
          mech, MECHALL,
          "These battlesuits are not capable of performing swarm attacks!");
      return;
    }
  }

  /* Make sure there are no suits already on us */
  if (bsuit_swarmer_count(mech) > 0) {
    mech_notify(mech, MECHALL,
                "That target already have battlesuits crawling all over it! "
                "There's no room for you!");

    return;
  }

  /* get our BTH... we make it easier for mounting */
  switch (bsuit_member_count(mech)) {
  case 1:
  case 2:
  case 3:
    baseToHit = 5;
    break;

  default:
    baseToHit = 2;
    break;
  }

  if (tIsMount)
    baseToHit -= 4;

  if (mech_condition_summary(mech).hidden) {
    if (mech_is_immobile(target))
      baseToHit -= 4;

    if (mech_condition_summary(target).fallen)
      baseToHit -= 4;
  } else {
    baseToHit += mech_target_movement_modifier(mech, target, 0.0);

    if (mech_condition_summary(target).fallen)
      baseToHit -= 2;
  }

  /* Well, we're here. Let's see if it works. */
  if (MadePilotSkillRoll(mech, baseToHit)) {
    mech_printf(target, MECHALL, "%s %s you!",
                mech_to_mech_display_id(target, mech).text,
                (tIsMount ? "mounts" : "swarms"));
    mech_printf(mech, MECHALL, "You %s %s!", (tIsMount ? "mount" : "swarm"),
                mech_to_mech_display_id(mech, target).text);

    mech_swarm_target_set(mech, mech_dbref(target));
    mech_swarmed_by_set(target, mech_dbref(mech));
    mech_mounting_set(mech, true);
    mech_mounted_set(target, true);

    if (tIsMount) {
      mech_los_broadcast_unit(mech, target, "mounts %s!");
    } else {
      mech_los_broadcast_unit(mech, target, "swarms %s!");
    }

    mech_current_speed_set(mech, 0.0F);
    mech_desired_speed_set(mech, 0.0F);
    mech_heading_set(mech, 270);
    mech_desired_heading_set(mech, 270);
    mech_position_mirror(mech, target, 1);
    MarkForLOSUpdate(mech);
    mech_flood(mech);
    mech_stop_lock(mech);
  } else {
    mech_printf(target, MECHALL, "%s attempts to %s you!",
                mech_to_mech_display_id(target, mech).text,
                (tIsMount ? "mount" : "swarm"));
    mech_printf(mech, MECHALL,
                "Nice try, but you don't succeed in your attempt at %s %s!",
                (tIsMount ? "mounting" : "swarming"),
                mech_to_mech_display_id(mech, target).text);
  }

  if (mech_condition_summary(mech).hidden) {
    mech_notify(mech, MECHALL, "You move too much and break your cover!");
    mech_los_broadcast(mech, "breaks from its cover.");
    mech_hidden_set(mech, false);
    mech_event_cancel(mech, EVENT_HIDE);
  }

  bsuit_recycle_start(mech, RECYCLE_SWARM);
}

void bsuit_attackleg(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  Mech *target;
  int baseToHit = 0;
  int wLegTemp = -1;
  int wLegID = -1;
  int wCritRoll = 0;
  char strAttackLoc[50];

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!(mech_infantry_technology_flags(mech) & INF_ANTILEG_TECH)) {
    mech_notify(mech, MECHALL,
                "These battlesuits are not capable of performing leg attacks!");
    return;
  }

  if (bsuit_action_validate(mech, player))
    return;

  if (bsuit_jettison_validate(mech))
    return;

  if (bsuit_target_find(player, mech, &target, buffer))
    return;

  if (IsMechLegLess(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That mech has no legs to grab!");
    return;
  }
  if (mech_team(mech) == mech_team(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't attack the leg of a friendly mech!");
    return;
  }

  switch (bsuit_member_count(mech)) {
  case 1:
    baseToHit = 7;
    break;

  case 2:
    baseToHit = 5;
    break;

  case 3:
    baseToHit = 2;
    break;

  default:
    baseToHit = -1;
    break;
  }

  if (mech_condition_summary(mech).hidden) {
    if (mech_is_immobile(target))
      baseToHit -= 4;

    if (mech_condition_summary(target).fallen)
      baseToHit -= 2;
  } else {
    baseToHit += mech_target_movement_modifier(mech, target, 0.0);
  }

  if (mech_is_quad(target)) {
    do {
      switch (btech_random_range(mech_context(mech), 0, 3)) {
      case 0:
        wLegTemp = RLEG;
        break;

      case 1:
        wLegTemp = LLEG;
        break;

      case 2:
        wLegTemp = RARM;
        break;

      case 3:
        wLegTemp = LARM;
        break;
      }

      if (mech_section_internal(target, wLegTemp))
        wLegID = wLegTemp;

    } while (wLegID == -1);
  } else {
    wLegTemp = (btech_random_range(mech_context(mech), 0, 1)) ? RLEG : LLEG;

    if (mech_section_internal(target, wLegTemp) == 0) {
      wLegID = (wLegTemp == RLEG) ? LLEG : RLEG;
    } else {
      wLegID = wLegTemp;
    }
  }

  ArmorStringFromIndex(wLegID, strAttackLoc, mech_class(target),
                       mech_movement_type(target));

  mech_printf(mech, MECHALL,
              "You go for %s's %s, placing explosives in the joints!",
              mech_to_mech_display_id(mech, target).text, strAttackLoc);

  if (MadePilotSkillRoll(mech, baseToHit)) {
    mech_printf(
        target, MECHALL,
        "%s swarms your %s putting small packets of explosives all over it!",
        mech_to_mech_display_id(target, mech).text, strAttackLoc);

    mech_los_broadcast_unit(mech, target, "attacks %s's legs!");

    /* find out if we do a crit or damage */
    wCritRoll = btech_random_roll(mech_context(mech));

    if (wCritRoll >= 8) {
      mech_printf(target, MECHALL,
                  "The explosives manage to rip into the internals of your %s!",
                  strAttackLoc);

      switch (wCritRoll) {
      case 8:
      case 9:
        mech_critical_handle(&(CriticalHitDispatch){.wounded = target,
                                                    .attacker = mech,
                                                    .line_of_sight = 1,
                                                    .section = wLegID,
                                                    .count = 1});
        break;
      case 10:
      case 11:
        mech_critical_handle(&(CriticalHitDispatch){.wounded = target,
                                                    .attacker = mech,
                                                    .line_of_sight = 1,
                                                    .section = wLegID,
                                                    .count = 2});
        break;
      case 12:
        switch (wLegID) {
        case RARM:
        case LARM:
        case RLEG:
        case LLEG:
          /* Limb blown off */
          mech_notify(target, MECHALL, "[fg=yellow bold]CRITICAL HIT!![reset]");

          mech_los_broadcast(
              target,
              tprintf("'s %s is blown off in a shower of sparks and smoke!",
                      strAttackLoc));
          mech_section_destroy(&(SectionDestructionRequest){.wounded = target,
                                                            .attacker = mech,
                                                            .line_of_sight = 1,
                                                            .section = wLegID});
          [[fallthrough]];
        default:
          mech_critical_handle(&(CriticalHitDispatch){.wounded = target,
                                                      .attacker = mech,
                                                      .line_of_sight = 1,
                                                      .section = wLegID,
                                                      .count = 3});
        }
        break;
      default:
        break;
      }
    } else {
      mech_printf(target, MECHALL,
                  "The explosives explode on the surface of your %s!",
                  strAttackLoc);
      mech_damage_apply(
          &(MechDamageRequest){.target = target,
                               .attacker = mech,
                               .line_of_sight = 1,
                               .attack_pilot = mech_pilot_dbref(mech),
                               .hit_location = wLegID,
                               .rear = 0,
                               .critical = 1,
                               .armor_damage = 4,
                               .internal_damage = 0,
                               .transfer = MECH_DAMAGE_NORMAL,
                               .cause = -1,
                               .base_to_hit = 0,
                               .weapon_index = -1,
                               .ammunition_mode = 0,
                               .ignore_swarmers = 1});
    }
  } else {
    mech_printf(target, MECHALL,
                "%s attempts to attacks your legs, but misses miserably.",
                mech_to_mech_display_id(target, mech).text);

    mech_printf(mech, MECHALL,
                "You realize that this is harder than it looks and fail in "
                "your attempt at hitting %s's legs!",
                mech_to_mech_display_id(mech, target).text);

    mech_los_broadcast_unit(
        mech, target, "attempts to climb %s's legs, but fails miserably!");
  }

  bsuit_recycle_start(mech, RECYCLE_ATTACKLEG);
}

void bsuit_pack_jettison(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  int wcJettisoned = 0;
  int wcSuits = 0;
  int i, j;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if ((!(mech_infantry_technology_flags(mech) & CAN_JETTISON_TECH))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You have no backpack that is capable of being jettisoned!");
    return;
  }

  for (i = 0; i < NUM_BSUIT_MEMBERS; i++) {
    for (j = 0; j < NUM_CRITICALS; j++) {
      if ((mech_critical_fire_mode(mech, i, j) & WILL_JETTISON_MODE) &&
          (!(mech_critical_fire_mode(mech, i, j) & IS_JETTISONED_MODE))) {
        mech_critical_jettison(mech, i, j);

        wcJettisoned++;
      }
    }
  }

  if (wcJettisoned > 0) {
    wcSuits = bsuit_member_count(mech);

    if (wcSuits > 1) {
      mech_notify(mech, MECHALL,
                  "The explosive bolts that hold the backpacks on blow, "
                  "allowing them to drop to the ground.");
      mech_los_broadcast(
          mech, "'s backpacks blow off in a shower of small explosions!");
    } else {
      mech_notify(mech, MECHALL,
                  "The explosive bolts that hold your backpack on blows, "
                  "allowing it to drop to the ground.");
      mech_los_broadcast(mech,
                         "'s backpack blows off in a puff of grey smoke!");
    }
  } else {
    mech_notify(
        mech, MECHALL,
        "You realize you have nothing to jettison. Maybe you already did it?");
  }
}
