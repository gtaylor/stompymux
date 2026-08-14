/* Implements battlesuit formation and swarm state operations. */

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
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
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"

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
    if (made_pilot_skill_roll(mech, 4)) {
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

      mech_damage_apply(&(MechDamageRequest){
          .target = mech,
          .attacker = mech,
          .line_of_sight = 1,
          .attack_pilot = -1,
          .hit_location = btech_random_range_int(mech_context(mech), 0,
                                                 NUM_BSUIT_MEMBERS - 1),
          .rear = 0,
          .critical = 0,
          .armor_damage = 11,
          .internal_damage = 0,
          .transfer = MECH_DAMAGE_NORMAL,
          .cause = -1,
          .base_to_hit = 0,
          .weapon_index = -1,
          .ammunition_mode = 0,
          .ignore_swarmers = 1});

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
  int count = 0;
  int i;

  if (!map)
    return 0;

  for (i = 0; i < battle_map_unit_count(map); i++) {
    if (i != mech_map_slot(mech)) {
      const DbRef UNIT = battle_map_unit_dbref(map, i);
      if (UNIT <= 0)
        continue;
      Mech *t = btech_context_get_mech(mech_context(mech), UNIT);
      if (!t)
        continue;

      if (mech_swarm_target(t) != mech_dbref(mech))
        continue;

      if (mech_team(mech) == mech_team(t))
        continue;

      count++;
      break;
    }
  }
  return count > 0;
}

int bsuit_has_friendly_riders(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int count = 0;
  int i;

  if (!map)
    return 0;

  for (i = 0; i < battle_map_unit_count(map); i++) {
    if (i != mech_map_slot(mech)) {
      const DbRef UNIT = battle_map_unit_dbref(map, i);
      if (UNIT <= 0)
        continue;
      Mech *t = btech_context_get_mech(mech_context(mech), UNIT);
      if (!t)
        continue;

      if (mech_swarm_target(t) != mech_dbref(mech))
        continue;

      if (mech_team(mech) != mech_team(t))
        continue;

      count++;
      break;
    }
  }
  return count > 0;
}

int bsuit_swarmer_count(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int count = 0;
  int i;

  if (!map)
    return 0;
  for (i = 0; i < battle_map_unit_count(map); i++) {
    if (i != mech_map_slot(mech)) {
      const DbRef UNIT = battle_map_unit_dbref(map, i);
      if (UNIT <= 0)
        continue;
      Mech *t = btech_context_get_mech(mech_context(mech), UNIT);
      if (!t)
        continue;
      if (mech_swarm_target(t) != mech_dbref(mech))
        continue;
      count++;
    }
  }
  return count;
}

Mech *bsuit_swarmer_find(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int i;

  if (!map)
    return 0;

  for (i = 0; i < battle_map_unit_count(map); i++) {
    if (i != mech_map_slot(mech)) {
      const DbRef UNIT = battle_map_unit_dbref(map, i);
      if (UNIT <= 0)
        continue;
      Mech *t = btech_context_get_mech(mech_context(mech), UNIT);
      if (!t)
        continue;

      if (mech_swarm_target(t) == mech_dbref(mech)) {
        return t;
      }
    }
  }

  return nullptr;
}

void bsuit_swarmers_stop(BattleMap *map, Mech *mech, int intentional) {
  int i;

  if (!map || !mech)
    return;
  for (i = 0; i < battle_map_unit_count(map); i++) {
    if (i != mech_map_slot(mech)) {
      const DbRef UNIT = battle_map_unit_dbref(map, i);
      if (UNIT <= 0)
        continue;
      Mech *t = btech_context_get_mech(mech_context(mech), UNIT);
      if (!t)
        continue;
      if (mech_swarm_target(t) != mech_dbref(mech))
        continue;
      bsuit_swarm_stop(t, intentional);
    }
  }
}

void bsuit_swarmers_position_update(BattleMap *map, Mech *mech) {
  int i;

  for (i = 0; i < battle_map_unit_count(map); i++) {
    if (i != mech_map_slot(mech)) {
      const DbRef UNIT = battle_map_unit_dbref(map, i);
      if (UNIT <= 0)
        continue;
      Mech *t = btech_context_get_mech(mech_context(mech), UNIT);
      if (!t)
        continue;
      if (mech_swarm_target(t) != mech_dbref(mech))
        continue;
      mech_position_mirror(t, mech, 1);
      mark_for_los_update(t);
      mech_flood(t);
    }
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
  for (i = 0; i < NUM_BSUIT_MEMBERS; i++) {
    if (!mech_section_is_destroyed(mech, i) &&
        mech_section_recycle_ticks(mech, i)) {
      mecha_notifyf(btech_context_evaluation(mech_context(mech)), player,
                    "Suit %d is still recovering from attack.", i + 1);
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
  int i;
  int j = 0;

  for (i = 0; i < NUM_BSUIT_MEMBERS; i++)
    if (mech_section_internal(mech, i))
      j++;
  return j;
}

int bsuit_target_find(DbRef player, Mech *mech, Mech **target, char *buffer) {
  int argc;
  char *args[3];
  float range;
  char target_id[2];
  DbRef targetnum;
  Mech *t = NULL;

  argc = mech_parseattributes(buffer, args, 3);
  if (argc > 1) {
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
    char *const *argument = (char *const *)checked_storage_at_const(
        (const void *)args, sizeof(args) / sizeof(*args), sizeof(*args), 0);
    target_id[0] = *checked_string_suffix(*argument, 0);
    target_id[1] = *checked_string_suffix(*argument, 1);
    targetnum = find_target_dbref_from_map_number(mech, target_id);
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
  int i;
  int j;

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
