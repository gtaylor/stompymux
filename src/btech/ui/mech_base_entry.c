#include "mech_maps_internal.h"

static void mech_enter_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data, *tmpm = NULL;
  MapObject *mapo;
  BattleMap *map = btech_context_get_map(mech->xcode.context, mech->mapindex),
            *newmap;
  long target = (long)e->data2;
  int x, y;
  int obj_x, obj_y;
  LuaLockInvocation lock;
  LuaLockResult lock_result;

  if (!(mapo = find_entrance_by_xy(map, MechX(mech), MechY(mech))))
    return;
  if (!Started(mech) || Uncon(mech) || Jumping(mech) ||
      (MechType(mech) == CLASS_MECH &&
       (Fallen(mech) || mech_event_count(mech, EVENT_STAND))) ||
      OODing(mech) ||
      (fabs(MechSpeed(mech)) * 5 >= MMaxSpeed(mech) &&
       fabs(MMaxSpeed(mech)) >= MP1) ||
      (MechType(mech) == CLASS_VTOL && AeroFuel(mech) <= 0))
    return;
  if (!(newmap = btech_context_get_map(mech->xcode.context, mapo->obj)))
    return;
  if (!find_entrance(newmap, target, &x, &y))
    return;

  if (!lock_test(btech_context_evaluation(mech->xcode.context), mech->mynum,
                 mech->mynum, mech->mynum, newmap->mynum, LUA_LOCK_ENTER,
                 LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock, &lock_result) &&
      (BuildIsSafe(newmap) || newmap->cf >= (newmap->cfmax / 2))) {
    char *msg = lock_result.has_enactor_message ? lock_result.enactor_message
                                                : "The hangar is locked.";
    mech_notify(mech, MECHALL, msg);
    return;
  }

  StopBSuitSwarmers(
      btech_context_find_object(mech->xcode.context, mech->mapindex), mech, 1);
  mech_printf(mech, MECHALL, "You enter %s.",
              structure_name(mech->xcode.context->database, mapo).text);
  MechLOSBroadcast(
      mech, tprintf("has entered %s at %d,%d.",
                    structure_name(mech->xcode.context->database, mapo).text,
                    MechX(mech), MechY(mech)));
  MarkForLOSUpdate(mech);
  if (MechType(mech) == CLASS_MW &&
      !is_in_character(mech->xcode.context->database, mapo->obj)) {
    enter_mw_bay(mech, mapo->obj);
    return;
  }
  if (MechCarrying(mech) > 0)
    tmpm = btech_context_get_mech(mech->xcode.context, MechCarrying(mech));
  obj_x = MechX(mech);
  obj_y = MechY(mech);
  mech_Rsetmapindex(GOD, (void *)mech, tprintf("%d", (int)mapo->obj));
  mech_Rsetxy(GOD, (void *)mech, tprintf("%d %d", x, y));
  MechLOSBroadcast(
      mech, tprintf("has entered %s at %d,%d.",
                    structure_name(mech->xcode.context->database, mapo).text,
                    obj_x, obj_y));
  if (tmpm)
    MechLOSBroadcast(
        tmpm, tprintf("has entered %s at %d,%d.",
                      structure_name(mech->xcode.context->database, mapo).text,
                      obj_x, obj_y));
  move_via_teleport(btech_context_evaluation(mech->xcode.context), mech->mynum,
                    mapo->obj, 1, 0);
  if (tmpm) {
    mech_Rsetmapindex(GOD, (void *)tmpm, tprintf("%d", (int)mapo->obj));
    mech_Rsetxy(GOD, (void *)tmpm, tprintf("%d %d", x, y));
    move_via_teleport(btech_context_evaluation(mech->xcode.context),
                      tmpm->mynum, mapo->obj, 1, 0);
  }
  auto_cal_mapindex(mech->xcode.context, mech);
}

void mech_enterbase(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *map, *newmap;
  int x, y;
  MapObject *mapo;
  char target, *tmpc;
  char *args[2];
  int argc;

  char fail_mesg[SBUF_SIZE];
  LuaLockInvocation lock;
  LuaLockResult lock_result;

  argc = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(mech->xcode.context, argc > 1,
                  "Invalid arguments to command!");
  tmpc = args[0];
  if (argc > 0 && *tmpc && !(*(tmpc + 1)))
    target = tolower(*tmpc);
  else
    target = 0;
  cch(MECH_USUAL);
  map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  /* For now, no dir checks */
  DOCHECK_CONTEXT(mech->xcode.context, Jumping(mech),
                  "While in mid-jump? No way.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) == CLASS_MECH &&
                      (Fallen(mech) || mech_event_count(mech, EVENT_STAND)),
                  "Crawl inside? I think not. Stand first.");
  DOCHECK_CONTEXT(mech->xcode.context, OODing(mech),
                  "While in mid-flight? No way.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) == CLASS_VTOL && AeroFuel(mech) <= 0,
                  "You lack fuel to maneuver in!");
  DOCHECK_CONTEXT(mech->xcode.context, FlyingT(mech) && !Landed(mech),
                  "You need to land before you can enter the hangar.");
  DOCHECK_CONTEXT(
      mech->xcode.context, IsDS(mech),
      "Heh, you're trying to be funny, right, a DropShip entering hangar?");
  DOCHECK_CONTEXT(mech->xcode.context,
                  fabs(MechSpeed(mech)) * 5 >= MMaxSpeed(mech) &&
                      fabs(MMaxSpeed(mech)) >= MP1,
                  "You are moving too fast to enter the hangar!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  !(mapo = find_entrance_by_xy(map, MechX(mech), MechY(mech))),
                  "You see nothing to enter here!");
  /* Wow, *gasp*, we got something to enter */
  if (!(newmap = btech_context_find_object(mech->xcode.context, mapo->obj))) {
    mech_notify(mech, MECHALL, "You sense wrongness in fabric of space..");
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Error: No map existing for mapindex #%d (@ %d,%d of #%ld)",
                (int)mapo->obj, mapo->x, mapo->y, mech->mapindex));
    return;
  }
  if (!find_entrance(newmap, target, &x, &y)) {
    mech_notify(mech, MECHALL, "You sense wrongness in fabric of space..");
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf(
            "Error: No entrance existing for mapindex #%d (@ %d,%d of #%ld)",
            (int)mapo->obj, mapo->x, mapo->y, mech->mapindex));
    return;
  }

  if (!lock_test(btech_context_evaluation(mech->xcode.context), player, player,
                 mech->mynum, newmap->mynum, LUA_LOCK_ENTER,
                 LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock, &lock_result) &&
      (BuildIsSafe(newmap) || newmap->cf >= (newmap->cfmax / 2))) {

    /* Trigger FAIL & AFAIL */
    memset(fail_mesg, 0, sizeof(fail_mesg));
    snprintf(fail_mesg, SBUF_SIZE, "The hangar is locked.");

    notify_lock_failure(btech_context_evaluation(mech->xcode.context), &lock,
                        &lock_result, fail_mesg, NULL, LUA_EVENT_FAIL);

    return;
  }

  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_event_count(mech, EVENT_ENTER_HANGAR),
                  "You are already entering the hangar!");
  /* XXX Check for other mechs in the hex possibly doing this as well (ick) */
  HexLOSBroadcast(map, MechX(mech), MechY(mech),
                  "The doors at $h start to open..");
  mech_event_schedule(mech, EVENT_ENTER_HANGAR, mech_enter_event, 18,
                      (long)target);
}
