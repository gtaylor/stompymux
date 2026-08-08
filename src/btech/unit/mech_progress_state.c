#include "mech_progress_api.h"

#include "checked_conversion.h"
#include "mech_internal.h"

bool mech_piloting_position_mark_changed(Mech *mech) {
  if (mech->rd.lx == mech->pd.x && mech->rd.ly == mech->pd.y)
    return false;
  mech->rd.lx = mech->pd.x;
  mech->rd.ly = mech->pd.y;
  return true;
}

int mech_battle_value(const Mech *mech) { return mech->ud.mechbv; }

void mech_battle_value_set(Mech *mech, int battle_value) {
  mech->ud.mechbv = battle_value;
}

double mech_experience_modifier(const Mech *mech) {
  return (double)mech->rd.xpmod;
}

void mech_shot_result_record(Mech *mech, bool hit) {
  if (hit)
    mech->rd.shots_hit++;
  else
    mech->rd.shots_missed++;
}

void mech_shots_fired_increment(Mech *mech) { mech->rd.shots_fired++; }

int mech_hexes_walked_advance(Mech *mech) {
  mech->pd.hexes_walked += 1.0F;
  return clamp_float_to_int(mech->pd.hexes_walked);
}
