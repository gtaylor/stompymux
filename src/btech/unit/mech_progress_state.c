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

float mech_experience_modifier(const Mech *mech) { return mech->rd.xpmod; }
