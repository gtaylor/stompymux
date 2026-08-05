#include "mech_progress_api.h"

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

float mech_experience_modifier(const Mech *mech) { return mech->rd.xpmod; }

void mech_shot_result_record(Mech *mech, bool hit) {
  if (hit)
    mech->rd.shots_hit++;
  else
    mech->rd.shots_missed++;
}

int mech_hexes_walked_advance(Mech *mech) { return ++mech->pd.hexes_walked; }
