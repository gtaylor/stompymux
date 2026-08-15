
/* Declares unit physical attack types and interfaces. */

#pragma once

/* mech.physical.h */
typedef enum PhysicalAttackType : int {
  PA_PUNCH = 1,
  PA_CLUB = 2,
  PA_KICK = 3,
  PA_AXE = 4,
  PA_SWORD = 5,
  PA_MACE = 6,
  PA_TRIP = 7,
  PA_SAW = 8,
  PA_CLAW = 9,
} PhysicalAttackType;

static_assert((PA_PUNCH == 1 && PA_CLAW == 9) != 0);

constexpr int P_LEFT = 1;
constexpr int P_RIGHT = 2;
