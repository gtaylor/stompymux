#include "mech_scan_internal.h"

void PrintEnemyWeaponStatus(Mech *mech, DbRef player) {
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  int count;
  int loop;
  int ii;
  char weapbuff[LBUF_SIZE] = {0};
  char tempbuff[50] = {0};
  char location[20] = {0};
  int running_sum = 0;

  recycle_weaponry(mech);
  notify(evaluation, player, "================WEAPON SYSTEMS================");
  if (MechType(mech) == CLASS_BSUIT)
    notify(evaluation, player,
           "----- Weapon ------ [##]  Holder ------ Status");
  else
    notify(evaluation, player,
           "----- Weapon ------ [##]  Location ---- Status");
  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    if (SectIsDestroyed(mech, loop))
      continue;
    count = FindWeapons(mech, loop, weaparray, weapdata, critical);
    if (count > 0) {
      ArmorStringFromIndex(loop, tempbuff, MechType(mech), MechMove(mech));
      snprintf(location, sizeof(location), "%-14.14s", tempbuff);

      for (ii = 0; ii < count; ii++) {
        snprintf(weapbuff, sizeof(weapbuff), " %-18.18s [%2d]  ",
                 &MechWeapons[weaparray[ii]].name[3], running_sum + ii);
        strcat(weapbuff, location);

        if (PartIsNonfunctional(mech, loop, critical[ii])) {
          strcat(weapbuff, "[fg=black bold]*****[reset]");
        } else {
          if (weapdata[ii]) {
            strcat(weapbuff, "-----");
          } else {
            strcat(weapbuff, "[fg=green]Ready[reset]");
          }
        }
        notify(evaluation, player, weapbuff);
      }
      running_sum += count;
    }
  }
}

void mech_sight(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  BattleMap *mech_map;
  char *args[5];
  int argc;
  int weapnum;

  mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  cch(MECH_USUAL);
  argc = mech_parseattributes(buffer, args, 5);
  if (argc >= 1) {
    weapnum = atoi(args[0]);
    FireWeaponNumber(player, mech, mech_map, weapnum, argc, args, 1);
  } else {
    notify(evaluation, player, "Not enough arguments to the function");
  }
}

void mech_view(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  int targetnum;
  char targetID[5];
  char *args[5];
  int argc;
  char *target_desc;

  cch(MECH_USUAL);
  argc = mech_parseattributes(buffer, args, 2);
  if (argc == 0) { /* default target */
    if (MechTarget(mech) == -1) {
      mech_notify(mech, MECHALL, "You do not have a default target set!");
      return;
    }
    target = btech_context_get_mech(mech->xcode.context, MechTarget(mech));
    if (!target) {
      mech_notify(mech, MECHALL, "Invalid default target!");
      MechTarget(mech) = -1;
      return;
    }
    DOCHECK_CONTEXT(
        mech->xcode.context,
        !InLineOfSight_NB(mech, target, MechX(target), MechY(target),
                          FaMechRange(mech, target)),
        "That target isn't seen well enough by the scannfers for viewing!");
    if (*(target_desc = btech_attribute_read(target->xcode.context->database,
                                             target->mynum, A_MECHDESC,
                                             (char[LBUF_SIZE]){0})))
      notify(evaluation, player, target_desc);
    else
      notify(evaluation, player, "That target has no markings.");
  } else if (argc == 1) { /* ID number */
    targetID[0] = args[0][0];
    targetID[1] = args[0][1];
    targetnum = FindTargetDBREFFromMapNumber(mech, targetID);
    if (targetnum == -1) {
      mech_notify(mech, MECHPILOT, "Target is not in line of sight!");
      return;
    }
    target = btech_context_get_mech(mech->xcode.context, targetnum);

    if (!target || !InLineOfSight(mech, target, MechX(target), MechY(target),
                                  FaMechRange(mech, target))) {
      mech_notify(mech, MECHPILOT, "Target is not in line of sight!");
      return;
    }

    DOCHECK_CONTEXT(
        mech->xcode.context,
        !InLineOfSight_NB(mech, target, MechX(target), MechY(target),
                          FaMechRange(mech, target)),
        "That target isn't seen well enough by the scanners for viewing!");

    if (*(target_desc = btech_attribute_read(target->xcode.context->database,
                                             target->mynum, A_MECHDESC,
                                             (char[LBUF_SIZE]){0})))
      notify(evaluation, player, target_desc);
    else
      notify(evaluation, player, "That target has no markings.");
  } else
    notify(evaluation, player, "Invalid number of arguments to function.");
}
