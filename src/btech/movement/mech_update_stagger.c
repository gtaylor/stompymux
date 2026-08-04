/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include "mech_update_internal.h"

void MarkStaggerDamage(Mech *mech, int staggerLevel) {
  struct MechDamageRecord *damage;
  int remove = staggerLevel * 20;
  int sum = 0;

  damage = (mech)->rd.staggerDamageList;
  while (sum < remove && damage != NULL) {
    // don't recount damage
    if (damage->counted == 1) {
      damage = damage->next;
      continue;
    }
    sum += damage->amount;
    damage->counted = 1;
    damage = damage->next;
  }
}

// This method will clear any damage that has happened for staggerLevel * 20
// If you are on staggerLevel = 1, this is 20-39 points of damage.
void RemoveStaggerDamage(Mech *mech, int staggerLevel) {
  struct MechDamageRecord *damage;
  struct MechDamageRecord *old;
  int remove = staggerLevel * 20;
  int sum = 0;

  damage = (mech)->rd.staggerDamageList;

  while (sum < remove && damage != NULL) {
    sum += damage->amount;
    old = damage;
    (mech)->rd.staggerDamageList = damage->next;
    damage = damage->next;
    free(old);
  }
}
void ClearAllStaggerDamage(Mech *mech) {
  struct MechDamageRecord *damage;
  struct MechDamageRecord *old;
  damage = (mech)->rd.staggerDamageList;
  while (damage != NULL) {
    old = damage;
    (mech)->rd.staggerDamageList = damage->next;
    free(old);
    damage = (mech)->rd.staggerDamageList;
  }
}

// This method will clear any damage that has happened <oneTurn> or more seconds
// ago
void ClearStaggerDamage(Mech *mech) {
  int oneTurn = 60;
  time_t now = mech->xcode.context->clock->now;
  struct MechDamageRecord *damage;
  struct MechDamageRecord *old;

  damage = (mech)->rd.staggerDamageList;
  while (damage != NULL) {
    if (now - damage->occuredAt >= oneTurn) {
      old = damage;
      (mech)->rd.staggerDamageList = damage->next;
      // is this scary? can we just free() the data? I think so.
      free(old);
      damage = (mech)->rd.staggerDamageList;
    } else {
      // we can stop the while loop here. linked list explicitly maintains order
      break;
    }
  }
}

int CurrentStaggerDamage(Mech *mech) {
  int sum = 0;
  time_t now = mech->xcode.context->clock->now;
  int oneTurn = 60;
  struct MechDamageRecord *damage;

  damage = (mech)->rd.staggerDamageList;
  while (damage != NULL) {
    if (now - damage->occuredAt <= oneTurn) {
      if (damage->counted == 0) {
        sum += damage->amount;
      }
    }
    damage = damage->next;
  }
  return sum;
}

int CurrentCountedStaggerDamage(Mech *mech) {
  int sum = 0;
  time_t now = mech->xcode.context->clock->now;
  int oneTurn = 60;
  struct MechDamageRecord *damage;

  damage = (mech)->rd.staggerDamageList;
  while (damage != NULL) {
    if (now - damage->occuredAt <= oneTurn) {
      if (damage->counted) {
        sum += damage->amount;
      }
    }
    damage = damage->next;
  }
  return sum;
}

void CheckDamage(Mech *wounded) {
  /* should be called from UpdatePilotSkillRolls */
  /* this is so that a roll will be made only when the mech takes damage */
  int now = wounded->xcode.context->events->tick % TURN;

  if (!wounded->xcode.context->configuration->btech_newstagger) {
    if (!IsDS(wounded) && MechTurnDamage(wounded) >= 20 &&
        (!MechStaggeredLastTurn(wounded) || MechStaggerStamp(wounded) == now)) {

      if (!Jumping(wounded) && !Fallen(wounded) && !OODing(wounded)) {
        mech_notify(wounded, MECHALL, "You stagger from the damage!");
        if (!MadePilotSkillRoll(wounded, 1)) {
          mech_notify(wounded, MECHALL, "You fall over from all the damage!");
          MechLOSBroadcast(wounded, "falls down, staggered by the damage!");
          MechFalls(wounded, 1, 0);
        }
      }
      MechTurnDamage(wounded) = 0;
      SetMechStaggerStamp(wounded, now);
      return;
    }
    if ((MechStaggeredLastTurn(wounded) && MechStaggerStamp(wounded) == now) ||
        (!MechStaggeredLastTurn(wounded) && !now)) {
      MechTurnDamage(wounded) = 0;
      SetMechStaggerStamp(wounded, -1);
    }
  }
}
