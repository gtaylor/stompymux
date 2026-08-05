#include "mech_update_internal.h"

#include "mech_ammunition_explosion_api.h"

void mech_overheat_handle(Mech *mech) {
  int avoided = 0, hasinferno = 0;
  BattleMap *mech_map;
  int ammoloc, ammocritnum, damage = 0;

  if (MechHeat(mech) < 10.)
    return;
  if ((MechHeatLast(mech) + TURN) > mech->xcode.context->events->tick)
    return;
  MechHeatLast(mech) = mech->xcode.context->events->tick;

  if (MechHeat(mech) >= 10.) {
    if (mech->xcode.context->configuration->btech_inferno_penalty)
      hasinferno = FindInfernoAmmo(mech, &ammoloc, &ammocritnum);
    if (MechHeat(mech) >= 28.) {
      if (hasinferno) {
        if (btech_random_roll(mech->xcode.context) >= 12)
          avoided = 1;
      } else if (btech_random_roll(mech->xcode.context) >= 8)
        avoided = 1;
    } else if (MechHeat(mech) >= 23.) {
      if (hasinferno) {
        if (btech_random_roll(mech->xcode.context) >= 10)
          avoided = 1;
      } else if (btech_random_roll(mech->xcode.context) >= 6)
        avoided = 1;
    } else if (MechHeat(mech) >= 19.) {
      if (hasinferno) {
        if (btech_random_roll(mech->xcode.context) >= 8)
          avoided = 1;
      } else if (btech_random_roll(mech->xcode.context) >= 4)
        avoided = 1;
    } else if ((MechHeat(mech) >= 14.) && hasinferno) {
      if (btech_random_roll(mech->xcode.context) >= 6)
        avoided = 1;
    } else if ((MechHeat(mech) >= 10.) && hasinferno) {
      if (btech_random_roll(mech->xcode.context) >= 4)
        avoided = 1;
    } else if ((MechHeat(mech) < 19.) && !hasinferno)
      avoided = 1;

    if (!avoided) {
      if (!hasinferno)
        damage = FindDestructiveAmmo(mech, &ammoloc, &ammocritnum);
      else
        damage = hasinferno;
      if (damage)
        mech_ammunition_explode(mech, mech, ammoloc, ammocritnum, damage);
      else
        mech_notify(mech, MECHALL, "You have no ammunition, lucky you!");
    }
  }

  avoided = 0;
#ifdef BT_EXILE_MW3STATS
  if (!is_player(mech->xcode.context->database, MechPilot(mech))) {
#endif
    if (MechHeat(mech) >= 30.) {
    } else if (MechHeat(mech) >= 26.) {
      if (btech_random_roll(mech->xcode.context) >= 10)
        avoided = 1;
    } else if (MechHeat(mech) >= 22.) {
      if (btech_random_roll(mech->xcode.context) >= 8)
        avoided = 1;
    } else if (MechHeat(mech) >= 18.) {
      if (btech_random_roll(mech->xcode.context) >= 6)
        avoided = 1;
    } else if (MechHeat(mech) >= 14.) {
      if (btech_random_roll(mech->xcode.context) >= 4)
        avoided = 1;
    }
#ifdef BT_EXILE_MW3STATS
  } else {
    avoided = 1;
    if (MechHeat(mech) >= 14.) {
      mech_notify(mech, MECHALL,
                  "You frantically attempt to override the shutdown process!");
      avoided =
          char_getskillsuccess(mech->xcode.context, MechPilot(mech), "computer",
                               (MechHeat(mech) >= 30.   ? 8
                                : MechHeat(mech) >= 26. ? 6
                                : MechHeat(mech) >= 22. ? 4
                                : MechHeat(mech) >= 18. ? 2
                                                        : 0),
                               1);
      if (avoided)
        AccumulateComputerXP(MechPilot(mech), mech, 1);
    }
  }
#endif
  if (!avoided && Started(mech)) {
    if (MechStatus(mech) & STARTED)
      mech_notify(mech, MECHALL,
                  "[fg=red inverse]Reactor shutting down...[reset]");
    if (MechStatus2(mech) & SLITE_ON) {
      mech_notify(mech, MECHALL, "Your searchlight shuts off.");
      MechStatus2(mech) &= ~SLITE_ON;
      MechCritStatus(mech) &= ~SLITE_LIT;
    }
    if (Jumping(mech) || OODing(mech) || (is_aero(mech) && !Landed(mech))) {
      mech_notify(mech, MECHALL, "[bold]You fall from the sky![reset]");
      mech_los_broadcast(mech, "falls from the sky!");
      mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);
      mech_fall(mech, JumpSpeedMP(mech, mech_map), 0);
      mech_domino_resolve(mech, MECH_DOMINO_FALL);
    } else {
      mech_los_broadcast(mech, "stops in mid-motion!");
      if ((fabs(MechSpeed(mech)) > MP1) && !Fallen(mech) &&
          !MadePilotSkillRoll(mech, 3))
        mech_fall(mech, 0, 1);
    }
    mech_power_down(mech);
    mech_event_cancel(mech, EVENT_MOVE);
    mech_event_cancel(mech, EVENT_STAND);
  }
}
