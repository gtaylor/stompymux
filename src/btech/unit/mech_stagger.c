#include "mech_stagger.h"

#include <stdlib.h>
#include <time.h>

#include "mech_internal.h"
#include "mech_state_types.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

static constexpr int STAGGER_WINDOW_SECONDS = 60;

bool mech_stagger_damage_history_is_empty(const Mech *mech) {
  return mech->rd.stagger_damage_list == nullptr;
}

bool mech_stagger_damage_append(const StaggerDamageApplication *application) {
  Mech *mech = application->mech;
  MechDamageRecord **link = &mech->rd.stagger_damage_list;
  MechDamageRecord *record;

  while (*link)
    link = &(*link)->next;
  record = checked_storage_try_allocate_array(1, sizeof(*record));
  if (!record)
    return false;
  record->amount = application->amount;
  record->occured_at = application->occurred_at;
  record->attacker_num = application->attacker;
  record->counted = application->counted;
  *link = record;
  return true;
}

void mech_stagger_damage_mark(Mech *mech, int stagger_level) {
  MechDamageRecord *damage = mech->rd.stagger_damage_list;
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

  while (sum < remove && mech->rd.stagger_damage_list) {
    MechDamageRecord *damage = mech->rd.stagger_damage_list;

    sum += damage->amount;
    mech->rd.stagger_damage_list = damage->next;
    free(damage);
  }
}

void mech_stagger_damage_clear(Mech *mech) {
  while (mech->rd.stagger_damage_list) {
    MechDamageRecord *damage = mech->rd.stagger_damage_list;

    mech->rd.stagger_damage_list = damage->next;
    free(damage);
  }
}

void mech_stagger_damage_expire(Mech *mech, time_t now) {
  while (mech->rd.stagger_damage_list &&
         now - mech->rd.stagger_damage_list->occured_at >=
             STAGGER_WINDOW_SECONDS) {
    MechDamageRecord *damage = mech->rd.stagger_damage_list;

    mech->rd.stagger_damage_list = damage->next;
    free(damage);
  }
}

static int mech_stagger_damage_sum(const Mech *mech, time_t now, bool counted) {
  const MechDamageRecord *damage = mech->rd.stagger_damage_list;
  int sum = 0;

  while (damage) {
    if (now - damage->occured_at <= STAGGER_WINDOW_SECONDS &&
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
  const MechDamageRecord *damage = mech->rd.stagger_damage_list;

  while (damage && index > 0) {
    damage = damage->next;
    index--;
  }
  if (!damage)
    return false;
  snapshot->amount = damage->amount;
  snapshot->occurred_at = damage->occured_at;
  snapshot->attacker = damage->attacker_num;
  snapshot->counted = damage->counted != 0;
  return true;
}

void mech_stagger_tracking_reset(Mech *mech) {
  mech->rd.stagger_damage = 0;
  mech->rd.last_stagger_notify = 0;
}

int mech_stagger_level(const Mech *mech) {
  return mech->rd.stagger_damage / 20;
}

int mech_stagger_damage_total(const Mech *mech) {
  return mech->rd.stagger_damage;
}
