
/*
 * $Id: mech.physical.h,v 1.1.1.1 2005/01/11 21:18:21 kstevens Exp $
 *
 * Author: Cord Awtry <kipsta@mediaone.net>
 *
 *  Copyright (c) 2001 Cord Awtry
 *       All rights reserved
 *
 * Created: Mon Apr 2 8:00:00 2001 spectre
 */

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

static_assert(PA_PUNCH == 1 && PA_CLAW == 9);

constexpr int P_LEFT = 1;
constexpr int P_RIGHT = 2;
