#include "mech_heat_api.h"

#include "checked_conversion.h"
#include "mech_internal.h"
#include "mech_status_types.h"
#include "section_types.h"

float mech_excess_heat(const Mech *mech) { return mech->rd.heat; }

float mech_heat_production(const Mech *mech) { return mech->rd.plus_heat; }

float mech_heat_dissipation(const Mech *mech) { return mech->rd.minus_heat; }

float mech_weapon_heat(const Mech *mech) { return mech->rd.weapheat; }

float mech_active_heat_sinks(const Mech *mech) {
  return (float)(mech->ud.numsinks - mech->rd.disabled_hs);
}

bool mech_uses_heat(const Mech *mech) {
  return (mech->ud.type == CLASS_MECH || mech->ud.type == CLASS_AERO ||
          mech->ud.type == CLASS_DS || mech->ud.type == CLASS_SPHEROID_DS) != 0;
}

float mech_added_heat(const Mech *mech) { return mech->rd.plus_heat; }

int mech_disabled_heat_sink_count(const Mech *mech) {
  return mech->rd.disabled_hs;
}

int mech_engine_heat(const Mech *mech) { return mech->rd.engineheat; }

void mech_engine_heat_set(Mech *mech, int heat) {
  mech->rd.engineheat = clamp_int_to_char(heat);
}

void mech_engine_heat_add(Mech *mech, int heat) { mech->rd.engineheat += heat; }

bool mech_heat_cutoff_is_enabled(const Mech *mech) {
  return (mech->rd.critstatus & HEATCUTOFF) != 0;
}

bool mech_life_support_is_destroyed(const Mech *mech) {
  return (mech->rd.critstatus & LIFE_SUPPORT_DESTROYED) != 0;
}

void mech_heat_production_set(Mech *mech, float heat) {
  mech->rd.plus_heat = heat;
}

void mech_heat_production_add(Mech *mech, float heat) {
  mech->rd.plus_heat += heat;
}

void mech_heat_dissipation_set(Mech *mech, float heat) {
  mech->rd.minus_heat = heat;
}

void mech_heat_dissipation_add(Mech *mech, float heat) {
  mech->rd.minus_heat += heat;
}

void mech_excess_heat_set(Mech *mech, float heat) { mech->rd.heat = heat; }

void mech_weapon_heat_set(Mech *mech, float heat) { mech->rd.weapheat = heat; }

void mech_weapon_heat_add(Mech *mech, float heat) { mech->rd.weapheat += heat; }

void mech_added_heat_add(Mech *mech, float heat) { mech->rd.plus_heat += heat; }

void mech_disabled_heat_sinks_set(Mech *mech, int count) {
  mech->rd.disabled_hs = count;
}

int mech_last_overheat_check_tick(const Mech *mech) {
  return mech->rd.heatboom_last;
}

void mech_last_overheat_check_tick_set(Mech *mech, int tick) {
  mech->rd.heatboom_last = tick;
}
