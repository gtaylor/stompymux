#include "mech_advanced_internal.h"

static void mech_toggle_mode_sub(DbRef player, Mech *mech, char *buffer,
                                 int nspecisspec, int nspec, int mode,
                                 int tFireMode, char *onmsg, char *offmsg,
                                 char *cant);

/* Toggles ECM on / off */
void mech_ams(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);

  SILLY_TOGGLE_MACRO(IS_ANTI_MISSILE_TECH | CL_ANTI_MISSILE_TECH, AMS_ENABLED,
                     "Anti-Missile System turned ON",
                     "Anti-Missile System turned OFF",
                     "This mech is not equipped with AMS");
}

void mech_fliparms(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  DOCHECK_CONTEXT(mech->xcode.context, Fallen(mech),
                  "You're using your arms to support yourself. Try "
                  "flipping something else.");
  SILLY_TOGGLE_MACRO(FLIPABLE_ARMS, FLIPPED_ARMS,
                     "Arms have been flipped to BACKWARD position",
                     "Arms have been flipped to FORWARD position",
                     "You cannot flip the arms in this mech");
}

/* Parameters:
   <player,mech,buffer> = parent's input
   nspecisspec, nspec
   6 = Artemis
         5 = check that it's a TAMMO weapon and has the specified spec
   4 = check that weapon's SRM (or SSRM)
   3 = check that weapon's NARC beacon
   2 = check that weapon's LRM
   1 = compare nspec to weapon's special
   0 = compare nspec to weapon's type and ensure it isn't NARCbcn
   -1 = compare nspec to weapon's type & check for Artemis
   mode           = mode to set if nspec check is successful
   onmsg          = msg for turning mode on
   offmsg         = msg for turning mode off
   cant           = system lacks nspec
 */

typedef struct ToggleModeContext ToggleModeContext;
struct ToggleModeContext {
  int special_kind;
  int special;
  int mode;
  int fire_mode;
  char *on_message;
  char *off_message;
  char *cannot_message;
};

/* The mode messages are string-literal arguments supplied by the callers
 * below; their literal-ness is lost when passed through the context. */
#ifdef __clang__
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wformat-nonliteral"
#endif
static int mech_toggle_mode_sub_func(Mech *mech, DbRef player, int index,
                                     int high, void *context) {
  int section, critical, weaptype;
  const ToggleModeContext *toggle = context;

  weaptype =
      FindWeaponNumberOnMech_Advanced(mech, index, &section, &critical, 0);

  DOCHECK0_CONTEXT(mech->xcode.context, weaptype == -1,
                   "The weapons system chirps: 'Illegal Weapon Number!'");
  DOCHECK0_CONTEXT(
      mech->xcode.context, weaptype == -2,
      "The weapons system chirps: 'That Weapon has been destroyed!'");
  DOCHECK0_CONTEXT(
      mech->xcode.context, weaptype == -3,
      "The weapon system chirps: 'That weapon is still reloading!'");
  DOCHECK0_CONTEXT(
      mech->xcode.context, weaptype == -4,
      "The weapon system chirps: 'That weapon is still recharging!'");
  DOCHECK0_CONTEXT(
      mech->xcode.context,
      PartTempNuke(mech, section, critical) == FAIL_AMMOJAMMED,
      "The ammo feed mechanism for that weapon is jammed! Unable to "
      "change modes!");
  DOCHECK0_CONTEXT(mech->xcode.context,
                   GetPartFireMode(mech, section, critical) & OS_MODE,
                   "One-shot weapons' mode cannot be altered!");
  DOCHECK0_CONTEXT(mech->xcode.context,
                   isWeapAmmoFeedLocked(mech, section, critical),
                   "That weapon's ammo feed mechanism is damaged!");

  if (toggle->special_kind == 6) {
    DOCHECK0_CONTEXT(mech->xcode.context,
                     !FindArtemisForWeapon(mech, section, critical),
                     "You do not have an Artemis system for that weapon.");
  }

  weaptype = Weapon2I(GetPartType(mech, section, critical));

  DOCHECK0_CONTEXT(mech->xcode.context, MechWeapons[weaptype].special & ROCKET,
                   "Rocket launchers' mode cannot be altered!");

  if ((toggle->special_kind == 6 && (MechWeapons[weaptype].type == TMISSILE)) ||
      (toggle->special_kind == 5 && (MechWeapons[weaptype].type == TAMMO)

       && !(MechWeapons[weaptype].special & toggle->special)) ||
      (toggle->special_kind == 4 && (MechWeapons[weaptype].type == TMISSILE) &&
       !(MechWeapons[weaptype].type & (IDF | DAR))) ||
      (toggle->special_kind == 2 && (MechWeapons[weaptype].special & IDF) &&
       !(MechWeapons[weaptype].special & DAR)) ||
      (toggle->special_kind == 1 && toggle->special &&
       (MechWeapons[weaptype].special & toggle->special)) ||
      (toggle->special_kind <= 0 && toggle->special &&
       (MechWeapons[weaptype].type == toggle->special &&
        !(MechWeapons[weaptype].special & NARC)))) {

    if (toggle->special_kind == 0 && (toggle->special & TARTILLERY))
      DOCHECK0_CONTEXT(
          mech->xcode.context,
          (GetPartAmmoMode(mech, section, critical) & ARTILLERY_MODES) &&
              !(GetPartAmmoMode(mech, section, critical) & toggle->mode),
          "That weapon has already been set to fire special rounds!");
    /* Fitz - Group RAC/INARC select: Handle clearing RAC and INARC modes first
     */
    if ((toggle->special == RAC) && !toggle->mode) {
      if (!(GetPartFireMode(mech, section, critical) & RAC_MODES)) {
        mech_notify(mech, MECHALL, tprintf(toggle->off_message, index));
      } else {
        GetPartFireMode(mech, section, critical) &= ~FIRE_MODES;
        mech_notify(mech, MECHALL, tprintf(toggle->on_message, index));
      }
      return 0;
    } else if ((toggle->special == INARC) && !toggle->mode) {
      if (!(GetPartAmmoMode(mech, section, critical) & INARC_MODES)) {
        mech_notify(mech, MECHALL, tprintf(toggle->off_message, index));
      } else {
        GetPartAmmoMode(mech, section, critical) &= ~AMMO_MODES;
        mech_notify(mech, MECHALL, tprintf(toggle->on_message, index));
      }
      return 0;
    } else {

      if (toggle->fire_mode) {
        if (GetPartFireMode(mech, section, critical) & toggle->mode) {
          if (toggle->special != RAC) { /* Fitz - Keep RAC type weapons on new
                                      setting if already there */
            GetPartFireMode(mech, section, critical) &= ~toggle->mode;
          }
          mech_notify(mech, MECHALL, tprintf(toggle->off_message, index));
          return 0;
        }
      } else {
        if (GetPartAmmoMode(mech, section, critical) & toggle->mode) {
          if (toggle->special != INARC) { /* Fitz - Keep INARC type weapons on
                                        new setting if already there */
            GetPartAmmoMode(mech, section, critical) &= ~toggle->mode;
          }
          mech_notify(mech, MECHALL, tprintf(toggle->off_message, index));
          return 0;
        }
      }

      if (toggle->fire_mode) {
        GetPartFireMode(mech, section, critical) &= ~FIRE_MODES;
        GetPartFireMode(mech, section, critical) |= toggle->mode;
      } else {
        GetPartAmmoMode(mech, section, critical) &= ~AMMO_MODES;
        GetPartAmmoMode(mech, section, critical) |= toggle->mode;
      }

      mech_notify(mech, MECHALL, tprintf(toggle->on_message, index));

      return 0;
    }
  }
  if (toggle->special != RAC) /* Keep RAC type weapons on this setting */
    notify(btech_context_evaluation(mech->xcode.context), player,
           toggle->cannot_message);
  return 0;
}
#ifdef __clang__
#pragma clang diagnostic pop
#endif

static void mech_toggle_mode_sub(DbRef player, Mech *mech, char *buffer,
                                 int nspecisspec, int nspec, int mode,
                                 int tFireMode, char *onmsg, char *offmsg,
                                 char *cant) {
  char *args[1];
  ToggleModeContext toggle = {
      .special_kind = nspecisspec,
      .special = nspec,
      .mode = mode,
      .fire_mode = tFireMode,
      .on_message = onmsg,
      .off_message = offmsg,
      .cannot_message = cant,
  };

  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_parseattributes(buffer, args, 1) != 1,
                  "Please specify a weapon number.");
  multi_weap_sel(mech, player, args[0], 1, mech_toggle_mode_sub_func, &toggle);
}

void mech_flamerheat(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 1, CHEAT, HEAT_MODE, 1,
                       "Weapon %d has been set to HEAT mode",
                       "Weapon %d has been set to normal mode",
                       "That weapon cannot be set HEAT!");
}

void mech_ultra(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 1, ULTRA, ULTRA_MODE, 1,
                       "Weapon %d has been set to ultra fire mode",
                       "Weapon %d has been set to normal fire mode",
                       "That weapon cannot be set ULTRA!");
}

void mech_inarc_ammo_toggle(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  int wcArgs = 0;
  char *args[2];

  cch(MECH_USUALO);

  wcArgs = mech_parseattributes(buffer, args, 2);

  if (wcArgs < 2)
    mech_toggle_mode_sub(player, mech, buffer, 1, INARC, 0, 0,
                         "Weapon %d has been set to fire INARC Homing pods",
                         "Weapon %d is already set to fire INARC Homing pods",
                         "That weapon is not an INARC launcher!");
  else {
    switch (toupper(args[1][0])) {
    case 'X':
      mech_toggle_mode_sub(
          player, mech, buffer, 1, INARC, INARC_EXPLO_MODE, 0,
          "Weapon %d has been set to fire INARC Explosive pods",
          "Weapon %d is already set to fire INARC Explosive pods",
          "That weapon is not an INARC launcher!");
      break;
    case 'Y':
      mech_toggle_mode_sub(
          player, mech, buffer, 1, INARC, INARC_HAYWIRE_MODE, 0,
          "Weapon %d has been set to fire INARC Haywire pods",
          "Weapon %d is already set to fire INARC Haywire pods",
          "That weapon is not an INARC launcher!");
      break;
    case 'E':
      mech_toggle_mode_sub(player, mech, buffer, 1, INARC, INARC_ECM_MODE, 0,
                           "Weapon %d has been set to fire INARC ECM pods",
                           "Weapon %d is already set to fire INARC ECM pods",
                           "That weapon is not an INARC launcher!");
      break;
    case 'Z':
      mech_toggle_mode_sub(
          player, mech, buffer, 1, INARC, INARC_NEMESIS_MODE, 0,
          "Weapon %d has been set to fire INARC Nemesis pods",
          "Weapon %d is already set to fire INARC Nemesis pods",
          "That weapon is not an INARC launcher!");
      break;
    default:
      mech_toggle_mode_sub(player, mech, buffer, 1, INARC, 0, 0,
                           "Weapon %d has been set to fire INARC Homing pods",
                           "Weapon %d is already set to fire INARC Homing pods",
                           "That weapon is not an INARC launcher!");
      break;
    }
  }
}

void mech_explosive(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);

  mech_toggle_mode_sub(player, mech, buffer, 1, NARC, NARC_MODE, 0,
                       "Weapon %d has been set to fire explosive rounds",
                       "Weapon %d has been set to fire NARC beacons",
                       "That weapon cannot be set to fire explosive rounds!");
}

void mech_lbx(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 1, LBX, LBX_MODE, 0,
                       "Weapon %d has been set to LBX fire mode",
                       "Weapon %d has been set to normal fire mode",
                       "That weapon cannot be set LBX!");
}

void mech_armorpiercing(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, AC_AP_MODE, 0,
                       "Weapon %d has been set to fire AP rounds",
                       "Weapon %d has been set to fire normal rounds",
                       "That weapon cannot fire AP rounds!");
}

void mech_caseless(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, AC_CASELESS_MODE, 0,
                       "Weapon %d has been set to fire CASELESS rounds",
                       "Weapon %d has been set to fire normal rounds",
                       "That weapon cannot fire CASELESS rounds!");
}

void mech_flechette(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, AC_FLECHETTE_MODE, 0,
                       "Weapon %d has been set to fire Flechette rounds",
                       "Weapon %d has been set to fire normal rounds",
                       "That weapon cannot fire Flechette rounds!");
}

void mech_incendiary(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, AC_INCENDIARY_MODE, 0,
                       "Weapon %d has been set to fire Incendiary rounds",
                       "Weapon %d has been set to fire normal rounds",
                       "That weapon cannot fire Incendiary rounds!");
}

void mech_precision(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, AC_PRECISION_MODE, 0,
                       "Weapon %d has been set to fire Precision rounds",
                       "Weapon %d has been set to fire normal rounds",
                       "That weapon cannot fire Precision rounds!");
}

void mech_rapidfire(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, RFAC_MODE, 1,
                       "Weapon %d has been set to Rapid Fire mode",
                       "Weapon %d has been set to normal fire mode",
                       "That weapon cannot be set to do rapid fire!");
}

void mech_stinger(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 2, 0, STINGER_MODE, 0,
                       "Weapon %d has been set to fire stinger missiles.",
                       "Weapon %d has been set to fire normal missiles",
                       "That weapon cannot be set STINGER!");
}

void mech_rac(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  int wcArgs = 0;
  char *args[2];

  wcArgs = mech_parseattributes(buffer, args, 2);

  if (wcArgs < 2)
    mech_toggle_mode_sub(player, mech, buffer, 1, RAC, 0, 1,
                         "Weapon %d has been set to fire one shot at a time.",
                         "Weapon %d is already set to fire one shot at a time.",
                         "That weapon is not a RotaryAC!");
  else {
    switch (args[1][0]) {
    case '2':
      mech_toggle_mode_sub(
          player, mech, buffer, 1, RAC, RAC_TWOSHOT_MODE, 1,
          "Weapon %d has been set to fire two shots at a time.",
          "Weapon %d is already set to fire two shots at a time.",
          "That weapon is not a RotaryAC!");
      break;
    case '4':
      mech_toggle_mode_sub(
          player, mech, buffer, 1, RAC, RAC_FOURSHOT_MODE, 1,
          "Weapon %d has been set to fire four shots at a time.",
          "Weapon %d is already set to fire four shots at a time.",
          "That weapon is not a RotaryAC!");
      break;
    case '6':
      mech_toggle_mode_sub(
          player, mech, buffer, 1, RAC, RAC_SIXSHOT_MODE, 1,
          "Weapon %d has been set to fire six shots at a time.",
          "Weapon %d is already set to fire six shots at a time.",
          "That weapon is not a RotaryAC!");
      break;
    default:
      mech_toggle_mode_sub(
          player, mech, buffer, 1, RAC, 0, 1,
          "Weapon %d has been set to fire one shot at a time.",
          "Weapon %d is already set to fire one shot at a time.",
          "That weapon is not a RotaryAC!");
      break;
    }
  }
}

static int mech_unjamammo_func(Mech *mech, DbRef player, int index, int high,
                               void *context) {
  (void)context;
  int section, critical, weaptype;
  int i;
  char location[50];

  weaptype = FindWeaponNumberOnMech(mech, index, &section, &critical);
  DOCHECK0_CONTEXT(mech->xcode.context, weaptype == -1,
                   "The weapons system chirps: 'Illegal Weapon Number!'");
  DOCHECK0_CONTEXT(
      mech->xcode.context, weaptype == -2,
      "The weapons system chirps: 'That Weapon has been destroyed!'");
  DOCHECK0_CONTEXT(mech->xcode.context,
                   PartTempNuke(mech, section, critical) != FAIL_AMMOJAMMED,
                   "The ammo feed mechanism for that weapon is not jammed.");
  DOCHECK0_CONTEXT(mech->xcode.context, Jumping(mech),
                   "You can't unjam the ammo feed while jumping!");
  DOCHECK0_CONTEXT(mech->xcode.context,
                   IsRunning(MechDesiredSpeed(mech), MMaxSpeed(mech)),
                   "You can't unjam the ammo feed while running!");

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (SectHasBusyWeap(mech, i)) {
      ArmorStringFromIndex(i, location, MechType(mech), MechMove(mech));
      mech_printf(mech, MECHALL, "You have weapons recycling on your %s.",
                  location);
      return 0;
    }
  }

  DOCHECK0_CONTEXT(mech->xcode.context,
                   mech_event_count(mech, EVENT_UNJAM_AMMO),
                   "You are already unjamming a weapon!");

  mech_event_schedule(mech, EVENT_UNJAM_AMMO, mech_unjam_ammo_event, 60,
                      (long)index);
  mech_printf(mech, MECHALL,
              "You begin to shake the jammed ammo loose on weapon #%d", index);
  return 0;
}
void mech_unjamammo(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[1];

  cch(MECH_USUALMO);
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_parseattributes(buffer, args, 1) != 1,
                  "Please specify a weapon number.");
  multi_weap_sel(mech, player, args[0], 1, mech_unjamammo_func, nullptr);
}

void mech_gattling(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 1, GMG, GATTLING_MODE, 1,
                       "Weapon %d has been set to Gattling mode",
                       "Weapon %d has been set to normal fire mode",
                       "That weapon cannot be set to do gattling fire!");
}

void mech_artemis(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(
      player, mech, buffer, 6, TMISSILE, ARTEMIS_MODE, 0,
      "Weapon %d has been set to fire Artemis IV compatible missiles.",
      "Weapon %d has been set to fire normal missiles",
      "That weapon cannot be set ARTEMIS!");
}

void mech_narc(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(
      player, mech, buffer, 0, TMISSILE, NARC_MODE, 0,
      "Weapon %d has been set to fire Narc Beacon compatible missiles.",
      "Weapon %d has been set to fire normal missiles",
      "That weapon cannot be set NARC!");
}

void mech_swarm(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 2, 0, SWARM_MODE, 0,
                       "Weapon %d has been set to fire Swarm missiles.",
                       "Weapon %d has been set to fire normal missiles",
                       "That weapon cannot be set to fire Swarm missiles!");
}

void mech_sguided(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 2, 0, SGUIDED_MODE, 0,
                       "Weapon %d has been set to fire Sguided missiles.",
                       "Weapon %d has been set to fire normal missiles",
                       "That weapon cannot be set to fire Sguided missiles!");
}

void mech_atmrange(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(
      player, mech, buffer, 2, TMISSILE, ATM_ER_MODE, 0,
      "Weapon %d has been set to fire Extended Range missiles.",
      "Weapon %d has been set to fire normal missiles.",
      "That weapon cannot be set to fire Extended Range missiles!");
}

void mech_atmexplosive(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(
      player, mech, buffer, 2, TMISSILE, ATM_HE_MODE, 0,
      "Weapon %d has been set to fire High Explosive missiles.",
      "Weapon %d has been set to fire normal missiles.",
      "That weapon cannot be set to fire High Explosive missiles!");
}

void mech_swarm1(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 2, 0, SWARM1_MODE, 0,
                       "Weapon %d has been set to fire Swarm1 missiles.",
                       "Weapon %d has been set to fire normal missiles",
                       "That weapon cannot be set to fire Swarm1 missiles!");
}

void mech_inferno(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 4, 0, INFERNO_MODE, 0,
                       "Weapon %d has been set to fire Inferno missiles.",
                       "Weapon %d has been set to fire normal missiles",
                       "That weapon cannot be set to fire Inferno missiles!");
}

void mech_hotload(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 1, IDF, HOTLOAD_MODE, 1,
                       "Hotloading for weapon %d has been toggled on.",
                       "Hotloading for weapon %d has been toggled off.",
                       "That weapon can not be hotloaded!");
}

void mech_cluster(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 0, TARTILLERY, CLUSTER_MODE, 0,
                       "Weapon %d has been set to fire cluster rounds.",
                       "Weapon %d has been set to fire normal rounds",
                       "Invalid weapon type!");
}

void mech_smoke(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 4, 0, SMOKE_MODE, 0,
                       "Weapon %d has been set to fire smoke rounds.",
                       "Weapon %d has been set to fire normal rounds",
                       "Invalid weapon type!");
}

void mech_mine(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALMO);
  mech_toggle_mode_sub(player, mech, buffer, 4, 0, MINE_MODE, 0,
                       "Weapon %d has been set to fire mine rounds.",
                       "Weapon %d has been set to fire normal rounds",
                       "Invalid weapon type!");
}

static void mech_explode_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long extra = (long)e->data2;
  int i, j, k, damage;

  if (Destroyed(mech) || !Started(mech))
    return;

  if (extra > 256 && !FindDestructiveAmmo(mech, &i, &j))
    return;

  if ((--extra) % 256) {
    mech_printf(mech, MECHALL, "Self-destruction in %ld second%s..",
                extra % 256, extra > 1 ? "s" : "");
    mech_event_schedule(mech, EVENT_EXPLODE, mech_explode_event, 1, extra);
  } else {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("#%ld explodes.", mech->mynum));
    if (MechType(mech) == CLASS_BSUIT) {
      mech_notify(mech, MECHALL,
                  "Your batttle suit triggers it's self-destruction sequence.. "
                  "you faint.. (and die)");
      mech_los_broadcast(mech, "suddenly explodes!");
      headhitmwdamage(mech, mech, 4);
      for (k = 0; k < NUM_BSUIT_MEMBERS; k++)
        DestroySection(mech, mech, -1, k);
      MechZ(mech) += 6;
    } else if (MechType(mech) != CLASS_MECH) {
      mech_notify(mech, MECHALL,
                  "Your life flashes before your eyes as your vehicle "
                  "immolates itself... you faint.. (and die)");
      mech_los_broadcast(mech, "suddenly explodes!");
      DestroySection(mech, mech, -1, 3); // This is the back side for vehicles
      // and the aft for aero's.
      headhitmwdamage(mech, mech, 4);
      MechZ(mech) += 6;

    } else if (extra >= 256) {
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                         tprintf("#%ld explodes [ammo]", mech->mynum));
      mech_notify(mech, MECHALL, "All your ammo explodes!");
      while ((damage = FindDestructiveAmmo(mech, &i, &j)))
        mech_ammunition_explode(mech, mech, i, j, damage);
    } else {
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                         tprintf("#%ld explodes [reactor]", mech->mynum));
      mech_los_broadcast(mech, "suddenly explodes!");
      mech_notify(mech, MECHALL,
                  "Suddenly you feel great heat overcoming your senses.. you "
                  "faint.. (and die)");
      reactor_explosion(mech, mech);
    }
  }
}

void mech_explode(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[3];
  int i;
  int ammoloc, ammocritnum;
  long time = (long)mech->xcode.context->configuration->btech_explode_time;
  int ammo = 1;
  int argc;
  int override = 0;

  cch(MECH_USUALO);
  override = (strstr(buffer, "override") != NULL) &&
             is_wizard(mech->xcode.context->database, player);
  argc = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(mech->xcode.context, argc < 1,
                  "Invalid number of arguments!");

  /* Can't do any of the explosion routine if we're recycling! */
  if (!override) {
    for (i = 0; i < NUM_SECTIONS; i++) {
      if (!SectIsDestroyed(mech, i))
        DOCHECK_CONTEXT(mech->xcode.context, SectHasBusyWeap(mech, i),
                        "You have weapons recycling!");
      DOCHECK_CONTEXT(mech->xcode.context, MechSections(mech)[i].recycle,
                      "You are still recovering from your last attack.");
    }
  }

  if (!strcasecmp(buffer, "stop")) {
    if (!override) {
      DOCHECK_CONTEXT(mech->xcode.context,
                      !mech->xcode.context->configuration->btech_explode_stop,
                      "It's too late to turn back now!");
    }
    DOCHECK_CONTEXT(mech->xcode.context, !mech_event_count(mech, EVENT_EXPLODE),
                    "Your mech isn't undergoing a self-destruct sequence!");

    mech_event_cancel(mech, EVENT_EXPLODE);
    mech_notify(mech, MECHALL, "Self-destruction sequence aborted.");
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
        tprintf("#%ld in #%ld stopped the self-destruction sequence.", player,
                mech->mynum));
    mech_los_broadcast(mech, "regains control over itself.");
    return;
  }
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_EXPLODE),
                  "Your mech is already undergoing a self-destruct sequence!");
  if (!strcasecmp(buffer, "ammo")) {
    /*
       Find SOME ammo to explode ; if possible, we engage the 'boom' process
     */
    if (!override) {
      DOCHECK_CONTEXT(mech->xcode.context,
                      !mech->xcode.context->configuration->btech_explode_ammo,
                      "You can't bring yourself to do it!");
      DOCHECK_CONTEXT(mech->xcode.context, MechStatus(mech) & EXPLODE_SAFE,
                      "That's not a possibility here.");
    }
    i = FindDestructiveAmmo(mech, &ammoloc, &ammocritnum);
    DOCHECK_CONTEXT(mech->xcode.context, !i,
                    "There is no 'damaging' ammo on your 'mech!");
    /* Engage the boom-event */
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
        tprintf("#%ld in #%ld initiates the ammo explosion sequence.", player,
                mech->mynum));
    mech_los_broadcast(mech, "starts billowing smoke!");
    time = time / 2;
  } else {
    if (!override) {
      DOCHECK_CONTEXT(
          mech->xcode.context,
          !mech->xcode.context->configuration->btech_explode_reactor,
          "You can't bring yourself to do it!");
      DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) != CLASS_MECH,
                      "Only mechs can do the 'big boom' effect.");
      DOCHECK_CONTEXT(mech->xcode.context, MechSpecials(mech) & ICE_TECH,
                      "You need a fusion reactor.");
    }
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
        tprintf("#%ld in #%ld initiates the reactor explosion sequence.",
                player, mech->mynum));
    mech_los_broadcast(mech, "loses reactions containment!");
    ammo = 0;
  }
  if (override)
    time = 3;
  mech_event_schedule(mech, EVENT_EXPLODE, mech_explode_event, 1, time);
  mech_notify(mech, MECHALL,
              "Self-destruction sequence engaged ; please stand by.");
  mech_printf(mech, MECHALL, "%s in %ld seconds.",
              ammo ? "The ammunition will explode" : "The reactor will blow up",
              time);
  MechPilot(mech) = -1; /* Pilot gives up control */
}
