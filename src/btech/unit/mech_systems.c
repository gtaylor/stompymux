#include "btech/context.h"
#include "btech_channel.h"
#include "checked_conversion.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_utils_api.h"
#include "mech_utils_internal.h"
#include "mux/commands/action_messages.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
#include <stddef.h>
#include <stdlib.h>

static float integer_as_float(int value) { return (float)value; }

int btech_random_roll(BtechContext *context) {
  long first_roll = btech_random_range(context, 1, 6);
  long second_roll = btech_random_range(context, 1, 6);
  int i = clamp_intptr_to_int(first_roll + second_roll);

  btech_context_roll_record(context, i);
  return i;
}

int map_hex_distance(const HexDistanceRequest *request) {
  const int X1 = request->start.x;
  const int Y1 = request->start.y;
  const int X2 = request->end.x;
  const int Y2 = request->end.y;
  const int TC = request->correction;
  int xd = abs(X2 - X1);
  int yd = abs(Y2 - Y1);
  int hm;

  /* _the_ base case */
  if (X1 == X2)
    return yd;
  /*
     +
     +
     +
     +
   */
  hm = xd / 2;
  if (hm <= yd)
    return (yd - hm) + TC + xd;

  /*
     +     +
     +   +
     + +
     +
   */
  if (!yd)
    return (xd + TC);
  /*
     +
     +
     +   +
     + +
     +
   */
  /* For now, same as above */
  return (xd + TC);
}

int count_destroyed_legs(Mech *obj_mech) {
  int wc_dead_legs = 0;

  if (((obj_mech)->ud.type) != CLASS_MECH)
    return 0;

  if (mech_is_quad(obj_mech)) {
    if (is_leg_destroyed(obj_mech, LARM))
      wc_dead_legs++;

    if (is_leg_destroyed(obj_mech, RARM))
      wc_dead_legs++;
  }

  if (is_leg_destroyed(obj_mech, LLEG))
    wc_dead_legs++;

  if (is_leg_destroyed(obj_mech, RLEG))
    wc_dead_legs++;

  return wc_dead_legs;
}

int is_leg_destroyed(Mech *obj_mech, int w_loc) {
  return (mech_section_is_destroyed(obj_mech, w_loc) ||
          mech_section_is_breached(obj_mech, w_loc) ||
          mech_section_is_flooded(obj_mech, w_loc));
}

int is_mech_leg_less(Mech *obj_mech) {
  int wc_max_legs = 0;

  if (((obj_mech)->ud.type) != CLASS_MECH)
    return 0;

  if (mech_is_quad(obj_mech))
    wc_max_legs = 4;
  else
    wc_max_legs = 2;

  if (count_destroyed_legs(obj_mech) >= wc_max_legs)
    return 1;

  return 0;
}

int mech_weapon_first_critical(const WeaponCriticalSearch *search) {
  Mech *obj_mech = search->mech;
  const int W_LOC = search->weapon.section;
  const int W_SLOT = search->weapon.critical;
  const int W_START_SLOT = search->start_critical;
  const int W_CRIT_TYPE = search->part_type;
  const int W_MAX_CRITS = search->maximum_criticals;
  int w_crits_in_loc = 0;
  int w_crit_iter;
  int w_first_crit;

  /*
   * First let's count the number of crits in this loc, incase
   * we have two of the same weapon
   */

  w_first_crit = -1;

  for (w_crit_iter = W_START_SLOT; w_crit_iter < NUM_CRITICALS; w_crit_iter++) {
    if (mech_critical_part_type(obj_mech, W_LOC, w_crit_iter) == W_CRIT_TYPE) {
      w_crits_in_loc++;

      if (w_first_crit == -1)
        w_first_crit = w_crit_iter;
    }
  }

  if ((w_first_crit > -1) && (W_SLOT == -1))
    return w_first_crit;

  /*
   * Now, if there are more crits than our max crit, then we have
   * two of the same weapon in this location. We need to figure
   * out which weapon this crit actually belongs to.
   */
  if (w_crits_in_loc > W_MAX_CRITS) {
    /*
     * Well, we have thje first crit of the first instance, so
     * let's see if our crit falls out of that range.. if so, then
     * we need to figure out what range it actually falls into.
     */
    if ((w_first_crit + W_MAX_CRITS) <= W_SLOT) {
      w_first_crit = mech_weapon_first_critical(&(WeaponCriticalSearch){
          .mech = obj_mech,
          .weapon = {.section = W_LOC, .critical = W_SLOT},
          .start_critical = w_first_crit + W_MAX_CRITS,
          .part_type = W_CRIT_TYPE,
          .maximum_criticals = W_MAX_CRITS,
      });
    }
  }

  return w_first_crit;
}

int check_all_sections(Mech *mech, int special_to_find) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (check_section_for_special(mech, special_to_find, i))
      return 1;
  }

  return 0;
}

int check_section_for_special(Mech *mech, int special_to_find, int w_sec) {
  if (mech_section_is_destroyed(mech, w_sec))
    return 0;

  if (mech_section_has_special(mech, w_sec, special_to_find))
    return 1;

  return 0;
}

int get_remaining_internal_percent(Mech *mech) {
  int i;
  float w_max = 0.0F;
  float w_remaining = 0.0F;

  for (i = 0; i < NUM_SECTIONS; i++) {
    w_max += integer_as_float(mech_section_original_internal(mech, i));

    w_remaining += integer_as_float(mech_section_internal(mech, i));
  }

  if (w_max <= 0.0F)
    return 0;

  return clamp_float_to_int((w_remaining / w_max) * 100.0F);
}

int get_remaining_armor_percent(Mech *mech) {
  int i;
  float w_max = 0.0F;
  float w_remaining = 0.0F;

  for (i = 0; i < NUM_SECTIONS; i++) {
    w_max += integer_as_float(mech_section_original_armor(mech, i));
    w_max += integer_as_float(mech_section_original_rear_armor(mech, i));

    w_remaining += integer_as_float(mech_section_armor(mech, i));
    w_remaining += integer_as_float(mech_section_rear_armor(mech, i));
  }

  if (w_max <= 0.0F)
    return 0;

  return clamp_float_to_int((w_remaining / w_max) * 100.0F);
}

int find_obj(Mech *mech, int loc, int type) {
  int count = 0;
  int i;

  for (i = 0; i < NUM_CRITICALS; i++)
    if (mech_critical_part_type(mech, loc, i) == type)
      if (!mech_critical_is_nonfunctional(mech, loc, i))
        count++;
  return count;
}

int find_obj_with_dest(Mech *mech, int loc, int type) {
  int count = 0;
  int i;

  for (i = 0; i < NUM_CRITICALS; i++)
    if (mech_critical_part_type(mech, loc, i) == type)
      count++;
  return count;
}

/* Usage:
   mech      = Mech who's looking for people
   mech_map  = Map mech's on
   x,y       = Target hex
   needlos   = Bitvector
   1 = Require LOS
   2 = We actually want a mech that is friendly and has LOS to hex
 */
Mech *find_mech_in_hex(Mech *mech, BattleMap *mech_map, int x, int y,
                       int needlos) {
  Mech *target;

  for (int loop = 0; loop < battle_map_unit_count(mech_map); loop++) {
    DbRef target_dbref = battle_map_unit_dbref(mech_map, loop);
    if (target_dbref != mech->mynum && target_dbref != -1) {
      target =
          (Mech *)btech_context_find_object(mech->xcode.context, target_dbref);
      if (!target)
        continue;
      if (!(((target)->pd.x) == x && ((target)->pd.y) == y) && !(needlos & 2))
        continue;
      if (needlos) {
        if (needlos & 1)
          if (!mech_los_check(mech, target, x, y, mech_range_to(mech, target)))
            continue;
        if (needlos & 2) {
          if (((mech)->pd.team) != ((target)->pd.team))
            continue;
          if (!(mech_sees_hex(target, mech_map, x, y)))
            continue;
          if (mech == target)
            continue;
        }
      }
      return target;
    }
  }
  return nullptr;
}

AmmunitionCheckResult ammunition_check(const AmmunitionCheckRequest *request) {
  Mech *mech = request->mech;
  int weapindx = request->weapon_index;
  int section = request->weapon.section;
  int critical = request->weapon.critical;
  AmmunitionCheckResult result = {.gatling_shots = request->gatling_shots};
  int mod;
  int nmod = 0;
  int w_max_shots = 0;
  int w_rounds_to_check = 1;
  int w_weap_mode = mech_critical_fire_mode(mech, section, critical);
  int t_reset_mode = 0;
  DbRef player = mech_gunner_dbref(mech);

  /* Return if it's an energy or PC weapon */
  if (weapon_catalogue_type(weapindx) == TBEAM ||
      weapon_catalogue_type(weapindx) == THAND)
    return (AmmunitionCheckResult){.available = true,
                                   .gatling_shots = result.gatling_shots};

  /* Check for rocket launchers */
  if (weapon_catalogue_specials(weapindx) == ROCKET) {
    if (w_weap_mode & ROCKET_FIRED) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   "That weapon has already been used!");
      return result;
    }
    result.available = true;
    return result;
  }

  /* Check for One-Shots */
  if (w_weap_mode & OS_MODE) {
    if (mech_critical_fire_mode(mech, section, critical) & OS_USED) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   "That weapon has already been used!");
      return result;
    }
    result.available = true;
    return result;
  }
  /* Check RACs - No special ammo type possible */
  if (weapon_catalogue_has_special(weapindx, RAC)) {
    w_max_shots = count_ammo_for_weapon(mech, weapindx);

    if ((w_weap_mode & RAC_TWOSHOT_MODE) && (w_max_shots < 2)) {
      mech_critical_fire_mode_clear(mech, section, critical, RAC_TWOSHOT_MODE);

      result.available = true;
      return result;
    }

    if ((w_weap_mode & RAC_FOURSHOT_MODE) && (w_max_shots < 4)) {
      mech_critical_fire_mode_clear(mech, section, critical, RAC_FOURSHOT_MODE);

      result.available = true;
      return result;
    }

    if ((w_weap_mode & RAC_SIXSHOT_MODE) && (w_max_shots < 6)) {
      mech_critical_fire_mode_clear(mech, section, critical, RAC_SIXSHOT_MODE);

      result.available = true;
      return result;
    }
  }
  /* Check GMGs */
  if (w_weap_mode & GATTLING_MODE) {
    w_max_shots = count_ammo_for_weapon(mech, weapindx);

    /*
     * Gattling MGs suck up damage * 3 in ammo
     */

    if ((w_max_shots / 3) < result.gatling_shots) {
      const int AVAILABLE_SHOTS = w_max_shots / 3;
      result.gatling_shots = AVAILABLE_SHOTS > 1 ? AVAILABLE_SHOTS : 1;
    }
  }
  /* If we're an ULTRA or RFAC, we need to check for multiple rounds */
  if ((w_weap_mode & ULTRA_MODE) || (w_weap_mode & RFAC_MODE))
    w_rounds_to_check = 2;

  mod = mech_critical_ammo_mode(mech, section, critical) & AMMO_MODES;
  AmmunitionLookupRequest lookup_request = {
      .mech = mech,
      .weapon = {.section = section, .critical = critical},
      .use_weapon_preference = true,
      .weapon_index = weapindx,
      .start_section = section,
  };

  if (!mod) {
    lookup_request.forbidden_modes = AMMO_MODES;
    CriticalSlotLookupResult primary = ammunition_find(&lookup_request);
    if (!primary.found) {
      mecha_notify(
          btech_context_evaluation(mech->xcode.context), player,
          "You don't have any ammo for that weapon stored on this mech!");
      return result;
    }
    result.primary = primary;

    if (!mech_critical_data(mech, result.primary.slot.section,
                            result.primary.slot.critical)) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   "You are out of ammo for that weapon!");
      return result;
    }

    if (w_rounds_to_check > 1) {
      mech_critical_data_set(
          mech, result.primary.slot.section, result.primary.slot.critical,
          mech_critical_data(mech, result.primary.slot.section,
                             result.primary.slot.critical) -
              1);

      CriticalSlotLookupResult secondary = ammunition_find(&lookup_request);
      if (secondary.found) {
        result.secondary = secondary;
        if (!mech_critical_data(mech, result.secondary.slot.section,
                                result.secondary.slot.critical))
          t_reset_mode = 1;
      } else {
        t_reset_mode = 1;
      }

      if (t_reset_mode)
        mech_critical_fire_mode_clear(mech, section, critical, w_weap_mode);

      mech_critical_data_set(
          mech, result.primary.slot.section, result.primary.slot.critical,
          mech_critical_data(mech, result.primary.slot.section,
                             result.primary.slot.critical) +
              1);
    }
  } else {
    if (weapon_catalogue_is_artillery(weapindx))
      nmod = (~mod) & ARTILLERY_MODES;
    else
      nmod = (~mod) & AMMO_MODES;
    mod = (mod & AMMO_MODES);
    lookup_request.forbidden_modes = nmod;
    lookup_request.required_modes = mod;

    CriticalSlotLookupResult primary = ammunition_find(&lookup_request);
    if (!primary.found) {
      mecha_notify(
          btech_context_evaluation(mech->xcode.context), player,
          "You don't have any ammo for that weapon stored on this mech!");
      return result;
    }
    result.primary = primary;

    if (!mech_critical_data(mech, result.primary.slot.section,
                            result.primary.slot.critical)) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   "You are out of the special ammo type for that weapon!");
      return result;
    }

    if (w_rounds_to_check > 1) {
      mech_critical_data_set(
          mech, result.primary.slot.section, result.primary.slot.critical,
          mech_critical_data(mech, result.primary.slot.section,
                             result.primary.slot.critical) -
              1);

      CriticalSlotLookupResult secondary = ammunition_find(&lookup_request);
      if (secondary.found) {
        result.secondary = secondary;
        if (!mech_critical_data(mech, result.secondary.slot.section,
                                result.secondary.slot.critical))
          t_reset_mode = 1;
      } else {
        t_reset_mode = 1;
      }

      if (t_reset_mode)
        mech_critical_fire_mode_clear(mech, section, critical, w_weap_mode);

      mech_critical_data_set(
          mech, result.primary.slot.section, result.primary.slot.critical,
          mech_critical_data(mech, result.primary.slot.section,
                             result.primary.slot.critical) +
              1);
    }
  }

  result.available = true;
  return result;
}

void channel_emit_kill(Mech *mech, Mech *attacker, const char *reason) {
  if (!attacker)
    attacker = mech;

  /* Very Rare Occassion where using btsetxcodevalue(mech,mechdamage,) triggers
   * this, we'll just ignore */
  if ((mech->mynum == attacker->mynum) &&
      !is_good_obj(mech->xcode.context->database, mech->mynum))
    return;

  if (mech != attacker)
    ((attacker)->rd.units_killed) = ((attacker)->rd.units_killed) + 1;

  if (reason) {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                       "#%ld [%s] has been killed by #%ld [%s] (%s)",
                       mech->mynum, ((mech)->ud.mech_type), attacker->mynum,
                       ((attacker)->ud.mech_type), reason);
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEATHS,
                       "#%ld [%s] has been killed by #%ld [%s] (%s)",
                       mech->mynum, ((mech)->ud.mech_type), attacker->mynum,
                       ((attacker)->ud.mech_type), reason);
  } else {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                       "#%ld [%s] has been killed by #%ld [%s]", mech->mynum,
                       ((mech)->ud.mech_type), attacker->mynum,
                       ((attacker)->ud.mech_type));
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEATHS,
                       "#%ld [%s] has been killed by #%ld [%s]", mech->mynum,
                       ((mech)->ud.mech_type), attacker->mynum,
                       ((attacker)->ud.mech_type));
  }

  if (mech_is_dropship(mech)) {
    if (reason) {
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_DS_INFO,
                         "#%ld has been killed by #%ld (%s)", mech->mynum,
                         attacker->mynum, reason);
    } else {
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_DS_INFO,
                         "#%ld has been killed by #%ld", mech->mynum,
                         attacker->mynum);
    }
  }

  /* Trigger AMECHDEST.  */
  if (is_good_obj(mech->xcode.context->database, mech->mynum) &&
      is_good_obj(mech->xcode.context->database, attacker->mynum)) {
    char *reason_copy = NULL;

    char *args[1] = {NULL};
    int nargs = 0;

    if (reason) {
      reason_copy = alloc_lbuf("bt.reason");

      if (reason_copy) {
        /* Safe because reason is a KILL_TYPE_*. */
        (void)string_copy_bounded(reason_copy, LBUF_SIZE, reason);

        args[0] = reason_copy;
        nargs = 1;
      }
    }

    notify_event(btech_context_evaluation(attacker->xcode.context), NULL,
                 attacker->mynum, attacker->mynum, mech->mynum,
                 LUA_EVENT_MECH_DESTROYED, args, nargs);

    if (reason_copy) {
      free_lbuf(reason_copy);
    }
  }
}

#define NUM_NEIGHBORS 6
typedef struct HexOffset {
  int x;
  int y;
} HexOffset;

static const HexOffset NEIGHBOR_OFFSETS[NUM_NEIGHBORS] = {
    {0, -1}, {1, 0}, {1, 1}, {0, 1}, {-1, 1}, {-1, 0}};

void visit_neighbor_hexes(BattleMap *map, int tx, int ty,
                          NeighborHexCallback callback, void *context) {
  for (int i = 0; i < NUM_NEIGHBORS; i++) {
    const HexOffset *offset = checked_storage_at_const(
        NEIGHBOR_OFFSETS, NUM_NEIGHBORS, sizeof(*NEIGHBOR_OFFSETS), (size_t)i);
    int x1 = tx + offset->x;
    int y1 = ty + offset->y;
    if (tx % 2 && !(x1 % 2))
      y1--;
    if (x1 < 0 || x1 >= map->map_width || y1 < 0 || y1 >= map->map_height)
      continue;
    callback(map, x1, y1, context);
  }
}
