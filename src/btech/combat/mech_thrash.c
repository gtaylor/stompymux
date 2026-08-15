/* Implements BattleTech combat mechanics for unit thrash. */

#include <stdlib.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"

static int thrashing_limb(int index) {
  switch (index) {
  case 0:
    return RARM;
  case 1:
    return LARM;
  case 2:
    return LLEG;
  case 3:
    return RLEG;
  default:
    abort();
  }
}
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"
/*
 * - Only when fallen
 * - Tonnage / 3 (rounded up for .5)
 * - 5 Point groups to PA
 * - Clear or paved terrain only
 * - Automatically works
 * - Doesn't hit suits that are swarmed or jumping
 * - No weapons recycling in arms and legs
 * - Arms and legs recycle after attack
 * - Make pskill roll or take damage as if 1 level fall
 */

void mech_thrash(DbRef player, void *data, char *buffer [[maybe_unused]]) {
  Mech *mech = (Mech *)data;
  Mech *target;
  BtechContext *context = mech_context(mech);
  BattleMap *map = btech_context_get_map(context, mech_map_dbref(mech));
  int terrain;
  int limbs = 4;
  int i;
  int temp_loc;
  char loc_name[50];
  int damage;
  int temp_damage;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (!mech_is_fallen(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need to be prone to thrash!");
    return;
  }
  if (!map) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid map! Contact a wizard!");
    return;
  }

  terrain = (unsigned char)map_real_terrain_get(map, mech_position_x(mech),
                                                mech_position_y(mech));

  if (!((terrain == GRASSLAND) || (terrain == ROAD) || (terrain == BRIDGE))) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "Thrashing only works in clear terrain or on roads or bridges.");
    return;
  }

  /* Check locations */
  for (i = 0; i < 4; i++) {
    temp_loc = thrashing_limb(i);

    if (mech_section_is_destroyed(mech, temp_loc)) {
      limbs--;
      continue;
    }

    armor_string_from_index(temp_loc, loc_name, mech_class(mech),
                            mech_movement_type(mech));

    if (mech_section_has_recycling_weapon(mech, temp_loc)) {
      mecha_notifyf(btech_context_evaluation(context), player,
                    "You have weapons recycling on your %s.", loc_name);
      return;
    }
    if (mech_section_recycle_ticks(mech, temp_loc)) {
      mecha_notifyf(btech_context_evaluation(context), player,
                    "Your %s is still recovering from your last attack.",
                    loc_name);
      return;
    }
  }

  /* Can't thrash if we have no limbs */
  if (!limbs) {
    mech_notify(mech, MECHALL, "You can't thrash if you have no limbs!");
    return;
  }
  damage = mech_tonnage(mech) / 3;

  /* Rules say tonnage/3, not tonnage/3 * limbs  Page 151, Total Warfare*/

  mech_notify(mech, MECHALL,
              "You start to flail your arms and legs like a wild man!");
  mech_los_broadcast(mech,
                     "starts to flail its arms and legs like a wild beast!");

  /* Let's see who we can smack around */
  for (i = 0; i < battle_map_unit_count(map); i++) {
    const DbRef UNIT = battle_map_unit_dbref(map, i);
    if (UNIT >= 0) {
      target = (Mech *)btech_context_find_object(context, UNIT);

      if (!target)
        continue;

      if (mech_class(target) != CLASS_BSUIT)
        continue;

      if (mech_team(target) == mech_team(mech))
        continue;

      if (mech_is_jumping(target) || mech_is_out_of_control(target))
        continue;

      if (mech_range_to(mech, target) > 1.0F)
        continue;

      mech_printf(mech, MECHALL, "You manage to hit %s!",
                  mech_to_mech_display_id(mech, target).text);
      mech_printf(target, MECHALL, "You get hit by %s's thrashing limbs!",
                  mech_to_mech_display_id(target, mech).text);

      temp_damage = damage;

      while (temp_damage > 0) {
        if (temp_damage > 5) {
          mech_damage_apply(&(MechDamageRequest){
              .target = target,
              .attacker = mech,
              .line_of_sight = true,
              .attack_pilot = mech_pilot_dbref(mech),
              .hit_location =
                  btech_random_range_int(context, 0, NUM_BSUIT_MEMBERS - 1),
              .rear = false,
              .critical = false,
              .armor_damage = 5,
              .internal_damage = 0,
              .transfer = MECH_DAMAGE_NORMAL,
              .cause = -1,
              .base_to_hit = 0,
              .weapon_index = -1,
              .ammunition_mode = 0,
              .ignore_swarmers = true});
          temp_damage -= 5;
        } else {
          mech_damage_apply(&(MechDamageRequest){
              .target = target,
              .attacker = mech,
              .line_of_sight = true,
              .attack_pilot = mech_pilot_dbref(mech),
              .hit_location =
                  btech_random_range_int(context, 0, NUM_BSUIT_MEMBERS - 1),
              .rear = false,
              .critical = false,
              .armor_damage = temp_damage,
              .internal_damage = 0,
              .transfer = MECH_DAMAGE_NORMAL,
              .cause = -1,
              .base_to_hit = 0,
              .weapon_index = -1,
              .ammunition_mode = 0,
              .ignore_swarmers = true});
          temp_damage = 0;
        }
      }
    }
  }

  /* Make our roll and recycle our limbs -- Removed. You gotta be prone anyways!
   */
  /* Dunno who commented this out. This is what it should be. You make a pilot
   * roll. if you miss, you take 1 level falling damage to emulate hitting
   * yourself */

  if (!mech_pilot_skill_roll(&(PilotSkillRollRequest){.mech = mech})) {
    mech_fall(mech, 1, true);
  }

  for (i = 0; i < 4; i++) {
    temp_loc = thrashing_limb(i);

    if (mech_section_is_destroyed(mech, temp_loc))
      continue;

    mech_set_recycle_limb(mech, temp_loc, PHYSICAL_RECYCLE_TIME);
  }
}
