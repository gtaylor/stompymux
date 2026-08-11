/* Implements BattleTech combat mechanics for unit hit resolution. */

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "failures.h"
#include "map.h"
#include "map_coordinates.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_combat_api.h"
#include "mech_combat_missile_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_events_api.h"
#include "mech_heat_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_spot_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/server/platform.h"
#include "pcombat_api.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

int mech_hit_damage_determine(const HitDamageRequest *request) {
  Mech *mech = request->attacker;
  const int W_SECTION = request->weapon.section;
  const int W_CRIT_SLOT = request->weapon.critical;
  Mech *hit_mech = request->target;
  const int HIT_X = request->target_hex.x;
  const int HIT_Y = request->target_hex.y;
  const int WEAPINDX = request->weapon_index;
  const int W_GATTLING_SHOTS = request->gatling_shots;
  const int W_BASE_WEAP_DAMAGE = request->base_damage;
  const int W_AMMO_MODE = request->ammunition_mode;
  const int TYPE = request->failure_type;
  const int MODIFIER = request->failure_modifier;
  const bool IS_TEMP_CALC = request->temporary_calculation;
  BattleMap *mech_map;
  float f_range = 0.0;
  int w_weap_damage = W_BASE_WEAP_DAMAGE;
  int w_clear_damage = 0;
  const WeaponRangeProfile RANGES = weapon_catalogue_ranges(WEAPINDX);

  /* Find the range to our target */
  if (hit_mech)
    f_range = mech_range_to(mech, hit_mech);
  else {
    float fx, fy;
    map_coord_to_real_coord(HIT_X, HIT_Y, &fx, &fy);
    f_range = map_real_range(&(MapRealSegment){
        .start = {.x = mech_position_real_x(mech),
                  .y = mech_position_real_y(mech)},
        .end = {.x = fx, .y = fy},
    });
  }

  /* If our Gattling shots are greater then 0, use that as the damage. */
  if (W_GATTLING_SHOTS > 0)
    w_weap_damage = W_GATTLING_SHOTS;

  /* If we're a heavy gauss rifle, damage gets altered by range. */
  if (weapon_catalogue_is_heavy_gauss(WEAPINDX)) {
    if (f_range > (float)RANGES.medium_range)
      w_weap_damage = 10;
    else if (f_range > (float)RANGES.short_range)
      w_weap_damage = 20;
  }

  /* If we're a snub ppc, damage gets altered by range. */
  if (weapon_catalogue_is_snub_ppc(WEAPINDX)) {
    if (f_range > (float)RANGES.medium_range)
      w_weap_damage = 5;
    else if (f_range > (float)RANGES.short_range)
      w_weap_damage = 8;
  }

  w_weap_damage -=
      mech_weapon_critical_damage_penalty(mech, W_SECTION, W_CRIT_SLOT);

  /* See if we're using flechette ammo */
  if (hit_mech) {
    if (W_AMMO_MODE & AC_FLECHETTE_MODE) {
      if (mech_class(hit_mech) == CLASS_MW) {
        if (mech_real_terrain_get(hit_mech) == GRASSLAND)
          w_weap_damage *= 4;
        else
          w_weap_damage *= 2;
      } else if (mech_class(hit_mech) != CLASS_BSUIT)
        w_weap_damage /= 2;
    }

    if (W_AMMO_MODE & AC_INCENDIARY_MODE) {
      if (mech_class(hit_mech) == CLASS_MW)
        w_weap_damage += 2;
    }
  }

  /* Check to see if we have an energy weapon and we're modding the damage based
   * on range */
  if (btech_context_range_modifies_damage(mech_context(mech)) &&
      weapon_catalogue_is_energy(WEAPINDX)) {
    if (f_range <= 1.0F)
      w_weap_damage++;
    else {
      if (mech_section_is_underwater(mech, W_SECTION)) {
        if (f_range > (float)RANGES.water_long_range)
          w_weap_damage = (w_weap_damage / 2);
        else if (f_range > (float)RANGES.water_medium_range)
          w_weap_damage--;
      } else {
        if (f_range > (float)RANGES.long_range)
          w_weap_damage = (w_weap_damage / 2);
        else if (f_range > (float)RANGES.medium_range)
          w_weap_damage--;
      }
    }
  }

  /* Check to see if we're modding the damage based on woods cover */
  mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  /* If there was a damage type failure, mod the damage */
  if (TYPE == DAMAGE)
    w_weap_damage -= MODIFIER;

  if (hit_mech && !IS_TEMP_CALC) {
    if (btech_context_woods_modify_damage(mech_context(mech)) &&
        battle_terrain_is_forest(map_real_terrain_get(
            mech_map, mech_position_x(hit_mech), mech_position_y(hit_mech))) &&
        ((mech_position_z(hit_mech) - 2) <=
         battle_map_hex_elevation(mech_map, mech_position_x(hit_mech),
                                  mech_position_y(hit_mech)))) {
      w_clear_damage = w_weap_damage;

      if (map_real_terrain_get(mech_map, mech_position_x(hit_mech),
                               mech_position_y(hit_mech)) == LIGHT_FOREST)
        w_weap_damage -= 2;
      else if (map_real_terrain_get(mech_map, mech_position_x(hit_mech),
                                    mech_position_y(hit_mech)) == HEAVY_FOREST)
        w_weap_damage -= 4;

      mech_notify(mech, MECHALL, "The woods absorb some of your shot!");
      mech_notify(hit_mech, MECHALL, "The woods absorb some of the damage!");

      mech_terrain_possibly_ignite_or_clear(&(TerrainWeaponEffectRequest){
          .mech = mech,
          .position = {.x = mech_position_x(hit_mech),
                       .y = mech_position_y(hit_mech)},
          .weapon_index = WEAPINDX,
          .ammunition_mode = W_AMMO_MODE,
          .damage = w_clear_damage,
          .intentional = true});
    }
  }

  if (w_weap_damage <= 0)
    w_weap_damage = 1;

  return w_weap_damage;
}

static int missile_hit_count(const Mech *mech, int weapon_index,
                             const char *fake_name, bool uses_fake_name,
                             int roll_index) {
  BtechContext *context = mech_context(mech);
  return uses_fake_name ? btech_context_missile_hit_count_by_name(
                              context, fake_name, roll_index)
                        : btech_context_missile_hit_count(&(MissileHitLookup){
                              .context = context,
                              .weapon = weapon_index,
                              .roll = roll_index,
                          });
}

void mech_hit_resolve(const HitResolutionRequest *request) {
  Mech *mech = request->attacker;
  const int WEAPINDX = request->weapon_index;
  const int W_SECTION = request->weapon.section;
  const int W_CRIT_SLOT = request->weapon.critical;
  Mech *hit_mech = request->target;
  const int HIT_X = request->target_hex.x;
  const int HIT_Y = request->target_hex.y;
  const int LOS = request->line_of_sight;
  const int TYPE = request->failure_type;
  const int MODIFIER = request->failure_modifier;
  const bool REALLYHIT = request->hit;
  const int BTH = request->base_to_hit;
  const int W_GATTLING_SHOTS = request->gatling_shots;
  const bool T_IS_SWARM_ATTACK = request->swarm_attack;
  const int PLAYER_ROLL = request->player_roll;
  int isrear = 0, iscritical = 0;
  int hitloc = 0;
  int roll;
  int aim_hit = 0;
  int w_base_weap_damage = weapon_catalogue_damage(WEAPINDX);
  int w_weap_damage = 0;
  int num_missiles_hit;
  int w_fire_mode = mech_critical_fire_mode(mech, W_SECTION, W_CRIT_SLOT);
  int w_ammo_mode = mech_critical_ammo_mode(mech, W_SECTION, W_CRIT_SLOT);
  int t_is_ultra = ((w_fire_mode & ULTRA_MODE) || (w_fire_mode & RFAC_MODE));
  int t_is_rac = (w_fire_mode & RAC_MODES);
  int t_is_lbx = (w_ammo_mode & LBX_MODE);
  int t_is_swarm = ((w_ammo_mode & SWARM_MODE) || (w_ammo_mode & SWARM1_MODE));
  const char *missile_fake_name = nullptr;
  int maximum_missile_hits;
  int t_using_tc =
      ((w_fire_mode & ON_TC) && !weapon_catalogue_is_artillery(WEAPINDX) &&
       !weapon_catalogue_is_missile(WEAPINDX) &&
       !mech_condition_summary(mech).targeting_computer_destroyed &&
       ((mech_aim_section(mech) != NUM_SECTIONS) && hit_mech &&
        (mech_aim_unit_class(mech) == mech_class(hit_mech))));
  int missileindex = 0;

  if (hit_mech) {

    /* Check to see if we're aiming at a particular location. Swarm attacks
     * can't aim. */
    if ((mech_aim_section(mech) != NUM_SECTIONS) && hit_mech &&
        mech_is_immobile(hit_mech) && !T_IS_SWARM_ATTACK) {

      roll = btech_random_roll(mech_context(mech));

      if (roll == 6 || roll == 7 || roll == 8)
        aim_hit = 1;
    }
  }

  if (!weapon_catalogue_is_missile(WEAPINDX)) {
    w_weap_damage = mech_hit_damage_determine(&(HitDamageRequest){
        .attacker = mech,
        .weapon = {.section = W_SECTION, .critical = W_CRIT_SLOT},
        .target = hit_mech,
        .target_hex = {.x = HIT_X, .y = HIT_Y},
        .weapon_index = WEAPINDX,
        .gatling_shots = W_GATTLING_SHOTS,
        .base_damage = w_base_weap_damage,
        .ammunition_mode = w_ammo_mode,
        .failure_type = TYPE,
        .failure_modifier = MODIFIER});

    /* Check if it is a glancing blow, if so, make an emit */
    if (btech_context_glancing_blows_enabled(mech_context(mech)) &&
        (PLAYER_ROLL == BTH) && hit_mech) {
      /* Yes, even though we have two different glance modes, the above is
       * correct because we modified the bth in FireWeapon. Nothing to see here.
       * move along
       */
      mech_los_broadcast(hit_mech, "is nicked by a glancing blow!");
      mech_notify(hit_mech, MECHALL, "You are nicked by a glancing blow!");
      w_weap_damage = (int)(w_weap_damage + 1) / 2;
      if (w_weap_damage < 1)
        w_weap_damage = 1; /* very rare case */
    }
  }

  /*
   * Ok, if we're not an artillery weapon or missile and we're not in
   * LBX, RAC, Ultra or RFAC mode...
   */
  if (!weapon_catalogue_is_artillery(WEAPINDX) &&
      !weapon_catalogue_is_missile(WEAPINDX) && !t_is_ultra && !t_is_lbx &&
      !t_is_rac) {

    if (hit_mech) {

      /* Flamers - if in heat mode don't do damage */
      if ((weapon_catalogue_is_flamer(WEAPINDX)) && (w_fire_mode & HEAT_MODE)) {

        mech_notify(
            hit_mech, MECHALL,
            "[fg=yellow bold]The flaming plasma sprays all over you![reset]");
        mech_notify(
            mech, MECHALL,
            "[fg=green]You cover your target in flaming plasma![reset]");
        mech_weapon_heat_add(hit_mech, (float)w_base_weap_damage);
        return;

      } else if ((weapon_catalogue_is_coolant(WEAPINDX)) &&
                 (mech_class(hit_mech) != CLASS_MW)) {

        /* Its a Coolant Gun */
        /* So now we figure out if we want to hit our unit with it
         * or a target */

        if (w_fire_mode & HEAT_MODE) {

          /* Hit our own unit with the coolant gun */
          mech_notify(mech, MECHALL,
                      "[fg=cyan]Coolant washes over your systems!![reset]");
          mech_weapon_heat_add(mech, -(float)w_base_weap_damage);

        } else {

          /* Hit the target with the coolant gun */
          mech_notify(mech, MECHALL,
                      "[fg=cyan]You hit with the stream of coolant!![reset]");
          mech_notify(hit_mech, MECHALL,
                      "[fg=cyan]Coolant washes over your systems!![reset]");
          mech_weapon_heat_add(hit_mech, -(float)w_base_weap_damage);
        }

        /* Never does damage so return */
        return;
      }

      if (aim_hit)
        hitloc = mech_aimed_hit_location(mech, hit_mech, &isrear, &iscritical);
      else if (t_using_tc)
        hitloc = mech_targeting_computer_hit_location(mech, hit_mech, &isrear,
                                                      &iscritical);
      else
        hitloc = mech_target_hit_location(mech, hit_mech, &isrear, &iscritical);

      mech_damage_apply(&(MechDamageRequest){
          .target = hit_mech,
          .attacker = mech,
          .line_of_sight = LOS,
          .attack_pilot = mech_gunner_dbref(mech),
          .hit_location = hitloc,
          .rear = isrear,
          .critical = iscritical,
          .armor_damage =
              personal_combat_damage_to_unit(&(PersonalCombatDamageConversion){
                  .target = hit_mech,
                  .weapon_index = WEAPINDX,
                  .damage = w_weap_damage,
              }),
          .internal_damage = 0,
          .transfer = MECH_DAMAGE_NORMAL,
          .cause = WEAPINDX,
          .base_to_hit = BTH,
          .weapon_index = WEAPINDX,
          .ammunition_mode = w_ammo_mode,
          .ignore_swarmers = T_IS_SWARM_ATTACK});

    } else {
      mech_terrain_hex_hit(
          &(TerrainWeaponHitRequest){.attacker = mech,
                                     .position = {.x = HIT_X, .y = HIT_Y},
                                     .weapon_index = WEAPINDX,
                                     .ammunition_mode = w_ammo_mode,
                                     .damage = w_weap_damage,
                                     .hit = true});
    }

    return;
  }

  /*
   * Since we're here, we're either
   *      - A missile weapon
   *      - An artillery weapon
   *      - An AC in Ultra, RF or LBX mode
   *      - A RAC in RAC mode
   */

  /*
   * Do special case for RACs since they don't have an entry in the
   * missile cluster registry.
   *
   * We're gonna fake it by pretending we're either an SRM-2, SRM-4 or SRM-6,
   * depending upon the mode
   */
  if (t_is_rac) {
    if (mech_critical_fire_mode(mech, W_SECTION, W_CRIT_SLOT) &
        RAC_TWOSHOT_MODE)
      missile_fake_name = "IS.SRM-2";
    else if (mech_critical_fire_mode(mech, W_SECTION, W_CRIT_SLOT) &
             RAC_FOURSHOT_MODE)
      missile_fake_name = "IS.SRM-4";
    else if (mech_critical_fire_mode(mech, W_SECTION, W_CRIT_SLOT) &
             RAC_SIXSHOT_MODE)
      missile_fake_name = "IS.SRM-6";
  }
  maximum_missile_hits =
      missile_hit_count(mech, WEAPINDX, missile_fake_name, t_is_rac, 10);
  if (maximum_missile_hits == 0)
    return;

  if (weapon_catalogue_is_missile(WEAPINDX)) {
    if (PLAYER_ROLL < BTH)
      return;

    if (t_is_swarm && hit_mech) /* No swarms on hex hits */
      mech_swarm_missile_hit_target(&(MissileAttackRequest){
          .attacker = mech,
          .target = hit_mech,
          .weapon = {.weapon_index = WEAPINDX,
                     .section = W_SECTION,
                     .critical = W_CRIT_SLOT},
          .los = LOS,
          .base_to_hit = BTH,
          .roll = REALLYHIT ? BTH + 1 : BTH - 1,
          .incoming = (TYPE == CRAZY_MISSILES)
                          ? maximum_missile_hits * MODIFIER / 100
                          : maximum_missile_hits,
          .friend_or_foe =
              mech_critical_ammo_mode(mech, W_SECTION, W_CRIT_SLOT) &
              SWARM1_MODE,
          .swarm_attack = T_IS_SWARM_ATTACK,
          .player_roll = PLAYER_ROLL,
      });
    else
      (void)mech_missile_hit_target(&(MissileAttackRequest){
          .attacker = mech,
          .target = hit_mech,
          .weapon = {.weapon_index = WEAPINDX,
                     .section = W_SECTION,
                     .critical = W_CRIT_SLOT},
          .target_hex = {.x = HIT_X, .y = HIT_Y},
          .los = LOS ? 1 : 0,
          .base_to_hit = BTH,
          .roll = REALLYHIT ? BTH + 1 : BTH - 1,
          .incoming = (TYPE == CRAZY_MISSILES)
                          ? maximum_missile_hits * MODIFIER / 100
                          : maximum_missile_hits,
          .swarm_attack = T_IS_SWARM_ATTACK,
          .player_roll = PLAYER_ROLL,
      });

    return;
  }

  missileindex = mech_missile_hit_index(&(MissileHitIndexRequest){
      .attacker = mech,
      .target = hit_mech,
      .weapon = {.weapon_index = WEAPINDX,
                 .section = W_SECTION,
                 .critical = W_CRIT_SLOT},
      .glancing = btech_context_glancing_blows_enabled(mech_context(mech)) &&
                  PLAYER_ROLL == BTH,
  });
  /* This is how we'll handle glancing. Any roll < 2 is considering just one
   * missile hit, full damage */
  if (missileindex == -1)
    num_missiles_hit = 1;
  else
    num_missiles_hit = missile_hit_count(mech, WEAPINDX, missile_fake_name,
                                         t_is_rac, missileindex);

  /*
   * Check for non-missile, multiple hit weapons, like LBXs, RACs, RFACs and
   * Ultras
   */
  if (LOS)
    mech_printf(mech, MECHALL, "[fg=green]You hit with %d %s%s![reset]",
                num_missiles_hit,
                (t_is_ultra || t_is_rac ? "slug"
                 : t_is_lbx             ? "pellet"
                                        : "missile"),
                (num_missiles_hit > 1 ? "s" : ""));

  if (t_is_lbx)
    mech_missile_apply_hits(&(MissileHitsRequest){
        .attacker = mech,
        .target = hit_mech,
        .target_hex = {.x = HIT_X, .y = HIT_Y},
        .rear = isrear,
        .critical = iscritical,
        .weapon = {.weapon_index = WEAPINDX,
                   .section = W_SECTION,
                   .critical = W_CRIT_SLOT},
        .fire_mode = w_fire_mode,
        .ammunition_mode = w_ammo_mode,
        .missile_count = num_missiles_hit,
        .damage_per_missile = t_is_lbx ? 1 : w_weap_damage,
        .salvo_size = weapon_catalogue_cluster_size(WEAPINDX),
        .los = LOS,
        .base_to_hit = BTH,
        .swarm_attack = T_IS_SWARM_ATTACK,
    });
  else {
    while (num_missiles_hit) {
      if (hit_mech) {
        if (t_using_tc)
          hitloc = mech_targeting_computer_hit_location(mech, hit_mech, &isrear,
                                                        &iscritical);
        else
          hitloc =
              mech_target_hit_location(mech, hit_mech, &isrear, &iscritical);
        mech_damage_apply(
            &(MechDamageRequest){.target = hit_mech,
                                 .attacker = mech,
                                 .line_of_sight = LOS,
                                 .attack_pilot = mech_gunner_dbref(mech),
                                 .hit_location = hitloc,
                                 .rear = isrear,
                                 .critical = iscritical,
                                 .armor_damage = personal_combat_damage_to_unit(
                                     &(PersonalCombatDamageConversion){
                                         .target = hit_mech,
                                         .weapon_index = WEAPINDX,
                                         .damage = w_weap_damage,
                                     }),
                                 .internal_damage = 0,
                                 .transfer = MECH_DAMAGE_NORMAL,
                                 .cause = WEAPINDX,
                                 .base_to_hit = BTH,
                                 .weapon_index = WEAPINDX,
                                 .ammunition_mode = w_ammo_mode,
                                 .ignore_swarmers = T_IS_SWARM_ATTACK});
      } else
        mech_terrain_hex_hit(
            &(TerrainWeaponHitRequest){.attacker = mech,
                                       .position = {.x = HIT_X, .y = HIT_Y},
                                       .weapon_index = WEAPINDX,
                                       .ammunition_mode = w_ammo_mode,
                                       .damage = w_weap_damage,
                                       .hit = true});

      num_missiles_hit--;
    }
  }
}

/****************************************
 * Start: Hex hitting related functions
 ****************************************/
