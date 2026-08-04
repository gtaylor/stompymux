/*
 * $Id: autogun.c,v 1.5 2005/08/03 21:40:54 av1-op Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Sun Nov 17 13:23:20 1996 fingon
 * Last modified: Sun Jun 14 16:29:44 1998 fingon
 *
 */

#include "autopilot_autogun_internal.h"

bool autogun_chase_target(Autopilot *autopilot, Mech *mech, BattleMap *map,
                          Mech *target) {
  char buffer[LBUF_SIZE];
  short x, y;
  float fx, fy;
  char do_chasetarget;

  /* Setup chasetarg
   *
   * Since chasetarget uses follow but we don't want follow getting
   * messed up if its following a normal target.  We define our own
   * follow (basicly its follow but with the command name chasetarget */

  /* Get the first command, if its chasetarget or there is no command
   * check to see if we need to update chasetarget, otherwise means
   * AI is doing something else important so don't try to chase
   * anything */

  if (ChasingTarget(autopilot)) {

    /* Reset the flag */
    do_chasetarget = 0;

    /* Get the first command's enum and check it */
    switch (auto_get_command_enum(autopilot, 1)) {

    case GOAL_CHASETARGET:

      /* Ok its our command so we can change it */
      do_chasetarget = 1;
      break;

    case -1:

      /* No current commands so we can do our chasetarget */
      do_chasetarget = 1;
      break;

      /* ALl the other stuff we don't want to mess with */
    default:

      /* Reset the chase values */
      autopilot->chase_target = -10;
      autopilot->chasetarg_update_tick = AUTOPILOT_CHASETARG_UPDATE_TICK;
      autopilot->follow_update_tick = AUTOPILOT_FOLLOW_UPDATE_TICK;
      break;
    }

    /* Check the flag */
    if (do_chasetarget) {

      /* Ok lets chase the guy */

      /* First see if we need to update */
      if ((autopilot->target != autopilot->chase_target) ||
          (autopilot->chasetarg_update_tick >=
           AUTOPILOT_CHASETARG_UPDATE_TICK)) {

        /* Tell the AI to follow its target */
        /* Basicly remove all the commands, add in
         * autogun and follow and engage */

        /* Let the AI know we chasing this guy */
        autopilot->chase_target = autopilot->target;

        /* Reset the tickers */
        autopilot->chasetarg_update_tick = 0;
        autopilot->follow_update_tick = AUTOPILOT_FOLLOW_UPDATE_TICK;

        /* Reset the AI */
        auto_disengage(autopilot->mynum, autopilot, "");
        auto_delcommand(autopilot->mynum, autopilot, "-1");

        /* Add in autogun and follow and engage */
        if (AssignedTarget(autopilot) && autopilot->target != -1) {
          snprintf(buffer, LBUF_SIZE, "autogun target %ld", autopilot->target);
        } else {
          snprintf(buffer, LBUF_SIZE, "autogun on");
        }

        auto_addcommand(autopilot->mynum, autopilot, buffer);
        snprintf(buffer, LBUF_SIZE, "chasetarget %ld", autopilot->target);
        auto_addcommand(autopilot->mynum, autopilot, buffer);
        auto_engage(autopilot->mynum, autopilot, "");

        /* Log it */
        print_autogun_log(autopilot, "Autogun Event Finished");

        return true;

      } else {

        /* Update the ticker */
        autopilot->chasetarg_update_tick++;

        /* Check to see if we need to turn to face the guy by
         * generating our target hex and seeing if we are in that
         * hex then face the bad guy */
        if ((target = btech_context_get_mech(mech->xcode.context,
                                             autopilot->target)) &&
            (!Destroyed(target) && (target->mapindex == mech->mapindex))) {

          /* Generate the target hex */
          /*! \todo {Instead of calcing this all the time, possibly add
           * variables to the AI to remember it} */
          FindXY(MechFX(target), MechFY(target),
                 MechFacing(target) + autopilot->ofsx, autopilot->ofsy, &fx,
                 &fy);

          RealCoordToMapCoord(&x, &y, fx, fy);

          /* Make sure the hex is sane */
          if (x < 0 || y < 0 || x >= map->map_width || y >= map->map_height) {

            /* Bad Target Hex */

            /* Reset the hex to the Target's current hex */
            x = MechX(target);
            y = MechY(target);
          }

          /* Are we in the target hex and is the target not moving */
          if ((MechX(mech) == x) && (MechY(mech) == y) &&
              (MechSpeed(target) < 0.5)) {

            /* Get his bearing and face him */
            MapCoordToRealCoord(x, y, &fx, &fy);

            /* If we're not facing him, turn towards him */
            if (MechDesiredFacing(mech) !=
                FindBearing(MechFX(mech), MechFY(mech), fx, fy)) {

              snprintf(buffer, LBUF_SIZE, "%d",
                       FindBearing(MechFX(mech), MechFY(mech), fx, fy));
              mech_heading(autopilot->mynum, mech, buffer);
            }
            /* Turn towards him */
          }
          /* Is he moving and are we in target hex */
        }
        /* Do we need to turn towards him */
      } /* Do we need to update */
    }
    /* Do we run chasetarget */
  }

  /* Is chasetarget on */
  /* Make sure multiple instances of autogun aren't running */

  /* Log it */
  print_autogun_log(autopilot, "Autogun Event Finished");

  /* The End */
  return false;
}
