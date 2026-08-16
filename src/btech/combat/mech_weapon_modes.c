
#include "btech/context.h"
#include "btech_event.h"
#include "btech_text_builder.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "failures.h"
#include "mech_advanced_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
#include <stddef.h>
#include <string.h>

static void mech_toggle_mode_sub(DbRef player, Mech *mech, char *buffer,
                                 int nspecisspec, int nspec, int mode,
                                 int t_fire_mode, const char *onmsg,
                                 const char *offmsg, const char *cant);

/* Toggles ECM on / off */
void mech_ams(DbRef player, Mech *mech, char *buffer [[maybe_unused]]) {
  BtechContext *context = mech_context(mech);

  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  if (!(mech_technology_flags(mech) &
        (IS_ANTI_MISSILE_TECH | CL_ANTI_MISSILE_TECH))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This mech is not equipped with AMS");
    return;
  }
  bool enabled = (!mech_condition_summary(mech).ams_enabled) != 0;
  mech_ams_enabled_set(mech, enabled);
  mech_notify(mech, MECHALL,
              enabled ? "Anti-Missile System turned ON"
                      : "Anti-Missile System turned OFF");
}

void mech_fliparms(DbRef player, Mech *mech, char *buffer [[maybe_unused]]) {
  BtechContext *context = mech_context(mech);

  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  if (mech_condition_summary(mech).fallen) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're using your arms to support yourself. Try "
                 "flipping something else.");
    return;
  }
  if (!(mech_technology_flags(mech) & FLIPABLE_ARMS)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot flip the arms in this mech");
    return;
  }
  bool flipped = mech_condition_summary(mech).arms_flipped;
  if (flipped)
    mech_arms_center(mech);
  else
    mech_arms_flip(mech);
  mech_notify(mech, MECHALL,
              flipped ? "Arms have been flipped to FORWARD position"
                      : "Arms have been flipped to BACKWARD position");
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
  const char *on_message;
  const char *off_message;
  const char *cannot_message;
};

static void notify_mode_message(Mech *mech, const char *message_template,
                                int weapon_number) {
  const char *placeholder = strstr(message_template, "%d");
  if (placeholder == nullptr) {
    mech_notify(mech, MECHALL, message_template);
    return;
  }

  char message[LBUF_SIZE];
  BtechTextBuilder output;
  btech_text_builder_initialize(&output, message, sizeof(message));
  btech_text_builder_append_count(&output, message_template,
                                  (size_t)(placeholder - message_template));
  btech_text_builder_append_format(&output, "%d", weapon_number);
  btech_text_builder_append(&output, checked_string_suffix(placeholder, 2));
  mech_notify(mech, MECHALL, message);
}

static bool mech_toggle_mode_sub_func(const MultiWeaponSelectionCall *call) {
  Mech *mech = call->mech;
  const DbRef PLAYER = call->actor;
  const int INDEX = call->first;
  int section;
  int critical;
  int weaptype;
  const ToggleModeContext *toggle = call->context;

  WeaponNumberLookupResult lookup = weapon_number_find(
      &(WeaponNumberLookupRequest){.mech = mech, .number = INDEX});
  weaptype = lookup.value;
  section = lookup.slot.section;
  critical = lookup.slot.critical;

  if (weaptype == -1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "The weapons system chirps: 'Illegal Weapon Number!'");
    return false;
  }
  if (weaptype == -2) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), PLAYER,
        "The weapons system chirps: 'That Weapon has been destroyed!'");
    return false;
  }
  if (weaptype == -3) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "The weapon system chirps: 'That weapon is still reloading!'");
    return false;
  }
  if (weaptype == -4) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), PLAYER,
        "The weapon system chirps: 'That weapon is still recharging!'");
    return false;
  }
  if (mech_critical_temporary_failure(mech, section, critical) ==
      FAIL_AMMOJAMMED) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "The ammo feed mechanism for that weapon is jammed! Unable to "
                 "change modes!");
    return false;
  }
  if (mech_critical_fire_mode(mech, section, critical) & OS_MODE) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "One-shot weapons' mode cannot be altered!");
    return false;
  }
  if (mech_weapon_ammo_feed_is_locked(mech, section, critical)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "That weapon's ammo feed mechanism is damaged!");
    return false;
  }

  if (toggle->special_kind == 6) {
    if (!find_artemis_for_weapon(mech, section, critical)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                   "You do not have an Artemis system for that weapon.");
      return false;
    }
  }

  weaptype = weapon_from_equipment_index(
      mech_critical_part_type(mech, section, critical));

  if (weapon_catalogue_is_rocket(weaptype)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "Rocket launchers' mode cannot be altered!");
    return false;
  }

  const int WEAPON_TYPE = weapon_catalogue_type(weaptype);
  if ((toggle->special_kind == 6 && WEAPON_TYPE == TMISSILE) ||
      (toggle->special_kind == 5 && WEAPON_TYPE == TAMMO

       && !weapon_catalogue_has_special(weaptype, toggle->special)) ||
      (toggle->special_kind == 4 && WEAPON_TYPE == TMISSILE &&
       !(WEAPON_TYPE & (IDF | DAR))) ||
      (toggle->special_kind == 2 &&
       weapon_catalogue_supports_indirect_fire(weaptype) &&
       !weapon_catalogue_has_special(weaptype, DAR)) ||
      (toggle->special_kind == 1 && toggle->special &&
       weapon_catalogue_has_special(weaptype, toggle->special)) ||
      (toggle->special_kind <= 0 && toggle->special &&
       (WEAPON_TYPE == toggle->special &&
        !weapon_catalogue_is_narc(weaptype)))) {
    if (toggle->special_kind == 0 && (toggle->special & TARTILLERY)) {
      if ((mech_critical_ammo_mode(mech, section, critical) &
           ARTILLERY_MODES) &&
          !(mech_critical_ammo_mode(mech, section, critical) & toggle->mode)) {
        mecha_notify(
            btech_context_evaluation(mech_context(mech)), PLAYER,
            "That weapon has already been set to fire special rounds!");
        return false;
      }
    }
    /* Fitz - Group RAC/INARC select: Handle clearing RAC and INARC modes first
     */
    if ((toggle->special == RAC) && !toggle->mode) {
      if (!(mech_critical_fire_mode(mech, section, critical) & RAC_MODES)) {
        notify_mode_message(mech, toggle->off_message, INDEX);
      } else {
        mech_critical_fire_mode_clear(mech, section, critical, FIRE_MODES);
        notify_mode_message(mech, toggle->on_message, INDEX);
      }
      return false;
    }
    if ((toggle->special == INARC) && !toggle->mode) {
      if (!(mech_critical_ammo_mode(mech, section, critical) & INARC_MODES)) {
        notify_mode_message(mech, toggle->off_message, INDEX);
      } else {
        mech_critical_ammo_mode_clear(mech, section, critical, AMMO_MODES);
        notify_mode_message(mech, toggle->on_message, INDEX);
      }
      return false;
    }
    if (toggle->fire_mode) {
      if (mech_critical_fire_mode(mech, section, critical) & toggle->mode) {
        if (toggle->special != RAC) { /* Fitz - Keep RAC type weapons on new
                                    setting if already there */
          mech_critical_fire_mode_clear(mech, section, critical, toggle->mode);
        }
        notify_mode_message(mech, toggle->off_message, INDEX);
        return false;
      }
    } else {
      if (mech_critical_ammo_mode(mech, section, critical) & toggle->mode) {
        if (toggle->special != INARC) { /* Fitz - Keep INARC type weapons on
                                      new setting if already there */
          mech_critical_ammo_mode_clear(mech, section, critical, toggle->mode);
        }
        notify_mode_message(mech, toggle->off_message, INDEX);
        return false;
      }
    }

    if (toggle->fire_mode) {
      mech_critical_fire_mode_clear(mech, section, critical, FIRE_MODES);
      mech_critical_fire_mode_add(mech, section, critical, toggle->mode);
    } else {
      mech_critical_ammo_mode_clear(mech, section, critical, AMMO_MODES);
      mech_critical_ammo_mode_add(mech, section, critical, toggle->mode);
    }

    notify_mode_message(mech, toggle->on_message, INDEX);

    return false;
  }
  if (toggle->special != RAC) /* Keep RAC type weapons on this setting */
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 toggle->cannot_message);
  return false;
}
static void mech_toggle_mode_sub(DbRef player, Mech *mech, char *buffer,
                                 int nspecisspec, int nspec, int mode,
                                 int t_fire_mode, const char *onmsg,
                                 const char *offmsg, const char *cant) {
  char *args[1];
  ToggleModeContext toggle = {
      .special_kind = nspecisspec,
      .special = nspec,
      .mode = mode,
      .fire_mode = t_fire_mode,
      .on_message = onmsg,
      .off_message = offmsg,
      .cannot_message = cant,
  };

  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Please specify a weapon number.");
    return;
  }
  multi_weapon_select(&(MultiWeaponSelectionRequest){
      .mech = mech,
      .actor = player,
      .selection = args[0],
      .mode = 1,
      .callback = mech_toggle_mode_sub_func,
      .context = &toggle,
  });
}

void mech_flamerheat(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 1, CHEAT, HEAT_MODE, 1,
                       "Weapon %d has been set to HEAT mode",
                       "Weapon %d has been set to normal mode",
                       "That weapon cannot be set HEAT!");
}

void mech_ultra(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 1, ULTRA, ULTRA_MODE, 1,
                       "Weapon %d has been set to ultra fire mode",
                       "Weapon %d has been set to normal fire mode",
                       "That weapon cannot be set ULTRA!");
}

void mech_inarc_ammo_toggle(DbRef player, Mech *mech, char *buffer) {
  int wc_args = 0;
  char *args[2];

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  wc_args = mech_parseattributes(buffer, args, 2);

  if (wc_args < 2) {
    mech_toggle_mode_sub(player, mech, buffer, 1, INARC, 0, 0,
                         "Weapon %d has been set to fire INARC Homing pods",
                         "Weapon %d is already set to fire INARC Homing pods",
                         "That weapon is not an INARC launcher!");
  } else {
    switch (ascii_to_upper(*checked_string_suffix(args[1], 0))) {
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

void mech_explosive(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;

  mech_toggle_mode_sub(player, mech, buffer, 1, NARC, NARC_MODE, 0,
                       "Weapon %d has been set to fire explosive rounds",
                       "Weapon %d has been set to fire NARC beacons",
                       "That weapon cannot be set to fire explosive rounds!");
}

void mech_lbx(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 1, LBX, LBX_MODE, 0,
                       "Weapon %d has been set to LBX fire mode",
                       "Weapon %d has been set to normal fire mode",
                       "That weapon cannot be set LBX!");
}

void mech_armorpiercing(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, AC_AP_MODE, 0,
                       "Weapon %d has been set to fire AP rounds",
                       "Weapon %d has been set to fire normal rounds",
                       "That weapon cannot fire AP rounds!");
}

void mech_caseless(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, AC_CASELESS_MODE, 0,
                       "Weapon %d has been set to fire CASELESS rounds",
                       "Weapon %d has been set to fire normal rounds",
                       "That weapon cannot fire CASELESS rounds!");
}

void mech_flechette(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, AC_FLECHETTE_MODE, 0,
                       "Weapon %d has been set to fire Flechette rounds",
                       "Weapon %d has been set to fire normal rounds",
                       "That weapon cannot fire Flechette rounds!");
}

void mech_incendiary(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, AC_INCENDIARY_MODE, 0,
                       "Weapon %d has been set to fire Incendiary rounds",
                       "Weapon %d has been set to fire normal rounds",
                       "That weapon cannot fire Incendiary rounds!");
}

void mech_precision(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, AC_PRECISION_MODE, 0,
                       "Weapon %d has been set to fire Precision rounds",
                       "Weapon %d has been set to fire normal rounds",
                       "That weapon cannot fire Precision rounds!");
}

void mech_rapidfire(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 1, RFAC, RFAC_MODE, 1,
                       "Weapon %d has been set to Rapid Fire mode",
                       "Weapon %d has been set to normal fire mode",
                       "That weapon cannot be set to do rapid fire!");
}

void mech_stinger(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 2, 0, STINGER_MODE, 0,
                       "Weapon %d has been set to fire stinger missiles.",
                       "Weapon %d has been set to fire normal missiles",
                       "That weapon cannot be set STINGER!");
}

void mech_rac(DbRef player, Mech *mech, char *buffer) {
  int wc_args = 0;
  char *args[2];

  wc_args = mech_parseattributes(buffer, args, 2);

  if (wc_args < 2) {
    mech_toggle_mode_sub(player, mech, buffer, 1, RAC, 0, 1,
                         "Weapon %d has been set to fire one shot at a time.",
                         "Weapon %d is already set to fire one shot at a time.",
                         "That weapon is not a RotaryAC!");
  } else {
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

static bool mech_unjamammo_func(const MultiWeaponSelectionCall *call) {
  Mech *mech = call->mech;
  const DbRef PLAYER = call->actor;
  const int INDEX = call->first;
  int section;
  int critical;
  int weaptype;
  int i;
  char location[50];

  WeaponNumberLookupResult lookup = weapon_number_find(
      &(WeaponNumberLookupRequest){.mech = mech, .number = INDEX});
  weaptype = lookup.value;
  section = lookup.slot.section;
  critical = lookup.slot.critical;
  if (weaptype == -1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "The weapons system chirps: 'Illegal Weapon Number!'");
    return false;
  }
  if (weaptype == -2) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), PLAYER,
        "The weapons system chirps: 'That Weapon has been destroyed!'");
    return false;
  }
  if (mech_critical_temporary_failure(mech, section, critical) !=
      FAIL_AMMOJAMMED) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "The ammo feed mechanism for that weapon is not jammed.");
    return false;
  }
  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "You can't unjam the ammo feed while jumping!");
    return false;
  }
  if (mech_desired_speed(mech) >
      (2.0F * mech_effective_maximum_speed(mech) / 3.0F) + 0.1F) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "You can't unjam the ammo feed while running!");
    return false;
  }

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (mech_section_has_recycling_weapon(mech, i)) {
      armor_string_from_index(i, location, mech_class(mech),
                              mech_movement_type(mech));
      mech_printf(mech, MECHALL, "You have weapons recycling on your %s.",
                  location);
      return false;
    }
  }

  if (mech_event_count(mech, EVENT_UNJAM_AMMO)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "You are already unjamming a weapon!");
    return false;
  }

  mech_event_schedule(mech, EVENT_UNJAM_AMMO, mech_unjam_ammo_event, 60,
                      (long)INDEX);
  mech_printf(mech, MECHALL,
              "You begin to shake the jammed ammo loose on weapon #%d", INDEX);
  return false;
}
void mech_unjamammo(DbRef player, Mech *mech, char *buffer) {
  char *args[1];

  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Please specify a weapon number.");
    return;
  }
  multi_weapon_select(&(MultiWeaponSelectionRequest){
      .mech = mech,
      .actor = player,
      .selection = args[0],
      .mode = 1,
      .callback = mech_unjamammo_func,
  });
}

void mech_gattling(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 1, GMG, GATTLING_MODE, 1,
                       "Weapon %d has been set to Gattling mode",
                       "Weapon %d has been set to normal fire mode",
                       "That weapon cannot be set to do gattling fire!");
}

void mech_artemis(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(
      player, mech, buffer, 6, TMISSILE, ARTEMIS_MODE, 0,
      "Weapon %d has been set to fire Artemis IV compatible missiles.",
      "Weapon %d has been set to fire normal missiles",
      "That weapon cannot be set ARTEMIS!");
}

void mech_narc(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(
      player, mech, buffer, 0, TMISSILE, NARC_MODE, 0,
      "Weapon %d has been set to fire Narc Beacon compatible missiles.",
      "Weapon %d has been set to fire normal missiles",
      "That weapon cannot be set NARC!");
}

void mech_swarm(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 2, 0, SWARM_MODE, 0,
                       "Weapon %d has been set to fire Swarm missiles.",
                       "Weapon %d has been set to fire normal missiles",
                       "That weapon cannot be set to fire Swarm missiles!");
}

void mech_sguided(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 2, 0, SGUIDED_MODE, 0,
                       "Weapon %d has been set to fire Sguided missiles.",
                       "Weapon %d has been set to fire normal missiles",
                       "That weapon cannot be set to fire Sguided missiles!");
}

void mech_atmrange(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(
      player, mech, buffer, 2, TMISSILE, ATM_ER_MODE, 0,
      "Weapon %d has been set to fire Extended Range missiles.",
      "Weapon %d has been set to fire normal missiles.",
      "That weapon cannot be set to fire Extended Range missiles!");
}

void mech_atmexplosive(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(
      player, mech, buffer, 2, TMISSILE, ATM_HE_MODE, 0,
      "Weapon %d has been set to fire High Explosive missiles.",
      "Weapon %d has been set to fire normal missiles.",
      "That weapon cannot be set to fire High Explosive missiles!");
}

void mech_swarm1(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 2, 0, SWARM1_MODE, 0,
                       "Weapon %d has been set to fire Swarm1 missiles.",
                       "Weapon %d has been set to fire normal missiles",
                       "That weapon cannot be set to fire Swarm1 missiles!");
}

void mech_inferno(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 4, 0, INFERNO_MODE, 0,
                       "Weapon %d has been set to fire Inferno missiles.",
                       "Weapon %d has been set to fire normal missiles",
                       "That weapon cannot be set to fire Inferno missiles!");
}

void mech_hotload(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 1, IDF, HOTLOAD_MODE, 1,
                       "Hotloading for weapon %d has been toggled on.",
                       "Hotloading for weapon %d has been toggled off.",
                       "That weapon can not be hotloaded!");
}

void mech_cluster(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 0, TARTILLERY, CLUSTER_MODE, 0,
                       "Weapon %d has been set to fire cluster rounds.",
                       "Weapon %d has been set to fire normal rounds",
                       "Invalid weapon type!");
}

void mech_smoke(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 4, 0, SMOKE_MODE, 0,
                       "Weapon %d has been set to fire smoke rounds.",
                       "Weapon %d has been set to fire normal rounds",
                       "Invalid weapon type!");
}

void mech_mine(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALMO))
    return;
  mech_toggle_mode_sub(player, mech, buffer, 4, 0, MINE_MODE, 0,
                       "Weapon %d has been set to fire mine rounds.",
                       "Weapon %d has been set to fire normal rounds",
                       "Invalid weapon type!");
}
