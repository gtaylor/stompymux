#include "mech_specification_api.h"

#include <math.h>
#include <stdlib.h>

#include "checked_conversion.h"
#include "mech_internal.h"
#include "mech_status_types.h"
#include "mux/support/checked_storage.h"

MechMovementType mech_movement_type(const Mech *mech) {
  return (MechMovementType)mech->ud.move;
}

void mech_movement_type_set(Mech *mech, MechMovementType movement_type) {
  mech->ud.move = (char)movement_type;
}

int mech_tonnage(const Mech *mech) { return mech->ud.tons; }

void mech_tonnage_set(Mech *mech, int tonnage) { mech->ud.tons = tonnage; }

int mech_real_tonnage(const Mech *mech) { return mech->rd.row / 1024; }

int mech_engine_rating(const Mech *mech) {
  if (mech->rd.erat > 0)
    return mech->rd.erat;
  return mech_calculated_engine_rating(mech);
}

int mech_calculated_engine_rating(const Mech *mech) {
  float rating = roundf((2.0F * mech->ud.maxspeed / KPH_PER_MP) / 3.0F);

  return (int)rating * mech->ud.tons;
}

float mech_jump_speed(const Mech *mech) { return mech->rd.jumpspeed; }

void mech_jump_speed_set(Mech *mech, float speed) {
  mech->rd.jumpspeed = speed;
}

void mech_jump_speed_lower(Mech *mech, float amount) {
  mech->rd.jumpspeed -= amount;
  if (mech->rd.jumpspeed < 0.0F)
    mech->rd.jumpspeed = 0.0F;
}

int mech_heat_sink_count(const Mech *mech) { return mech->ud.numsinks; }

void mech_heat_sink_count_set(Mech *mech, int count) {
  mech->ud.numsinks = clamp_int_to_char(count);
}

void mech_heat_sink_count_remove(Mech *mech, int count) {
  mech->ud.numsinks = clamp_int_to_char(mech->ud.numsinks - count);
}

void mech_heat_sink_count_add(Mech *mech, int count) {
  mech->ud.numsinks = clamp_int_to_char(mech->ud.numsinks + count);
}

bool mech_has_double_heat_sinks(const Mech *mech) {
  return mech->rd.specials & (CLAN_TECH | DOUBLE_HEAT_TECH);
}

int mech_heat_sink_critical_size(const Mech *mech) {
  if (mech->ud.type != CLASS_MECH)
    return 1;
  if (mech->rd.specials & CLAN_TECH)
    return 2;
  return mech->rd.specials & DOUBLE_HEAT_TECH ? 3 : 1;
}

int mech_technology_flags(const Mech *mech) { return mech->rd.specials; }

void mech_technology_flags_set(Mech *mech, int flags) {
  mech->rd.specials = flags;
}

void mech_technology_flags_add(Mech *mech, int flags) {
  mech->rd.specials |= flags;
}

int mech_technology_flags_secondary(const Mech *mech) {
  return mech->rd.specials2;
}

void mech_technology_flags_secondary_set(Mech *mech, int flags) {
  mech->rd.specials2 = flags;
}

void mech_technology_flags_secondary_add(Mech *mech, int flags) {
  mech->rd.specials2 |= flags;
}

void mech_technology_flags_remove(Mech *mech, int flags) {
  mech->rd.specials &= ~flags;
}

void mech_technology_flags_secondary_remove(Mech *mech, int flags) {
  mech->rd.specials2 &= ~flags;
}

void mech_masc_technology_destroy(Mech *mech) {
  mech->rd.specials &= ~MASC_TECH;
}

void mech_supercharger_technology_destroy(Mech *mech) {
  mech->rd.specials2 &= ~SUPERCHARGER_TECH;
}

int mech_infantry_technology_flags(const Mech *mech) {
  return mech->rd.infantry_specials;
}

void mech_infantry_technology_flags_set(Mech *mech, int flags) {
  mech->rd.infantry_specials = flags;
}

void mech_infantry_technology_flags_add(Mech *mech, int flags) {
  mech->rd.infantry_specials |= flags;
}

int mech_cargo_space(const Mech *mech) { return mech->ud.cargospace; }

void mech_cargo_space_set(Mech *mech, int space) {
  mech->ud.cargospace = space;
}

void mech_cargo_space_remove(Mech *mech, int amount) {
  mech->ud.cargospace -= amount;
}

void mech_cargo_space_add(Mech *mech, int amount) {
  mech->ud.cargospace += amount;
}

int mech_carrier_maximum_tonnage(const Mech *mech) {
  return mech->ud.carmaxton;
}

void mech_carrier_maximum_tonnage_set(Mech *mech, int tonnage) {
  mech->ud.carmaxton = clamp_int_to_char(tonnage);
}

int mech_maximum_battle_suits(const Mech *mech) { return mech->rd.maxsuits; }

float mech_current_speed(const Mech *mech) { return mech->rd.speed; }

void mech_current_speed_set(Mech *mech, float speed) { mech->rd.speed = speed; }

void mech_current_speed_scale(Mech *mech, float factor) {
  mech->rd.speed *= factor;
}

void mech_current_speed_reduce_toward_zero(Mech *mech, float amount) {
  if (mech->rd.speed > 0.0F) {
    mech->rd.speed -= amount;
    if (mech->rd.speed < 0.0F)
      mech->rd.speed = 0.0F;
  } else if (mech->rd.speed < 0.0F) {
    mech->rd.speed += amount;
    if (mech->rd.speed > 0.0F)
      mech->rd.speed = 0.0F;
  }
}

float mech_maximum_speed(const Mech *mech) { return mech->ud.maxspeed; }

float mech_walking_speed(const Mech *mech) {
  return 2.0F * mech->ud.maxspeed / 3.0F;
}

float mech_template_maximum_speed(const Mech *mech) {
  return mech->ud.template_maxspeed;
}

void mech_maximum_speed_set(Mech *mech, float speed) {
  mech->ud.maxspeed = speed;
}

bool mech_is_flying_type(const Mech *mech) {
  return mech->ud.type == CLASS_AERO || mech->ud.type == CLASS_DS ||
         mech->ud.type == CLASS_SPHEROID_DS || mech->ud.move == MOVE_VTOL;
}

bool mech_is_omni(const Mech *mech) {
  return mech->rd.specials2 & OMNIMECH_TECH;
}

int mech_fuel(const Mech *mech) { return mech->ud.fuel; }

void mech_fuel_set(Mech *mech, int fuel) { mech->ud.fuel = fuel; }

void mech_fuel_decrement(Mech *mech, int amount) { mech->ud.fuel -= amount; }

int mech_original_fuel(const Mech *mech) { return mech->ud.fuel_orig; }

int mech_structural_integrity(const Mech *mech) { return mech->ud.si; }

void mech_structural_integrity_set(Mech *mech, int integrity) {
  mech->ud.si = clamp_int_to_char(integrity);
}

int mech_original_structural_integrity(const Mech *mech) {
  return mech->ud.si_orig;
}

DbRef mech_bay_dbref(const Mech *mech, int bay) {
  if (bay < 0)
    abort();
  const DbRef *slot = checked_storage_at_const(
      mech->pd.bay, NUM_BAYS, sizeof(*mech->pd.bay), (size_t)bay);
  return *slot;
}

void mech_maximum_fuel_set(Mech *mech, int fuel) { mech->rd.maxfuel = fuel; }

void mech_cargo_weight_set(Mech *mech, int weight) {
  mech->rd.cargo_weight = weight;
  mech->rd.critstatus &= ~LOAD_OK;
}

bool mech_has_sixth_sense(const Mech *mech) {
  return mech->rd.specials & SS_ABILITY;
}

void mech_sixth_sense_set(Mech *mech, bool enabled) {
  if (enabled)
    mech->rd.specials |= SS_ABILITY;
  else
    mech->rd.specials &= ~SS_ABILITY;
}

void mech_bay_dbref_set(Mech *mech, int bay, DbRef bay_dbref) {
  if (bay < 0)
    abort();
  DbRef *slot = checked_storage_at(mech->pd.bay, NUM_BAYS,
                                   sizeof(*mech->pd.bay), (size_t)bay);
  *slot = bay_dbref;
}

int mech_carried_cargo_weight(const Mech *mech) {
  return mech->rd.cargo_weight;
}

bool mech_load_cache_is_valid(const Mech *mech) {
  return mech->rd.critstatus & LOAD_OK;
}

bool mech_weight_cache_is_valid(const Mech *mech) {
  return mech->rd.critstatus & OWEIGHT_OK;
}

void mech_weight_cache_invalidate(Mech *mech) {
  mech->rd.critstatus &= ~OWEIGHT_OK;
}

bool mech_speed_cache_is_valid(const Mech *mech) {
  return mech->rd.critstatus & SPEED_OK;
}

void mech_load_cache_invalidate(Mech *mech) { mech->rd.critstatus &= ~LOAD_OK; }

int mech_cached_calculated_weight(const Mech *mech) { return mech->rd.row; }

void mech_cached_calculated_weight_set(Mech *mech, int weight) {
  mech->rd.row = weight;
}

int mech_cached_lugged_weight(const Mech *mech) { return mech->rd.rcw; }

void mech_load_cache_record(Mech *mech, int lugged_weight) {
  mech->rd.rcw = lugged_weight;
  mech->rd.critstatus |= LOAD_OK;
}

float mech_cached_maximum_speed(const Mech *mech) { return mech->rd.rspd; }

void mech_speed_cache_record(Mech *mech, float speed, int walk_xp_factor) {
  mech->rd.rspd = speed;
  mech->rd.wxf = walk_xp_factor;
  mech->rd.critstatus |= SPEED_OK;
}
