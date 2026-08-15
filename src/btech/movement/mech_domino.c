/* Implements BattleTech movement mechanics for unit domino. */

#include <math.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"
int battle_map_mech_count_in_hex(const BattleMapHexOccupancyRequest *request) {
  BattleMap *map = request->map;
  int x = request->position.x;
  int y = request->position.y;
  Mech *mech;
  int i;
  int cnt = 0;

  for (i = 0; i < battle_map_unit_count(map); i++) {
    mech = btech_context_get_mech(battle_map_context(map),
                                  battle_map_unit_dbref(map, i));
    if (mech) {
      if (mech_position_x(mech) != x || mech_position_y(mech) != y)
        continue;
      if (mech_is_destroyed(mech))
        continue;
      if (!(mech_technology_flags_secondary(mech) & CARRIER_TECH) &&
          mech_is_dropship(mech) &&
          (mech_is_landed(mech) || !mech_is_started(mech))) {
        cnt += 2;
        continue;
      }
      if (mech_class(mech) != CLASS_MECH)
        continue;
      if (mech_is_jumping(mech) || mech_is_out_of_control(mech))
        continue;
      bool same_team = mech_team(mech) == request->team;
      if (request->relationship == TEAM_RELATIONSHIP_ANY ||
          (request->relationship == TEAM_RELATIONSHIP_FRIENDLY && same_team) ||
          (request->relationship == TEAM_RELATIONSHIP_HOSTILE && !same_team))
        cnt++;
    }
  }
  return cnt;
}

typedef enum CollisionDamageTable {
  COLLISION_DAMAGE_NORMAL,
  COLLISION_DAMAGE_PUNCH,
  COLLISION_DAMAGE_KICK,
} CollisionDamageTable;

typedef struct CollisionDamageRequest {
  Mech *attacker;
  Mech *target;
  int damage;
  CollisionDamageTable table;
} CollisionDamageRequest;

static void collision_apply_damage(const CollisionDamageRequest *request) {
  Mech *att = request->attacker;
  Mech *mech = request->target;
  int dam = request->damage;
  CollisionDamageTable table = request->table;
  int hit_group;
  int isrear;
  int iscrit = 0;
  int hitloc = 0;
  int i;
  int sp = (dam - 1) / 5;

  if (!dam)
    return;
  if (att == mech)
    hit_group = FRONT;
  else
    hit_group = mech_hit_group(att, mech);
  isrear = (hit_group == BACK);
  if (mech_is_fallen(mech))
    table = COLLISION_DAMAGE_NORMAL;
  for (i = 0; i <= sp; i++) {
    switch (table) {
    case COLLISION_DAMAGE_NORMAL:
      hitloc = mech_hit_location(mech, hit_group, &iscrit, &isrear);
      break;
    case COLLISION_DAMAGE_PUNCH:
      if (mech_class(mech) != CLASS_MECH) {
        hitloc = mech_hit_location(mech, hit_group, &iscrit, &isrear);
      } else {
        hitloc = mech_punch_hit_location(mech, hit_group);
      }
      break;
    case COLLISION_DAMAGE_KICK:
      if (mech_class(mech) != CLASS_MECH) {
        hitloc = mech_hit_location(mech, hit_group, &iscrit, &isrear);
      } else {
        hitloc = mech_kick_hit_location(mech, hit_group);
      }
      break;
    }
    if (dam <= 0)
      return;
    mech_damage_apply(&(MechDamageRequest){
        .target = mech,
        .attacker = att,
        .line_of_sight = ((att == mech) ? 0 : 1) != 0,
        .attack_pilot = (att == mech) ? -1 : mech_pilot_dbref(att),
        .hit_location = hitloc,
        .rear = isrear != 0,
        .critical = iscrit != 0,
        .armor_damage = dam > 5 ? 5 : dam,
        .internal_damage = 0,
        .transfer = MECH_DAMAGE_NORMAL,
        .cause = -1,
        .base_to_hit = 0,
        .weapon_index = -1,
        .ammunition_mode = 0,
        .ignore_swarmers = false});
    dam -= 5;
  }
}

static int mech_adjusted_jump_speed_mp(const Mech *mech, const BattleMap *map) {
  float speed = mech_jump_speed(mech);

  if (mech_is_under_gravity(mech) && map != nullptr) {
    const int GRAVITY = max(50, battle_map_gravity(map));
    speed = speed * 100.0F / (float)GRAVITY;
  }
  return (int)(speed * MP_PER_KPH);
}

typedef struct MechDominoRequest {
  BattleMap *map;
  Mech *moving_mech;
  MapHexPosition position;
  TeamRelationship relationship;
  MechDominoMode mode;
  int candidate_count;
} MechDominoRequest;

static bool mech_domino_resolve_in_hex(const MechDominoRequest *request) {
  BattleMap *map = request->map;
  Mech *me = request->moving_mech;
  int x = request->position.x;
  int y = request->position.y;
  int cnt = request->candidate_count;
  MechDominoMode mode = request->mode;
  int tar = btech_random_range_int(mech_context(me), 0, cnt - 1);
  int i;
  int head;
  int td;
  Mech *mech = nullptr;
  int team = mech_team(me);

  for (i = 0; i < battle_map_unit_count(map); i++) {
    mech = btech_context_get_mech(battle_map_context(map),
                                  battle_map_unit_dbref(map, i));
    if (mech) {
      if (mech_position_x(mech) != x || mech_position_y(mech) != y)
        continue;
      if (mech == me)
        continue;
      if (mech_is_dropship(mech) &&
          (mech_is_landed(mech) || !mech_is_started(mech))) {
        tar -= 2;
      } else {
        if (!mech_is_started(mech))
          continue;
        if (mech_class(mech) != CLASS_MECH)
          continue;
        if (mech_is_jumping(mech) || mech_is_out_of_control(mech))
          continue;
        bool same_team = mech_team(mech) == team;
        if (request->relationship == TEAM_RELATIONSHIP_ANY ||
            (request->relationship == TEAM_RELATIONSHIP_FRIENDLY &&
             same_team) ||
            (request->relationship == TEAM_RELATIONSHIP_HOSTILE && !same_team))
          tar--;
        else
          continue;
      }
      if (tar <= 0)
        break;
    }
  }
  if (i == battle_map_unit_count(map))
    return false;
  /* Now we got a mech we hit, accidentally or otherwise */
  /* Next, we figure out what'll happen */

  /* 'wannabe-charge' is entirely based on the directional difference */
  /* Multiplied by the speed - if both go in same direction at same speed,
     nothing untoward happens (unlikely, though) */
  /* Jumping to a hex with multiple guys is BAD Thing(tm), though */

  switch (mode) {
  case MECH_DOMINO_JUMP:
  case MECH_DOMINO_FALL:
    td = mech_adjusted_jump_speed_mp(me, map) *
         ((mech_calculated_weight(me) / 1024) + 5) / 10;
    break;
  case MECH_DOMINO_GROUND:
  default:
    head = mech_heading_degrees(me) + mech_lateral_movement(me);
    const int HEADING_DELTA =
        head - (mech_heading_degrees(mech) + mech_lateral_movement(mech));
    const float RELATIVE_SPEED =
        mech_current_speed(me) -
        (mech_current_speed(mech) *
         cosf((float)HEADING_DELTA * (float)M_PI / 180.0F));
    const int MECH_WEIGHT = mech_calculated_weight(me);
    const float DAMAGE = fabsf(RELATIVE_SPEED * MP_PER_KPH) *
                         (((float)MECH_WEIGHT / 1024.0F) + 5.0F) / 15.0F;
    td = (int)DAMAGE;
    break;
  }
  if (td > 10)
    td = 10 + ((td - 10) / 3);
  if (td <= 1) /* No point in 1pt hits */
    return false;
  switch (mode) {
  case MECH_DOMINO_JUMP:
  case MECH_DOMINO_FALL:
    if (btech_context_stacking_mode(mech_context(mech)) == 2) {
      int factor = btech_context_stacking_damage(mech_context(mech));
      mech_printf(me, MECHALL, "You land on %s!",
                  mech_to_mech_display_id(me, mech).text);
      mech_printf(mech, MECHALL, "%s lands on you!",
                  mech_to_mech_display_id(mech, me).text);
      mech_los_broadcast_unit(me, mech, "lands on %s!");
      if (mech_is_dropship(mech)) {
        collision_apply_damage(
            &(CollisionDamageRequest){.attacker = me,
                                      .target = mech,
                                      .damage = max(1, td * factor / 500),
                                      .table = COLLISION_DAMAGE_PUNCH});
        collision_apply_damage(
            &(CollisionDamageRequest){.attacker = me,
                                      .target = me,
                                      .damage = max(1, td * factor / 100),
                                      .table = COLLISION_DAMAGE_KICK});
      } else {
        collision_apply_damage(
            &(CollisionDamageRequest){.attacker = me,
                                      .target = mech,
                                      .damage = max(1, td * factor / 100),
                                      .table = COLLISION_DAMAGE_PUNCH});
        collision_apply_damage(
            &(CollisionDamageRequest){.attacker = me,
                                      .target = me,
                                      .damage = max(1, td * factor / 500),
                                      .table = COLLISION_DAMAGE_KICK});
      }
    } else {
      mech_printf(me, MECHALL, "You nearly land on %s!",
                  mech_to_mech_display_id(me, mech).text);
      mech_printf(mech, MECHALL, "%s nearly lands on you!",
                  mech_to_mech_display_id(mech, me).text);
      mech_los_broadcast_unit(me, mech, "nearly lands on %s!");
      if (!made_pilot_skill_roll(
              me, cnt + (mech_adjusted_jump_speed_mp(me, map) / 2)))
        mech_fall(me, 1, (mech_adjusted_jump_speed_mp(me, map) / 2) != 0);
    }
    return true;
  case MECH_DOMINO_GROUND:
  default:
    break;
  }
  if (btech_context_stacking_mode(mech_context(mech)) == 2) {
    int factor = btech_context_stacking_damage(mech_context(mech));
    mech_printf(me, MECHALL, "You bump into %s!",
                mech_to_mech_display_id(me, mech).text);
    mech_printf(mech, MECHALL, "%s bumps into you!",
                mech_to_mech_display_id(mech, me).text);
    mech_los_broadcast_unit(me, mech, "bumps into %s!");
    if (mech_is_dropship(mech)) {
      collision_apply_damage(
          &(CollisionDamageRequest){.attacker = me,
                                    .target = mech,
                                    .damage = max(1, td * factor / 500),
                                    .table = COLLISION_DAMAGE_NORMAL});
      collision_apply_damage(
          &(CollisionDamageRequest){.attacker = me,
                                    .target = me,
                                    .damage = max(1, td * factor / 100),
                                    .table = COLLISION_DAMAGE_NORMAL});
    } else {
      collision_apply_damage(
          &(CollisionDamageRequest){.attacker = me,
                                    .target = mech,
                                    .damage = max(1, td * factor / 100),
                                    .table = COLLISION_DAMAGE_NORMAL});
      collision_apply_damage(
          &(CollisionDamageRequest){.attacker = me,
                                    .target = me,
                                    .damage = max(1, td * factor / 500),
                                    .table = COLLISION_DAMAGE_NORMAL});
    }
  } else {
    mech_printf(me, MECHALL, "You nearly bump into %s!",
                mech_to_mech_display_id(me, mech).text);
    mech_printf(mech, MECHALL, "%s nearly bumps into you!",
                mech_to_mech_display_id(mech, me).text);
    mech_los_broadcast_unit(me, mech, "nearly bumps into %s!");
    if (!made_pilot_skill_roll(me, cnt))
      mech_fall(me, 1, false);
    mech_movement_stop(me);
  }
  mech_charge_reset(me);
  return true;
}

bool mech_domino_resolve(Mech *mech, MechDominoMode mode) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int cnt;
  int fcnt;

  if (!map)
    return false;
  if (mech_class(mech) != CLASS_MECH)
    return false;
  if (btech_context_stacking_mode(mech_context(mech)) == 0)
    return false;
  MapHexPosition position = {.x = mech_position_x(mech),
                             .y = mech_position_y(mech)};
  cnt = battle_map_mech_count_in_hex(&(BattleMapHexOccupancyRequest){
      .map = map, .position = position, .relationship = TEAM_RELATIONSHIP_ANY});
  if (cnt <= 2)
    return false;
  /* Possible nastiness */
  fcnt = battle_map_mech_count_in_hex(&(BattleMapHexOccupancyRequest){
      .map = map,
      .position = position,
      .relationship = TEAM_RELATIONSHIP_FRIENDLY,
      .team = mech_team(mech)});
  if (fcnt > 2) {
    return mech_domino_resolve_in_hex(
        &(MechDominoRequest){.map = map,
                             .moving_mech = mech,
                             .position = position,
                             .relationship = TEAM_RELATIONSHIP_FRIENDLY,
                             .mode = mode,
                             .candidate_count = fcnt});
  }
  if (cnt > 6) {
    return mech_domino_resolve_in_hex(
        &(MechDominoRequest){.map = map,
                             .moving_mech = mech,
                             .position = position,
                             .relationship = TEAM_RELATIONSHIP_HOSTILE,
                             .mode = mode,
                             .candidate_count = cnt - fcnt});
  }
  return false;
}
