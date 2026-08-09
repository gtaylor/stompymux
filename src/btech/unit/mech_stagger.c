#include "mech_stagger.h"

#include <stdlib.h>
#include <time.h>

#include "mech_internal.h"
#include "mech_state_types.h"
#include "mux/server/platform.h"

static constexpr int STAGGER_WINDOW_SECONDS = 60;

bool mech_stagger_damage_history_is_empty(const Mech *mech) {
  return mech->rd.staggerDamageList == nullptr;
}

bool mech_stagger_damage_append(Mech *mech, int amount, time_t occurred_at,
                                DbRef attacker, bool counted) {
  MechDamageRecord **link = &mech->rd.staggerDamageList;
  MechDamageRecord *record;

  while (*link)
    link = &(*link)->next;
  record = calloc(1, sizeof(*record));
  if (!record)
    return false;
  record->amount = amount;
  record->occuredAt = occurred_at;
  record->attackerNum = attacker;
  record->counted = counted;
  *link = record;
  return true;
}

void mech_stagger_damage_mark(Mech *mech, int stagger_level) {
  MechDamageRecord *damage = mech->rd.staggerDamageList;
  int remove = stagger_level * 20;
  int sum = 0;

  while (sum < remove && damage) {
    if (damage->counted) {
      damage = damage->next;
      continue;
    }
    sum += damage->amount;
    damage->counted = 1;
    damage = damage->next;
  }
}

void mech_stagger_damage_remove(Mech *mech, int stagger_level) {
  int remove = stagger_level * 20;
  int sum = 0;

  while (sum < remove && mech->rd.staggerDamageList) {
    MechDamageRecord *damage = mech->rd.staggerDamageList;

    sum += damage->amount;
    mech->rd.staggerDamageList = damage->next;
    free(damage);
  }
}

void mech_stagger_damage_clear(Mech *mech) {
  while (mech->rd.staggerDamageList) {
    MechDamageRecord *damage = mech->rd.staggerDamageList;

    mech->rd.staggerDamageList = damage->next;
    free(damage);
  }
}

void mech_stagger_damage_expire(Mech *mech, time_t now) {
  while (mech->rd.staggerDamageList &&
         now - mech->rd.staggerDamageList->occuredAt >=
             STAGGER_WINDOW_SECONDS) {
    MechDamageRecord *damage = mech->rd.staggerDamageList;

    mech->rd.staggerDamageList = damage->next;
    free(damage);
  }
}

static int mech_stagger_damage_sum(const Mech *mech, time_t now, bool counted) {
  const MechDamageRecord *damage = mech->rd.staggerDamageList;
  int sum = 0;

  while (damage) {
    if (now - damage->occuredAt <= STAGGER_WINDOW_SECONDS &&
        (damage->counted != 0) == counted)
      sum += damage->amount;
    damage = damage->next;
  }
  return sum;
}

int mech_stagger_damage_current(const Mech *mech, time_t now) {
  return mech_stagger_damage_sum(mech, now, false);
}

int mech_stagger_damage_current_counted(const Mech *mech, time_t now) {
  return mech_stagger_damage_sum(mech, now, true);
}

bool mech_stagger_damage_get(const Mech *mech, int index,
                             MechStaggerDamageSnapshot *snapshot) {
  const MechDamageRecord *damage = mech->rd.staggerDamageList;

  while (damage && index > 0) {
    damage = damage->next;
    index--;
  }
  if (!damage)
    return false;
  snapshot->amount = damage->amount;
  snapshot->occurred_at = damage->occuredAt;
  snapshot->attacker = damage->attackerNum;
  snapshot->counted = damage->counted != 0;
  return true;
}

void mech_stagger_tracking_reset(Mech *mech) {
  mech->rd.staggerDamage = 0;
  mech->rd.lastStaggerNotify = 0;
}

int mech_stagger_level(const Mech *mech) { return mech->rd.staggerDamage / 20; }

int mech_stagger_damage_total(const Mech *mech) {
  return mech->rd.staggerDamage;
}
