#include "mech_damage_history_api.h"

#include "mech_internal.h"

MechDamageHistory mech_damage_history(const Mech *mech) {
  return (MechDamageHistory){
      .turn_damage = mech->rd.turndamage,
      .staggered_last_turn = mech->rd.staggerstamp,
      .stagger_stamp = mech->rd.staggerstamp - 1,
  };
}

void mech_turn_damage_clear(Mech *mech) { mech->rd.turndamage = 0; }

void mech_turn_damage_add(Mech *mech, int damage) {
  mech->rd.turndamage += damage;
}

void mech_damage_taken_add(Mech *mech, int damage) {
  mech->rd.damage_taken += damage;
}

void mech_damage_inflicted_add(Mech *mech, int damage) {
  mech->rd.damage_inflicted += damage;
}

void mech_stagger_stamp_set(Mech *mech, int stamp) {
  mech->rd.staggerstamp = stamp + 1;
}
