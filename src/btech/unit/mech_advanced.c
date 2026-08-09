#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "equipment_types.h"
#include "failures.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_advanced_api.h"
#include "mech_build_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_events_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "random.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static int mech_disableweap_func(Mech *mech, DbRef player, int index, int high,
                                 void *context) {
  (void)context;
  int section, critical, weaptype;

  weaptype =
      FindWeaponNumberOnMech_Advanced(mech, index, &section, &critical, 1);
  if (weaptype == -1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "The weapons system chirps: 'Illegal Weapon Number!'");
    return 0;
  }
  if (weaptype == -2) {
    mecha_notify(
        btech_context_evaluation(mech->xcode.context), player,
        "The weapons system chirps: 'That Weapon has been destroyed!'");
    return 0;
  }
  weaptype = weapon_from_equipment_index(
      mech_critical_part_type(mech, section, critical));
  if (!weapon_catalogue_has_special(weaptype, GAUSS)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "You can only disable Gauss weapons.");
    return 0;
  }
  if (mech_weapon_is_recycling_at(mech, section, critical)) {
    mecha_notify(
        btech_context_evaluation(mech->xcode.context), player,
        "The weapon system chirps: 'That weapon is still recharging!'");
    return 0;
  }

  mech_critical_temporary_failure_set(mech, section, critical, FAIL_DESTROYED);
  mech_printf(mech, MECHALL, "You power down weapon %d.", index);
  return 0;
}

void mech_disableweap(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[1];

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Please specify a weapon number.");
    return;
  }

  multi_weap_sel(mech, player, args[0], 1, mech_disableweap_func, nullptr);
}

int FindMainWeapon(Mech *mech, int (*callback)(Mech *, int, int, int, int)) {
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  int count;
  int loop;
  int ii;
  int tempcrit;
  int maxcrit = 0;
  int maxloc = 0;
  int critfound = 0;
  int maxcount = 0;

  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    if (mech_section_is_destroyed(mech, loop))
      continue;
    count = FindWeapons_Advanced(mech, loop, weaparray, weapdata, critical, 1);
    if (count > 0) {
      for (ii = 0; ii < count; ii++) {
        const int critical_index = *(const int *)checked_storage_at_const(
            critical, MAX_WEAPS_SECTION, sizeof(*critical), (size_t)ii);
        if (!mech_critical_is_broken(mech, loop, critical_index)) {
          /* tempcrit = GetWeaponCrits(mech, weaparray[ii]); */
          tempcrit = (int)btech_random_i31(&mech->xcode.context->random);
          if (tempcrit > maxcrit) {
            critfound = 1;
            maxcrit = tempcrit;
            maxloc = loop;
            maxcount = ii;
          }
        }
      }
    }
  }
  if (critfound) {
    const unsigned char weapon =
        *(const unsigned char *)checked_storage_at_const(
            weaparray, MAX_WEAPS_SECTION, sizeof(*weaparray), (size_t)maxcount);
    return callback(mech, maxloc, weapon, maxcount, maxcrit);
  } else
    return 0;
}

void mech_auto_turret(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALSO))
    return;

  if (!mech_section_internal(mech, TURRET)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "You have no turret to autoturn!");
    return;
  }

  mech_printf(mech, MECHALL, "Automatic turret turning is now %s",
              (((mech)->rd.status2) & AUTOTURN_TURRET) ? "OFF" : "ON");

  if (((mech)->rd.status2) & AUTOTURN_TURRET)
    ((mech)->rd.status2) &= ~AUTOTURN_TURRET;
  else
    ((mech)->rd.status2) |= AUTOTURN_TURRET;
}

void mech_usebin(DbRef player, Mech *mech, char *buffer) {
  char strLocation[80];
  int wLoc, wCurLoc;
  int wSection, wCritSlot, wWeapNum, wWeapType;
  char *args[2];

  if (!common_checks(player, mech, MECH_USUALSO))
    return;

  if (mech_parseattributes(buffer, args, 2) != 2) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }

  if (!parse_int_checked(args[0], &wWeapNum)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 tprintf("Invalid value: %s", args[0]));
    return;
  }
  wWeapType = FindWeaponNumberOnMech(mech, wWeapNum, &wSection, &wCritSlot);

  if (wWeapType == -1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "The weapons system chirps: 'Illegal Weapon Number!'");
    return;
  }
  if (wWeapType == -2) {
    mecha_notify(
        btech_context_evaluation(mech->xcode.context), player,
        "The weapons system chirps: 'That Weapon has been destroyed!'");
    return;
  }
  if (wWeapType == -3) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "The weapon system chirps: 'That weapon is still reloading!'");
    return;
  }
  if (wWeapType == -4) {
    mecha_notify(
        btech_context_evaluation(mech->xcode.context), player,
        "The weapon system chirps: 'That weapon is still recharging!'");
    return;
  }
  if (weapon_catalogue_is_energy(wWeapType)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Energy weapons do not use ammo!");
    return;
  }

  if (args[1][0] == '-') {
    mech_printf(mech, MECHALL, "Prefered ammo source reset for weapon #%d",
                wWeapNum);
    mech_critical_desired_ammo_section_set(mech, wSection, wCritSlot, -1);
    return;
  }

  wLoc = ArmorSectionFromString(((mech)->ud.type), ((mech)->ud.move), args[1]);

  if (wLoc == -1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid section!");
    return;
  }
  if (!mech_section_original_internal(mech, wLoc)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid section!");
    return;
  }
  if (!mech_section_internal(mech, wLoc)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "That section is destroyed!");
    return;
  }

  ArmorStringFromIndex(wLoc, strLocation, ((mech)->ud.type), ((mech)->ud.move));
  wCurLoc = mech_critical_desired_ammo_section(mech, wSection, wCritSlot);

  if (wCurLoc == wLoc) {
    mecha_notify(
        btech_context_evaluation(mech->xcode.context), player,
        tprintf("Prefered ammo source already set to %s for weapon #%d",
                strLocation, wWeapNum));
    return;
  }

  mech_printf(mech, MECHALL, "Prefered ammo source set to %s for weapon #%d",
              strLocation, wWeapNum);
  mech_critical_desired_ammo_section_set(mech, wSection, wCritSlot, wLoc);
}

void mech_safety(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  if (((mech)->ud.type) == CLASS_MW) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Your weapons dont have safeties.");
    return;
  }
  if (buffer && !strcasecmp(buffer, "on")) {
    mech_player_killer_set(mech, false);
    mech_notify(mech, MECHALL, "Safeties flipped [fg=green bold]ON[reset].");
    return;
  }
  if (buffer && !strcasecmp(buffer, "off")) {
    mech_player_killer_set(mech, true);
    mech_notify(mech, MECHALL, "Safeties flipped [fg=red bold]OFF[reset].");
    return;
  }

  mech_printf(mech, MECHPILOT, "Weapon safeties are [bold]%s[reset]",
              mech_condition_summary(mech).player_killer ? "[fg=red]OFF"
                                                         : "[fg=green]ON");
  return;
}

#define MECHPREF_FLAG_INVERTED 0x01
#define MECHPREF_FLAG_NEGATIVE 0x02

static struct mechpref_info {
  int bit;
  unsigned char flags;
  const char *name;
  const char *msg;
} mech_preferences[] = {
    {MECHPREF_PKILL, MECHPREF_FLAG_INVERTED, "MWSafety",
     "MechWarrior Safeties flipped"},
    {MECHPREF_SLWARN, 0, "SLWarn",
     "The warning when lit by searchlight is now"},
    {MECHPREF_AUTOFALL, MECHPREF_FLAG_NEGATIVE, "AutoFall",
     "Suicidal jumps off cliffs toggled"},
    {MECHPREF_NOARMORWARN, MECHPREF_FLAG_INVERTED, "ArmorWarn",
     "Low-armor warnings turned"},
    {MECHPREF_NOAMMOWARN, MECHPREF_FLAG_INVERTED, "AmmoWarn",
     "Warning when running out of Ammunition switched"},
    {MECHPREF_AUTOCON_SD, MECHPREF_FLAG_NEGATIVE, "AutoconShutdown",
     "Autocon on shutdown units turned"},
    {MECHPREF_NOFRIENDLYFIRE, 0, "FFSafety", "Friendly Fire Safeties flipped"},
    {MECHPREF_BTHDEBUG, MECHPREF_FLAG_NEGATIVE, "BTHDebug",
     "BTH Debugging is now"}

};
#define NUM_MECHPREFERENCES                                                    \
  (sizeof(mech_preferences) / sizeof(struct mechpref_info))

static struct mechpref_info mech_preference(size_t index) {
  return *(const struct mechpref_info *)checked_storage_at_const(
      mech_preferences, NUM_MECHPREFERENCES, sizeof(*mech_preferences), index);
}

static char *display_mechpref(void *context, int i,
                              char buffer[static LBUF_SIZE]) {
  Mech *mech = context;
  struct mechpref_info info = mech_preference((size_t)i);
  const char *state;

  if (((((mech)->rd.mech_prefs) & info.bit) &&
       (info.flags & MECHPREF_FLAG_INVERTED)) ||
      (!(((mech)->rd.mech_prefs) & info.bit) &&
       !(info.flags & MECHPREF_FLAG_INVERTED))) {
    if (info.flags & MECHPREF_FLAG_NEGATIVE)
      state = "[fg=green bold]OFF[reset]";
    else
      state = "[fg=red bold]OFF[reset]";
  } else {
    if (info.flags & MECHPREF_FLAG_NEGATIVE)
      state = "[fg=red bold]ON[reset]";
    else
      state = "[fg=green bold]ON[reset]";
  }

  snprintf(buffer, LBUF_SIZE, "        %-40s%s", info.name, state);
  return buffer;
}

void mech_mechprefs(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  int nargs;
  char *args[3];
  char buf[LBUF_SIZE];
  CoolMenu *c;

  if (!common_checks(player, mech, MECH_USUALSMO))
    return;
  nargs = mech_parseattributes(buffer, args, 2);

  /* Default, no arguments passed */
  if (!nargs) {

    /* Show mechprefs */
    c = SelCol_FunStringMenuContextK(1, "Mech Preferences", display_mechpref,
                                     mech, NUM_MECHPREFERENCES);
    ShowCoolMenu(btech_context_evaluation(mech->xcode.context), player, c);
    KillCoolMenu(c);

  } else {

    size_t i;
    struct mechpref_info info;
    const char *newstate;

    /* Looking through the different mech preferences to find the
     * one the user wants to change */
    for (i = 0; i < NUM_MECHPREFERENCES; i++) {
      if (strcasecmp(args[0], mech_preference(i).name) == 0)
        break;
    }
    if (i == NUM_MECHPREFERENCES) {
      snprintf(buf, LBUF_SIZE, "Unknown MechPreference: %s", args[0]);
      mecha_notify(btech_context_evaluation(mech->xcode.context), player, buf);
      return;
    }

    /* Get the current setting */
    info = mech_preference(i);

    /* Did they provide a ON or OFF flag */
    if (nargs == 2) {

      /* Check to make sure its either ON or OFF */
      if ((strcasecmp(args[1], "ON") != 0) &&
          (strcasecmp(args[1], "OFF") != 0)) {

        /* Insert notify here */
        mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                     "Only accept ON or OFF as valid extra "
                     "parameter for mechprefs pref");
        return;
      }

      /* Set the value to what they want */
      if (strcasecmp(args[1], "ON") == 0) {

        /* Set the bit */
        if (info.flags & MECHPREF_FLAG_INVERTED) {
          ((mech)->rd.mech_prefs) &= ~(info.bit);
        } else {
          ((mech)->rd.mech_prefs) |= (info.bit);
        }

      } else {

        /* Unset the bit */
        if (info.flags & MECHPREF_FLAG_INVERTED) {
          ((mech)->rd.mech_prefs) |= (info.bit);
        } else {
          ((mech)->rd.mech_prefs) &= ~(info.bit);
        }
      }

    } else {

      /* If set, unset it, otherwise set the preference */
      if (((mech)->rd.mech_prefs) & info.bit)
        ((mech)->rd.mech_prefs) &= ~(info.bit);
      else
        ((mech)->rd.mech_prefs) |= (info.bit);
    }

    /* Which way did the preference get changed and
     * is it the default or non-standard mode of
     * the preference */
    if (((((mech)->rd.mech_prefs) & info.bit) &&
         (info.flags & MECHPREF_FLAG_INVERTED)) ||
        (!(((mech)->rd.mech_prefs) & info.bit) &&
         !(info.flags & MECHPREF_FLAG_INVERTED))) {

      if (info.flags & MECHPREF_FLAG_NEGATIVE)
        newstate = "[fg=green bold]OFF[reset]";
      else
        newstate = "[fg=red bold]OFF[reset]";

    } else {

      if (info.flags & MECHPREF_FLAG_NEGATIVE)
        newstate = "[fg=red bold]ON[reset]";
      else
        newstate = "[fg=green bold]ON[reset]";
    }

    /* Tell them the preference has been changed */
    snprintf(buf, LBUF_SIZE, "%s %s", info.msg, newstate);
    mecha_notify(btech_context_evaluation(mech->xcode.context), player, buf);
  }
}
