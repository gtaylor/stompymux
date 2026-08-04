#include "mech_advanced_internal.h"

static int mech_disableweap_func(Mech *mech, DbRef player, int index, int high,
                                 void *context) {
  (void)context;
  int section, critical, weaptype;

  weaptype =
      FindWeaponNumberOnMech_Advanced(mech, index, &section, &critical, 1);
  DOCHECK0_CONTEXT(mech->xcode.context, weaptype == -1,
                   "The weapons system chirps: 'Illegal Weapon Number!'");
  DOCHECK0_CONTEXT(
      mech->xcode.context, weaptype == -2,
      "The weapons system chirps: 'That Weapon has been destroyed!'");
  weaptype = Weapon2I(GetPartType(mech, section, critical));
  DOCHECK0_CONTEXT(mech->xcode.context,
                   !(MechWeapons[weaptype].special & GAUSS),
                   "You can only disable Gauss weapons.");
  DOCHECK0_CONTEXT(
      mech->xcode.context, WpnIsRecycling(mech, section, critical),
      "The weapon system chirps: 'That weapon is still recharging!'");

  SetPartTempNuke(mech, section, critical, FAIL_DESTROYED);
  mech_printf(mech, MECHALL, "You power down weapon %d.", index);
  return 0;
}

void mech_disableweap(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[1];

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_parseattributes(buffer, args, 1) != 1,
                  "Please specify a weapon number.");

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
    if (SectIsDestroyed(mech, loop))
      continue;
    count = FindWeapons(mech, loop, weaparray, weapdata, critical);
    if (count > 0) {
      for (ii = 0; ii < count; ii++) {
        if (!PartIsBroken(mech, loop, critical[ii])) {
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
  if (critfound)
    return callback(mech, maxloc, weaparray[maxcount], maxcount, maxcrit);
  else
    return 0;
}

void mech_auto_turret(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALSO);

  DOCHECK_CONTEXT(mech->xcode.context, !GetSectInt(mech, TURRET),
                  "You have no turret to autoturn!");

  mech_printf(mech, MECHALL, "Automatic turret turning is now %s",
              (MechStatus2(mech) & AUTOTURN_TURRET) ? "OFF" : "ON");

  if (MechStatus2(mech) & AUTOTURN_TURRET)
    MechStatus2(mech) &= ~AUTOTURN_TURRET;
  else
    MechStatus2(mech) |= AUTOTURN_TURRET;
}

void mech_usebin(DbRef player, Mech *mech, char *buffer) {
  char strLocation[80];
  int wLoc, wCurLoc;
  int wSection, wCritSlot, wWeapNum, wWeapType;
  char *args[2];

  cch(MECH_USUALSO);

  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_parseattributes(buffer, args, 2) != 2,
                  "Invalid number of arguments!");

  DOCHECK_CONTEXT(mech->xcode.context, Readnum(wWeapNum, args[0]),
                  tprintf("Invalid value: %s", args[0]));
  wWeapType = FindWeaponNumberOnMech(mech, wWeapNum, &wSection, &wCritSlot);

  DOCHECK_CONTEXT(mech->xcode.context, wWeapType == -1,
                  "The weapons system chirps: 'Illegal Weapon Number!'");
  DOCHECK_CONTEXT(
      mech->xcode.context, wWeapType == -2,
      "The weapons system chirps: 'That Weapon has been destroyed!'");
  DOCHECK_CONTEXT(
      mech->xcode.context, wWeapType == -3,
      "The weapon system chirps: 'That weapon is still reloading!'");
  DOCHECK_CONTEXT(
      mech->xcode.context, wWeapType == -4,
      "The weapon system chirps: 'That weapon is still recharging!'");
  DOCHECK_CONTEXT(mech->xcode.context, IsEnergy(wWeapType),
                  "Energy weapons do not use ammo!");

  if (args[1][0] == '-') {
    mech_printf(mech, MECHALL, "Prefered ammo source reset for weapon #%d",
                wWeapNum);
    SetPartDesiredAmmoLoc(mech, wSection, wCritSlot, -1);
    return;
  }

  wLoc = ArmorSectionFromString(MechType(mech), MechMove(mech), args[1]);

  DOCHECK_CONTEXT(mech->xcode.context, wLoc == -1, "Invalid section!");
  DOCHECK_CONTEXT(mech->xcode.context, !GetSectOInt(mech, wLoc),
                  "Invalid section!");
  DOCHECK_CONTEXT(mech->xcode.context, !GetSectInt(mech, wLoc),
                  "That section is destroyed!");

  ArmorStringFromIndex(wLoc, strLocation, MechType(mech), MechMove(mech));
  wCurLoc = GetPartDesiredAmmoLoc(mech, wSection, wCritSlot);

  DOCHECK_CONTEXT(
      mech->xcode.context, wCurLoc == wLoc,
      tprintf("Prefered ammo source already set to %s for weapon #%d",
              strLocation, wWeapNum));

  mech_printf(mech, MECHALL, "Prefered ammo source set to %s for weapon #%d",
              strLocation, wWeapNum);
  SetPartDesiredAmmoLoc(mech, wSection, wCritSlot, wLoc);
}

void mech_safety(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) == CLASS_MW,
                  "Your weapons dont have safeties.");
  if (buffer && !strcasecmp(buffer, "on")) {
    UnSetMechPKiller(mech);
    mech_notify(mech, MECHALL, "Safeties flipped [fg=green bold]ON[reset].");
    return;
  }
  if (buffer && !strcasecmp(buffer, "off")) {
    SetMechPKiller(mech);
    mech_notify(mech, MECHALL, "Safeties flipped [fg=red bold]OFF[reset].");
    return;
  }

  mech_printf(mech, MECHPILOT, "Weapon safeties are [bold]%s[reset]",
              MechPKiller(mech) ? "[fg=red]OFF" : "[fg=green]ON");
  return;
}

#define MECHPREF_FLAG_INVERTED 0x01
#define MECHPREF_FLAG_NEGATIVE 0x02

static struct mechpref_info {
  int bit;
  unsigned char flags;
  char *name;
  char *msg;
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

static char *display_mechpref(void *context, int i,
                              char buffer[static LBUF_SIZE]) {
  Mech *mech = context;
  struct mechpref_info info = mech_preferences[i];
  char *state;

  if (((MechPrefs(mech) & info.bit) && (info.flags & MECHPREF_FLAG_INVERTED)) ||
      (!(MechPrefs(mech) & info.bit) &&
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

  cch(MECH_USUALSMO);
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
    char *newstate;

    /* Looking through the different mech preferences to find the
     * one the user wants to change */
    for (i = 0; i < NUM_MECHPREFERENCES; i++) {
      if (strcasecmp(args[0], mech_preferences[i].name) == 0)
        break;
    }
    if (i == NUM_MECHPREFERENCES) {
      snprintf(buf, LBUF_SIZE, "Unknown MechPreference: %s", args[0]);
      notify(btech_context_evaluation(mech->xcode.context), player, buf);
      return;
    }

    /* Get the current setting */
    info = mech_preferences[i];

    /* Did they provide a ON or OFF flag */
    if (nargs == 2) {

      /* Check to make sure its either ON or OFF */
      if ((strcasecmp(args[1], "ON") != 0) &&
          (strcasecmp(args[1], "OFF") != 0)) {

        /* Insert notify here */
        notify(btech_context_evaluation(mech->xcode.context), player,
               "Only accept ON or OFF as valid extra "
               "parameter for mechprefs pref");
        return;
      }

      /* Set the value to what they want */
      if (strcasecmp(args[1], "ON") == 0) {

        /* Set the bit */
        if (info.flags & MECHPREF_FLAG_INVERTED) {
          MechPrefs(mech) &= ~(info.bit);
        } else {
          MechPrefs(mech) |= (info.bit);
        }

      } else {

        /* Unset the bit */
        if (info.flags & MECHPREF_FLAG_INVERTED) {
          MechPrefs(mech) |= (info.bit);
        } else {
          MechPrefs(mech) &= ~(info.bit);
        }
      }

    } else {

      /* If set, unset it, otherwise set the preference */
      if (MechPrefs(mech) & info.bit)
        MechPrefs(mech) &= ~(info.bit);
      else
        MechPrefs(mech) |= (info.bit);
    }

    /* Which way did the preference get changed and
     * is it the default or non-standard mode of
     * the preference */
    if (((MechPrefs(mech) & info.bit) &&
         (info.flags & MECHPREF_FLAG_INVERTED)) ||
        (!(MechPrefs(mech) & info.bit) &&
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
    notify(btech_context_evaluation(mech->xcode.context), player, buf);
  }
}
