#include "mech_physical_internal.h"
int checkGrabClubLocation(Mech *mech, int section, int emit) {
  int tCanGrab = 1;
  char buf[100] = {0};
  char location[20] = {0};

  ArmorStringFromIndex(section, location, MechType(mech), MechMove(mech));

  if (SectIsDestroyed(mech, section)) {
    snprintf(buf, sizeof(buf), "Your %s is destroyed.", location);
    tCanGrab = 0;
  } else if (!OkayCritSectS(section, 3, HAND_OR_FOOT_ACTUATOR)) {
    snprintf(buf, sizeof(buf),
             "Your %s's hand actuator is destroyed or missing.", location);
    tCanGrab = 0;
  } else if (!OkayCritSectS(section, 0, SHOULDER_OR_HIP)) {
    snprintf(buf, sizeof(buf),
             "Your %s's shoulder actuator is destroyed or missing.", location);
    tCanGrab = 0;
  } else if (SectHasBusyWeap(mech, section)) {
    snprintf(buf, sizeof(buf),
             "Your %s is still recovering from it's last attack.", location);
    tCanGrab = 0;
  }

  if (!tCanGrab && emit)
    mech_notify(mech, MECHALL, buf);

  return tCanGrab;
} // end checkGrabClubLocation()

/*
 * Handles the grabbing of a club.
 */
void mech_grabclub(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  int wcArgs = 0;
  int location = 0;
  char *args[1];
  char locname[20];

  cch(MECH_USUALO);

  wcArgs = mech_parseattributes(buffer, args, 1);

  // If we grabclub -, we're attempting to drop it.
  if (wcArgs >= 1 && toupper(args[0][0]) == '-') {
    if ((MechSections(mech)[LARM].specials & CARRYING_CLUB) ||
        (MechSections(mech)[RARM].specials & CARRYING_CLUB)) {
      mech_drop_club(mech);
    } else {
      mech_notify(mech, MECHALL, "You aren't currently carrying a club.");
    }
    return;
  } // end if() - Check to drop club.

  DOCHECKMA(MechIsQuad(mech), "Quads can't carry a club.");
  DOCHECKMA(Fallen(mech),
            "You can't grab a club while lying flat on your face.");
  DOCHECKMA(Jumping(mech), "You can't grab a club while jumping!");
  DOCHECKMA(OODing(mech), "Your rapid descent prevents that.");
  DOCHECKMA(mech_event_count(mech, EVENT_UNJAM_AMMO),
            "You are too busy unjamming a weapon!");
  DOCHECKMA(mech_event_count(mech, EVENT_REMOVE_PODS),
            "You are too busy removing iNARC pods!");

  // If they already have a physical weapon, disallow the grabbing of a club.
  DOCHECKMA(have_axe(mech, LARM) || have_axe(mech, RARM),
            "You can not grab a club if you carry an axe.");
  DOCHECKMA(have_sword(mech, LARM) || have_sword(mech, RARM),
            "You can not grab a club if you carry a sword.");
  DOCHECKMA(have_mace(mech, LARM) || have_mace(mech, RARM),
            "You can not grab a club if you carry an mace.");

  if (wcArgs == 0) {
    if (checkGrabClubLocation(mech, LARM, 0))
      location = LARM;
    else if (checkGrabClubLocation(mech, RARM, 0))
      location = RARM;
    else {
      mech_notify(mech, MECHALL,
                  "You don't have a free arm with a working hand actuator!");
      return;
    }
  } else {

    // Figure out which arm to use.
    switch (toupper(args[0][0])) {
    case 'R':
      location = RARM;
      break;
    case 'L':
      location = LARM;
      break;
    default:
      mech_notify(mech, MECHALL, "Invalid option for 'grabclub'");
      return;
    } // end switch() - Determine location.

    // see if we have actuators and a working arm.
    if (!checkGrabClubLocation(mech, location, 1))
      return;
  }

  DOCHECKMA(CarryingClub(mech), "You're already carrying a club.");
  DOCHECKMA(mech_real_terrain_get(mech) != HEAVY_FOREST &&
                mech_real_terrain_get(mech) != LIGHT_FOREST,
            "There don't appear to be any trees within grabbing distance.");

  ArmorStringFromIndex(location, locname, MechType(mech), MechMove(mech));

  mech_los_broadcast(mech, "reaches down and yanks a tree out of the ground!");
  mech_printf(mech, MECHALL,
              "You reach down and yank a tree out of the ground with your %s.",
              locname);

  // Grabbing a club sets a flag and recycles the arm used.
  MechSections(mech)[location].specials |= CARRYING_CLUB;
  mech_set_recycle_limb(mech, location, PHYSICAL_RECYCLE_TIME);
} // end mech_grabclub()
