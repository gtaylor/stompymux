/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <stdlib.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_restrict_api.h"
#include "mech_utils_api.h"
#include "mux/commands/action_messages.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"

void mech_ood_damage(Mech *wounded, Mech *attacker, int damage) {
  mech_printf(attacker, MECHALL,
              "[fg=green]You hit the cocoon for %d points of damage![reset]",
              damage);
  mech_printf(wounded, MECHALL,
              "[fg=yellow bold]Your cocoon has been hit for %d points of "
              "damage![reset]",
              damage);
  MechCocoon(wounded) = MAX(0, MechCocoon(wounded) - damage);
  if (MechCocoon(wounded))
    return;
  /* Abort the OOD and initiate falling */
  if (MechZ(wounded) > MechElevation(wounded)) {
    if (MechJumpSpeed(wounded) >= MP1) {
      mech_notify(
          wounded, MECHALL,
          "You initiate your jumpjets to compensate for the breached cocoon!");
      MechCocoon(wounded) = -1;
      return;
    }
    mech_notify(wounded, MECHALL,
                "Your cocoon has been destroyed - have a nice fall!");
    mech_los_broadcast(
        wounded,
        "starts plummeting down, as the final blast blows the cocoon apart!");
    mech_event_cancel(wounded, EVENT_OOD);
    mech_event_schedule(wounded, EVENT_FALL, mech_fall_event, FALL_TICK, -1);
  }
}

void mech_ood_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int mof = 0, roll, roll_needed, para = 0;

  if (!OODing(mech))
    return;
  MarkForLOSUpdate(mech);
  if ((MechsElevation(mech) - DropGetElevation(mech)) > OOD_SPEED) {
    MechZ(mech) -= OOD_SPEED;
    MechFZ(mech) = MechZ(mech) * ZSCALE;
    mech_event_schedule(mech, EVENT_OOD, mech_ood_event, OOD_TICK, 0);
    return;
  }
  /* Time to hit da ground */
  mech_notify(mech, MECHALL, "Your unit touches down!");

  notify_event(btech_context_evaluation(mech->xcode.context), NULL, mech->mynum,
               mech->mynum, mech->mynum, LUA_EVENT_OOD_LAND, (char **)NULL, 0);

  if (MechStatus(mech) & COMBAT_SAFE) {
    /* If we're combat safe, we land regardless, since we're not gonna take any
     * damage */
    MechCocoon(mech) = 0;
    mech_los_broadcast(mech, "touches down safely!");
    DropSetElevation(mech, 1);
    mech_maybe_move(mech);
    return;
  }

  if (Fallen(mech))
    mof = -10;
  if (Uncon(mech) || !Started(mech) || Blinded(mech))
    mof = -20;
  roll = btech_random_roll(mech->xcode.context);
  roll_needed = MechType(mech) == CLASS_BSUIT || MechType(mech) == CLASS_MW
                    ? FindPilotPiloting(mech) - 1
                    : FindSPilotPiloting(mech) + MechPilotSkillBase(mech);

  if (!Started(mech))
    roll_needed += 10;
  if (MechCocoon(mech) == 1) {
    para = 1;
  } else if (MechCocoon(mech) < 0) {
    roll_needed += 4;
  } else if (MechCocoon(mech) == 0) {
    roll_needed += 10;
  }

  if (mech_real_terrain_get(mech) != GRASSLAND &&
      mech_real_terrain_get(mech) != ROAD) {
    if (mech_real_terrain_get(mech) == WATER ||
        mech_real_terrain_get(mech) == HIGHWATER)
      roll_needed += 2;
    else
      roll_needed += 3;
  }

  MechCocoon(mech) = 0;

  if (is_in_character(mech->xcode.context->database, mech->mynum) &&
      game_object_location(mech->xcode.context->database, MechPilot(mech)) !=
          mech->mynum)
    roll_needed += 99;

  mech_notify(mech, MECHPILOT, "You make a piloting skill roll!");

  mech_notify(
      mech, MECHPILOT,

      tprintf("Modified Pilot Skill: BTH %d\tRoll: %d", roll_needed, roll));

  mof += (roll - roll_needed);

  if (roll >= roll_needed) {

    if (roll_needed > 2)

      AccumulatePilXP(MechPilot(mech), mech, BOUNDED(1, (abs(mof) + 1) * 2, 20),
                      1);
  }

  mof += (roll - roll_needed);

  if (mof < 0) {

    if (MechType(mech) == CLASS_MECH) {

      mech_notify(
          mech, MECHALL,

          "You are unable to control your momentum and fall on your face!");

      mech_los_broadcast(mech,

                         "touches down on the ground, twists, and falls down!");

      MechFalls(mech, (abs(mof) * (para ? 1 : 2)), 1);

    } else if (MechType(mech) == CLASS_BSUIT) {

      int i, ii, dam;

      mech_notify(mech, MECHALL,

                  "You are unable to control your momentum and crash!");

      mech_los_broadcast(mech, "crashes to the ground!");

      for (i = 0; i < NUM_SECTIONS; i++) {

        dam = 0;

        if (GetSectOInt(mech, i) > 0) {

          for (ii = mof; ii < 0; ii++)

            dam += btech_random_range(mech->xcode.context, 1, 4);

          DamageMech(mech, mech, 0, -1, i, 0, 0, dam, -1, -1, 0, 0, 0, 0);

          MechFloods(mech);
        }
      }

      MechFalls(mech, 0, 1);

    } else {

      mech_notify(mech, MECHALL,

                  "You are unable to control your momentum and crash!");

      mech_los_broadcast(mech, "crashes at the ground!");

      MechFalls(mech, (abs(mof) * (para ? 1 : 3)), 1);
    }

  } else if (!para) {

    mech_los_broadcast(mech, "touches down!");

  } else if (para) {

    mech_los_broadcast(mech, "touches down and rolls on the ground!");

    /*	mech_notify(mech, MECHALL,
                "As you hit the ground you roll and sponge some damage!");
            MechFalls(mech, (MechTons(mech) / 25), 0); */
  }

  DropSetElevation(mech, 1);

  if (!Fallen(mech))

    domino_space(mech, 2);

  if (WaterBeast(mech) && NotInWater(mech))

    MechDesiredSpeed(mech) = 0.0;

  mech_maybe_move(mech);

  /* Lets handle dropping right into the water. Anything but a mech/hover goes
   * glub */
  if (InWater(mech) &&
      (MechType(mech) == CLASS_VEH_GROUND || MechType(mech) == CLASS_VTOL ||
       MechType(mech) == CLASS_BSUIT || MechType(mech) == CLASS_AERO ||
       MechType(mech) == CLASS_DS) &&
      !(MechSpecials2(mech) & WATERPROOF_TECH)) {

    mech_notify(mech, MECHALL,
                "Water floods your engine and your unit "
                "becomes unoperable.");
    if (MechType(mech) == CLASS_BSUIT)
      mech_los_broadcast(mech,
                         "emits some bubbles and flails their arms around "
                         "as they sink to the bottom!");
    else
      mech_los_broadcast(mech,
                         "emits some bubbles as its engines are flooded.");
    DestroyMech(mech, mech, 0, KILL_TYPE_FLOOD);
  }
}

void initiate_ood(DbRef player, Mech *mech, char *buffer) {
  char *args[4];
  int x, y, z = ORBIT_Z, argc;

  DOCHECK_CONTEXT(mech->xcode.context,
                  (argc = mech_parseattributes(buffer, args, 3)) < 2,
                  "Invalid attributes!");
  DOCHECK_CONTEXT(mech->xcode.context, Readnum(x, args[0]),
                  "Invalid number! (x)");
  DOCHECK_CONTEXT(mech->xcode.context, Readnum(y, args[1]),
                  "Invalid number! (y)");
  if (argc == 3)
    DOCHECK_CONTEXT(mech->xcode.context, Readnum(z, args[2]),
                    "Invalid number! (z)");
  DOCHECK_CONTEXT(mech->xcode.context, OODing(mech),
                  "OOD already in progress!");
  mech_Rsetxy(GOD, (void *)mech, tprintf("%d %d", x, y));
  DOCHECK_CONTEXT(mech->xcode.context, MechX(mech) != x || MechY(mech) != y,
                  "Invalid co-ordinates!");
  DOCHECK_CONTEXT(mech->xcode.context, Fallen(mech),
                  "You'll have to get up first.");
  DOCHECK_CONTEXT(mech->xcode.context, Digging(mech),
                  "You're too busy digging in.");
  MechZ(mech) = z;
  MechFZ(mech) = ZSCALE * MechZ(mech);
  MarkForLOSUpdate(mech);
  notify(btech_context_evaluation(mech->xcode.context), player,
         "OOD initiated.");
  if (Evading(mech)) {
    MechStatus2(mech) &= ~EVADING;
  }

  if (Sprinting(mech)) {
    MechStatus2(mech) &= ~SPRINTING;
  }

  if (FlyingT(mech)) {
    MechStatus(mech) &= ~LANDED;
    MechDesiredSpeed(mech) = MechMaxSpeed(mech) / 2;
    if (is_aero(mech))
      MechDesiredAngle(mech) = 0;
    mech_maybe_move(mech);
  } else {
    MechCocoon(mech) = MechRTons(mech) / 5 / 1024 + 1;
    mech_event_cancel(mech, EVENT_MOVE);
    mech_event_schedule(mech, EVENT_OOD, mech_ood_event, OOD_TICK, 0);
  }
}
