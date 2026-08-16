#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
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
#include "mux/support/stringutil.h"
#include "random.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static bool mech_disableweap_func(const MultiWeaponSelectionCall *call) {
  Mech *mech = call->mech;
  const int INDEX = call->first;
  int section;
  int critical;
  int weaptype;

  WeaponNumberLookupResult lookup = weapon_number_find(&(
      WeaponNumberLookupRequest){.mech = mech, .number = INDEX, .sight = true});
  weaptype = lookup.value;
  section = lookup.slot.section;
  critical = lookup.slot.critical;
  if (weaptype == -1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), call->actor,
                 "The weapons system chirps: 'Illegal Weapon Number!'");
    return false;
  }
  if (weaptype == -2) {
    mecha_notify(
        btech_context_evaluation(mech->xcode.context), call->actor,
        "The weapons system chirps: 'That Weapon has been destroyed!'");
    return false;
  }
  weaptype = weapon_from_equipment_index(
      mech_critical_part_type(mech, section, critical));
  if (!weapon_catalogue_has_special(weaptype, GAUSS)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), call->actor,
                 "You can only disable Gauss weapons.");
    return false;
  }
  if (mech_weapon_is_recycling_at(mech, section, critical)) {
    mecha_notify(
        btech_context_evaluation(mech->xcode.context), call->actor,
        "The weapon system chirps: 'That weapon is still recharging!'");
    return false;
  }

  mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
      .mech = mech,
      .slot = {.section = section, .critical = critical},
      .failure = FAIL_DESTROYED});
  mech_printf(mech, MECHALL, "You power down weapon %d.", INDEX);
  return false;
}

void mech_disableweap(DbRef player, Mech *mech, char *buffer) {
  char *args[1];

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Please specify a weapon number.");
    return;
  }

  multi_weapon_select(&(MultiWeaponSelectionRequest){
      .mech = mech,
      .actor = player,
      .selection = args[0],
      .mode = 1,
      .callback = mech_disableweap_func,
  });
}

int find_main_weapon(Mech *mech, int (*callback)(Mech *, int, int, int, int)) {
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
    count = find_weapons_advanced(mech, loop, weaparray, weapdata, critical, 1);
    if (count > 0) {
      for (ii = 0; ii < count; ii++) {
        const int CRITICAL_INDEX = *(const int *)checked_storage_at_const(
            critical, MAX_WEAPS_SECTION, sizeof(*critical), (size_t)ii);
        if (!mech_critical_is_broken(mech, loop, CRITICAL_INDEX)) {
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
    const unsigned char WEAPON =
        *(const unsigned char *)checked_storage_at_const(
            weaparray, MAX_WEAPS_SECTION, sizeof(*weaparray), (size_t)maxcount);
    return callback(mech, maxloc, WEAPON, maxcount, maxcrit);
  }
  return 0;
}

void mech_auto_turret(DbRef player, Mech *mech, char *buffer [[maybe_unused]]) {
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
  char str_location[80];
  int w_loc;
  int w_cur_loc;
  int w_section;
  int w_crit_slot;
  int w_weap_num;
  int w_weap_type;
  char *args[2];

  if (!common_checks(player, mech, MECH_USUALSO))
    return;

  if (mech_parseattributes(buffer, args, 2) != 2) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }

  if (!parse_int_checked(args[0], &w_weap_num)) {
    mecha_notifyf(btech_context_evaluation(mech->xcode.context), player,
                  "Invalid value: %s", args[0]);
    return;
  }
  WeaponNumberLookupResult lookup = weapon_number_find(
      &(WeaponNumberLookupRequest){.mech = mech, .number = w_weap_num});
  w_weap_type = lookup.value;
  w_section = lookup.slot.section;
  w_crit_slot = lookup.slot.critical;

  if (w_weap_type == -1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "The weapons system chirps: 'Illegal Weapon Number!'");
    return;
  }
  if (w_weap_type == -2) {
    mecha_notify(
        btech_context_evaluation(mech->xcode.context), player,
        "The weapons system chirps: 'That Weapon has been destroyed!'");
    return;
  }
  if (w_weap_type == -3) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "The weapon system chirps: 'That weapon is still reloading!'");
    return;
  }
  if (w_weap_type == -4) {
    mecha_notify(
        btech_context_evaluation(mech->xcode.context), player,
        "The weapon system chirps: 'That weapon is still recharging!'");
    return;
  }
  if (weapon_catalogue_is_energy(w_weap_type)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Energy weapons do not use ammo!");
    return;
  }

  if (args[1][0] == '-') {
    mech_printf(mech, MECHALL, "Prefered ammo source reset for weapon #%d",
                w_weap_num);
    mech_critical_desired_ammo_section_set(mech, w_section, w_crit_slot, -1);
    return;
  }

  w_loc =
      armor_section_from_string((UnitClass)((mech)->ud.type),
                                (MechMovementType)((mech)->ud.move), args[1]);

  if (w_loc == -1) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid section!");
    return;
  }
  if (!mech_section_original_internal(mech, w_loc)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "Invalid section!");
    return;
  }
  if (!mech_section_internal(mech, w_loc)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                 "That section is destroyed!");
    return;
  }

  armor_string_from_index(w_loc, str_location, (UnitClass)((mech)->ud.type),
                          (MechMovementType)((mech)->ud.move));
  w_cur_loc = mech_critical_desired_ammo_section(mech, w_section, w_crit_slot);

  if (w_cur_loc == w_loc) {
    mecha_notifyf(btech_context_evaluation(mech->xcode.context), player,
                  "Prefered ammo source already set to %s for weapon #%d",
                  str_location, w_weap_num);
    return;
  }

  mech_printf(mech, MECHALL, "Prefered ammo source set to %s for weapon #%d",
              str_location, w_weap_num);
  mech_critical_desired_ammo_section_set(mech, w_section, w_crit_slot, w_loc);
}

void mech_safety(DbRef player, Mech *mech, char *buffer) {
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
}

enum MechPreferenceFlag : int {
  MECHPREF_FLAG_INVERTED = 0x01,
  MECHPREF_FLAG_NEGATIVE = 0x02,
};

typedef struct MechPreference {
  int bit;
  unsigned char flags;
  const char *name;
  const char *msg;
} MechPreference;

static const MechPreference MECH_PREFERENCES[] = {
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
static size_t mech_preference_count(void) {
  return sizeof(MECH_PREFERENCES) / sizeof(*MECH_PREFERENCES);
}

static MechPreference mech_preference(size_t index) {
  return *(const MechPreference *)checked_storage_at_const(
      MECH_PREFERENCES, mech_preference_count(), sizeof(*MECH_PREFERENCES),
      index);
}

static char *display_mechpref(void *context, int i,
                              char buffer[static LBUF_SIZE]) {
  Mech *mech = context;
  MechPreference info = mech_preference((size_t)i);
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

  (void)snprintf(buffer, LBUF_SIZE, "        %-40s%s", info.name, state);
  return buffer;
}

void mech_mechprefs(DbRef player, Mech *mech, char *buffer) {
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
    c = sel_col_fun_string_menu_context_k(
        1, "Mech Preferences", display_mechpref, mech,
        clamp_size_to_int(mech_preference_count()));
    show_cool_menu(btech_context_evaluation(mech->xcode.context), player, c);
    kill_cool_menu(c);

  } else {

    size_t i;
    MechPreference info;
    const char *newstate;

    /* Looking through the different mech preferences to find the
     * one the user wants to change */
    for (i = 0; i < mech_preference_count(); i++) {
      if (strcasecmp(args[0], mech_preference(i).name) == 0)
        break;
    }
    if (i == mech_preference_count()) {
      (void)snprintf(buf, LBUF_SIZE, "Unknown MechPreference: %s", args[0]);
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
    (void)snprintf(buf, LBUF_SIZE, "%s %s", info.msg, newstate);
    mecha_notify(btech_context_evaluation(mech->xcode.context), player, buf);
  }
}
