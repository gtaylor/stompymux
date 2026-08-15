/* Declares the BattleTech unit physical API. */

#pragma once

#include "mech_physical.h"
#include "mux/server/platform.h"

/* mech.physical.c */
void mech_punch(DbRef player, void *data, char *buffer);
void mech_club(DbRef player, void *data, char *buffer);
int have_axe(Mech *mech, int loc);
int have_sword(Mech *mech, int loc);
int have_mace(Mech *mech, int loc);
int have_saw(Mech *mech, int loc);
int have_claw(Mech *mech, int loc);
void mech_saw(DbRef player, void *data, char *buffer);
void mech_axe(DbRef player, void *data, char *buffer);
void mech_sword(DbRef player, void *data, char *buffer);
void mech_mace(DbRef player, void *data, char *buffer);
void mech_claw(DbRef player, void *data, char *buffer);
void mech_kick(DbRef player, void *data, char *buffer);
void mech_trip(DbRef player, void *data, char *buffer);
void mech_kickortrip(DbRef player, void *data, char *buffer,
                     PhysicalAttackType attack_type);
void mech_charge(DbRef player, void *data, char *buffer);
typedef struct PhysicalVerbRequest {
  PhysicalAttackType attack_type;
  bool third_person;
} PhysicalVerbRequest;

const char *physical_attack_verb(const PhysicalVerbRequest *request);
void phys_succeed(Mech *mech, Mech *target, PhysicalAttackType attack_type);
void phys_fail(Mech *mech, Mech *target, PhysicalAttackType attack_type);
typedef struct PhysicalAttackRequest {
  Mech *mech;
  int damage_weight;
  int base_to_hit;
  PhysicalAttackType attack_type;
  int argument_count;
  char **arguments;
  BattleMap *map;
  int section;
} PhysicalAttackRequest;

void physical_attack_resolve(const PhysicalAttackRequest *request);
void physical_trip(Mech *mech, Mech *target);
typedef struct PhysicalDamageRequest {
  Mech *attacker;
  Mech *target;
  int weight_divisor;
  PhysicalAttackType attack_type;
  int section;
  int glancing_damage;
} PhysicalDamageRequest;

void physical_damage_resolve(const PhysicalDamageRequest *request);
bool death_from_above(Mech *mech, Mech *target);
void charge_mech(Mech *mech, Mech *target);
bool mech_club_location_is_usable(Mech *mech, int section, bool emit_failure);
void mech_grabclub(DbRef player, void *data, char *buffer);
