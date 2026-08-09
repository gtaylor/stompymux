/* Implements BattleTech combat mechanics for battle armor. */

#include <math.h>
#include <string.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
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
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
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
constexpr int RECYCLE_INT_STOPSWARM = PHYSICAL_RECYCLE_TIME / 3;
constexpr int RECYCLE_UNINT_STOPSWARM = PHYSICAL_RECYCLE_TIME / 2;
constexpr int RECYCLE_FALL_STOPSWARM = (PHYSICAL_RECYCLE_TIME / 4) * 3;

const char *bsuit_formation_name(const Mech *mech) {
  return (mech_technology_flags(mech) & CLAN_TECH) ? "Point" : "Squad";
}

const char *bsuit_formation_name_lowercase(const Mech *mech) {
  return (mech_technology_flags(mech) & CLAN_TECH) ? "point" : "squad";
}

void bsuit_recycle_start(Mech *mech, int time) {
  int i;

  for (i = 0; i < NUM_BSUIT_MEMBERS; i++)
    if (mech_section_internal(mech, i))
      mech_set_recycle_limb(mech, i, time);
}

void bsuit_swarm_stop(Mech *mech, int intentional) {
  Mech *target =
      btech_context_get_mech(mech_context(mech), mech_swarm_target(mech));

  if (!target || mech_swarm_target(mech) <= 0)
    return;

  mech_swarm_target_set(mech, -1);
  mech_swarmed_by_set(target, -1);
  mech_mounting_set(mech, false);
  mech_mounted_set(target, false);

  if (intentional > 0) {
    mech_notify(mech, MECHALL,
                "You let your hold loosen and you drop from the 'mech!");
    mech_printf(target, MECHALL, "%s lets go of you!",
                mech_to_mech_display_id(target, mech).text);
    mech_los_broadcast_unit(mech, target, "lets go of %s!");

    bsuit_recycle_start(mech, RECYCLE_INT_STOPSWARM);
  } else {
    if (MadePilotSkillRoll(mech, 4)) {
      mech_notify(mech, MECHALL,
                  "The hold loosens and you drop from the 'mech!");
      mech_los_broadcast_unit(mech, target, "jumps off of %s!");
      mech_printf(target, MECHALL, "%s jumps off!",
                  mech_to_mech_display_id(target, mech).text);

      bsuit_recycle_start(mech, RECYCLE_UNINT_STOPSWARM);
    } else {
      mech_notify(mech, MECHALL,
                  "You're suprised by the sudden action and find yourself "
                  "rapidly approaching the ground!");
      mech_los_broadcast_unit(mech, target, "falls off %s!");
      mech_printf(target, MECHALL, "%s falls off!",
                  mech_to_mech_display_id(target, mech).text);

      DamageMech(
          mech, mech, 1, -1,
          btech_random_range_int(mech_context(mech), 0, NUM_BSUIT_MEMBERS - 1),
          0, 0, 11, 0, -1, 0, -1, 0, 1);

      bsuit_recycle_start(mech, RECYCLE_FALL_STOPSWARM);
    }
  }

  mech_current_speed_set(mech, 0);
  mech_maybe_move(mech);
  mech_drop_surface_set(mech, false);
  mech_flood(mech);
}

int bsuit_has_enemy_swarmers(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int count = 0, i;
  DbRef j;
  Mech *t;

  if (!map)
    return 0;

  for (i = 0; i < battle_map_unit_count(map); i++)
    if ((j = battle_map_unit_dbref(map, i)) > 0 && i != mech_map_slot(mech)) {
      if (!(t = btech_context_get_mech(mech_context(mech), j)))
        continue;

      if (mech_swarm_target(t) != mech_dbref(mech))
        continue;

      if (mech_team(mech) == mech_team(t))
        continue;

      count++;
      break;
    }
  return count > 0;
}

int bsuit_has_friendly_riders(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int count = 0, i;
  DbRef j;
  Mech *t;

  if (!map)
    return 0;

  for (i = 0; i < battle_map_unit_count(map); i++)
    if ((j = battle_map_unit_dbref(map, i)) > 0 && i != mech_map_slot(mech)) {
      if (!(t = btech_context_get_mech(mech_context(mech), j)))
        continue;

      if (mech_swarm_target(t) != mech_dbref(mech))
        continue;

      if (mech_team(mech) != mech_team(t))
        continue;

      count++;
      break;
    }
  return count > 0;
}

int bsuit_swarmer_count(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int count = 0, i;
  DbRef j;
  Mech *t;

  if (!map)
    return 0;
  for (i = 0; i < battle_map_unit_count(map); i++)
    if ((j = battle_map_unit_dbref(map, i)) > 0 && i != mech_map_slot(mech)) {
      if (!(t = btech_context_get_mech(mech_context(mech), j)))
        continue;
      if (mech_swarm_target(t) != mech_dbref(mech))
        continue;
      count++;
    }
  return count;
}

Mech *bsuit_swarmer_find(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int i;
  DbRef j;
  Mech *t;

  if (!map)
    return 0;

  for (i = 0; i < battle_map_unit_count(map); i++)
    if ((j = battle_map_unit_dbref(map, i)) > 0 && i != mech_map_slot(mech)) {
      if (!(t = btech_context_get_mech(mech_context(mech), j)))
        continue;

      if (mech_swarm_target(t) == mech_dbref(mech)) {
        return t;
      }
    }

  return nullptr;
}

void bsuit_swarmers_stop(BattleMap *map, Mech *mech, int intentional) {
  int i;
  DbRef j;
  Mech *t;

  if (!map || !mech)
    return;
  for (i = 0; i < battle_map_unit_count(map); i++)
    if ((j = battle_map_unit_dbref(map, i)) > 0 && i != mech_map_slot(mech)) {
      if (!(t = btech_context_get_mech(mech_context(mech), j)))
        continue;
      if (mech_swarm_target(t) != mech_dbref(mech))
        continue;
      bsuit_swarm_stop(t, intentional);
    }
}

void bsuit_swarmers_position_update(BattleMap *map, Mech *mech) {
  int i;
  DbRef j;
  Mech *t;

  for (i = 0; i < battle_map_unit_count(map); i++)
    if ((j = battle_map_unit_dbref(map, i)) > 0 && i != mech_map_slot(mech)) {
      if (!(t = btech_context_get_mech(mech_context(mech), j)))
        continue;
      if (mech_swarm_target(t) != mech_dbref(mech))
        continue;
      mech_position_mirror(t, mech, 1);
      MarkForLOSUpdate(t);
      mech_flood(t);
    }
}

int bsuit_action_validate(Mech *mech, DbRef player) {
  int i;

  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Unavailable when jumping - sorry.");
    return -1;
  }
  if (mech_swarm_target(mech) > 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are already busy with a special attack!");
    return -1;
  }
#ifdef BT_MOVEMENT_MODES
  if (mech_move_mode_locked(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Unavailable when performing movement modes - deal.");
    return -1;
  }
#endif
  for (i = 0; i < NUM_BSUIT_MEMBERS; i++) {
    if (!mech_section_is_destroyed(mech, i) &&
        mech_section_recycle_ticks(mech, i)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   tprintf("Suit %d is still recovering from attack.", i + 1));
      return -1;
    }
    if (mech_section_has_recycling_weapon(mech, i)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You have weapons recycling!");
      return -1;
    }
  }

  return 0;
}

int bsuit_member_count(const Mech *mech) {
  int i, j = 0;

  for (i = 0; i < NUM_BSUIT_MEMBERS; i++)
    if (mech_section_internal(mech, i))
      j++;
  return j;
}

int bsuit_target_find(DbRef player, Mech *mech, Mech **target, char *buffer) {
  int argc;
  char *args[3];
  float range;
  char targetID[2];
  DbRef targetnum;
  Mech *t = NULL;

  if ((argc = mech_parseattributes(buffer, args, 3)) > 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid arguments!");
    return -1;
  }
  switch (argc) {
  case 0:
    if (mech_target_dbref(mech) <= 0) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You do not have a default target set!");
      return -1;
    }
    t = btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));
    if (!(t)) {
      mech_notify(mech, MECHALL, "Invalid default target!");
      mech_targeting_target_clear(mech);
      return 1;
    }
    break;
  case 1:
    char *const *argument = checked_storage_at_const(
        args, sizeof(args) / sizeof(*args), sizeof(*args), 0);
    targetID[0] = *checked_string_suffix(*argument, 0);
    targetID[1] = *checked_string_suffix(*argument, 1);
    targetnum = FindTargetDBREFFromMapNumber(mech, targetID);
    if (targetnum <= 0) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Target is not in line of sight!");
      return -1;
    }
    t = btech_context_get_mech(mech_context(mech), targetnum);
    if (!(t)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid default target!");
      return -1;
    }
    break;
  default:
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid target!");
    return 1;
  }
  range = mech_range_to(mech, t);
  if (!mech_los_check_unblocked(mech, t, mech_position_x(t), mech_position_y(t),
                                range)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Target is not in line of sight!");
    return -1;
  }
  if (range >= 1.0F) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Target out of range!");
    return -1;
  }
  if (mech_is_jumping(t)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That target's unreachable right now!");
    return -1;
  }
  if (mech_class(t) != CLASS_MECH) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That target is of invalid type.");
    return -1;
  }
  if (mech_is_destroyed(t)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "A dead 'mech? C'mon :P");
    return -1;
  }
  *target = t;
  return 0;
}

int bsuit_jettison_validate(Mech *mech) {
  int i, j;

  if (!(mech_infantry_technology_flags(mech) & MUST_JETTISON_TECH))
    return 0;

  for (i = 0; i < NUM_BSUIT_MEMBERS; i++) {
    for (j = 0; j < NUM_CRITICALS; j++) {
      if ((mech_critical_fire_mode(mech, i, j) & WILL_JETTISON_MODE) &&
          (!(mech_critical_fire_mode(mech, i, j) & IS_JETTISONED_MODE))) {
        mech_printf(mech, MECHALL,
                    "Suit %d can not perform this feat before it jettisons its "
                    "backpack!",
                    i + 1);

        return 1;
      }
    }
  }

  return 0;
}

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
        mech_critical_handle(target, mech, 1, wLegID, 1);
        break;
      case 10:
      case 11:
        mech_critical_handle(target, mech, 1, wLegID, 2);
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
          mech_section_destroy(target, mech, 1, wLegID);
          [[fallthrough]];
        default:
          mech_critical_handle(target, mech, 1, wLegID, 3);
        }
        break;
      default:
        break;
      }
    } else {
      mech_printf(target, MECHALL,
                  "The explosives explode on the surface of your %s!",
                  strAttackLoc);
      DamageMech(target, mech, 1, mech_pilot_dbref(mech), wLegID, 0, 1, 4, 0,
                 -1, 0, -1, 0, 1);
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
