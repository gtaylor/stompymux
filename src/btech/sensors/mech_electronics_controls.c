#include <string.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_advanced_api.h"
#include "mech_api_types.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"

typedef bool (*MechElectronicToggle)(Mech *mech, bool eccm);

typedef struct MechElectronicModeRequest {
  DbRef player;
  Mech *mech;
  bool has_technology;
  bool eccm;
  MechElectronicToggle toggle;
  const char *online_message;
  const char *offline_message;
  const char *missing_message;
} MechElectronicModeRequest;

static void
mech_electronic_mode_toggle(const MechElectronicModeRequest *request) {
  if (!request->has_technology) {
    mecha_notify(btech_context_evaluation(mech_context(request->mech)),
                 request->player, request->missing_message);
    return;
  }
  mech_notify(request->mech, MECHALL,
              request->toggle(request->mech, request->eccm)
                  ? request->online_message
                  : request->offline_message);
}

void mech_ecm(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (mech_condition_summary(mech).ecm_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your Guardian ECM has been destroyed already!");
    return;
  }
  mech_electronic_mode_toggle(&(MechElectronicModeRequest){
      .player = player,
      .mech = mech,
      .has_technology = (mech_technology_flags(mech) & ECM_TECH) != 0,
      .toggle = mech_ecm_mode_toggle,
      .online_message = "You turn your ECM suite online (ECM mode).",
      .offline_message = "You turn your ECM suite offline.",
      .missing_message = "This unit isn't equipped with an ECM suite!"});
  mark_for_los_update(mech);
}

void mech_eccm(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (mech_condition_summary(mech).ecm_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your Guardian ECM has been destroyed already!");
    return;
  }
  mech_electronic_mode_toggle(&(MechElectronicModeRequest){
      .player = player,
      .mech = mech,
      .has_technology = (mech_technology_flags(mech) & ECM_TECH) != 0,
      .eccm = true,
      .toggle = mech_ecm_mode_toggle,
      .online_message = "You turn your ECM suite online (ECCM mode).",
      .offline_message = "You turn your ECM suite offline.",
      .missing_message = "This unit isn't equipped with an ECM suite!"});
  mark_for_los_update(mech);
}

void mech_perecm(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  mech_electronic_mode_toggle(&(MechElectronicModeRequest){
      .player = player,
      .mech = mech,
      .has_technology = (mech_infantry_technology_flags(mech) &
                         FC_INFILTRATORII_STEALTH_TECH) != 0,
      .toggle = mech_personal_ecm_mode_toggle,
      .online_message = "You turn your Personal ECM suite online (ECM mode).",
      .offline_message = "You turn your Personal ECM suite offline.",
      .missing_message =
          "This unit isn't equipped with a Personal ECM suite!"});
  mark_for_los_update(mech);
}

void mech_pereccm(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  mech_electronic_mode_toggle(&(MechElectronicModeRequest){
      .player = player,
      .mech = mech,
      .has_technology = (mech_infantry_technology_flags(mech) &
                         FC_INFILTRATORII_STEALTH_TECH) != 0,
      .eccm = true,
      .toggle = mech_personal_ecm_mode_toggle,
      .online_message = "You turn your Personal ECM suite online (ECCM mode).",
      .offline_message = "You turn your Personal ECM suite offline.",
      .missing_message =
          "This unit isn't equipped with a Personal ECM suite!"});
  mark_for_los_update(mech);
}

void mech_angelecm(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (mech_condition_summary(mech).angel_ecm_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your Angel ECM has been destroyed already!");
    return;
  }
  mech_electronic_mode_toggle(&(MechElectronicModeRequest){
      .player = player,
      .mech = mech,
      .has_technology =
          (mech_technology_flags_secondary(mech) & ANGEL_ECM_TECH) != 0,
      .toggle = mech_angel_ecm_mode_toggle,
      .online_message = "You turn your Angel ECM suite online (ECM mode).",
      .offline_message = "You turn your Angel ECM suite offline.",
      .missing_message = "This unit isn't equipped with an Angel ECM suite!"});
  mark_for_los_update(mech);
}

void mech_angeleccm(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (mech_condition_summary(mech).angel_ecm_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your Angel ECM has been destroyed already!");
    return;
  }
  mech_electronic_mode_toggle(&(MechElectronicModeRequest){
      .player = player,
      .mech = mech,
      .has_technology =
          (mech_technology_flags_secondary(mech) & ANGEL_ECM_TECH) != 0,
      .eccm = true,
      .toggle = mech_angel_ecm_mode_toggle,
      .online_message = "You turn your Angel ECM suite online (ECCM mode).",
      .offline_message = "You turn your Angel ECM suite offline.",
      .missing_message = "This unit isn't equipped with an Angel ECM suite!"});
  mark_for_los_update(mech);
}

static void mech_searchlight_change_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long w_type = (long)e->data2;

  if (mech_condition_summary(mech).searchlight_destroyed)
    return;

  if (!mech_is_started(mech))
    return;

  if (!mech_is_started(mech)) {
    mech_searchlight_set(mech, false);
    return;
  }

  if (w_type == 1) {
    mech_searchlight_set(mech, true);

    mech_notify(mech, MECHALL, "Your searchlight comes on to full power.");
    mech_los_broadcast(mech, "turns on a searchlight!");
  } else {
    mech_searchlight_set(mech, false);

    mech_notify(mech, MECHALL, "Your searchlight shuts off.");
    mech_los_broadcast(mech, "turns off a searchlight!");
  }

  mark_for_los_update(mech);
}

void mech_slite(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!(mech_technology_flags(mech) & SLITE_TECH)) {
    mech_notify(mech, MECHALL, "Your 'mech isn't equipped with searchlight!");
    return;
  }

  if (mech_condition_summary(mech).searchlight_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your searchlight has been destroyed already!");
    return;
  }

  if (mech_event_count(mech, EVENT_SLITECHANGING)) {
    if (mech_condition_summary(mech).searchlight_on)
      mech_notify(mech, MECHALL,
                  "Your searchlight is already in the process of turning off.");
    else
      mech_notify(mech, MECHALL,
                  "Your searchlight is already in the process of turning on.");

    return;
  }

  if (mech_condition_summary(mech).searchlight_on) {
    mech_notify(mech, MECHALL, "Your searchlight starts to cool down.");
    mech_event_schedule(mech, EVENT_SLITECHANGING,
                        mech_searchlight_change_event, 5, 0);
  } else {
    mech_notify(mech, MECHALL, "Your searchlight starts to warm up.");
    mech_event_schedule(mech, EVENT_SLITECHANGING,
                        mech_searchlight_change_event, 5, 1);
  }
}

static void mech_stealth_armor_change_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long w_type = (long)e->data2;

  if (!mech_is_started(mech))
    return;

  if (!mech_has_working_ecm_suite(mech))
    return;

  if (w_type) {
    mech_notify(mech, MECHALL, "Stealth Armor system engaged!");

    mech_stealth_armor_active_set(mech, true);
    mech_ecm_check(mech);
    mark_for_los_update(mech);
  } else {
    mech_notify(mech, MECHALL, "Stealth Armor system disengaged!");

    mech_stealth_armor_active_set(mech, false);
    mech_ecm_check(mech);
    mark_for_los_update(mech);
  }
}

void mech_stealtharmor(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!(mech_technology_flags_secondary(mech) & STEALTH_ARMOR_TECH)) {
    mech_notify(mech, MECHALL,
                "Your 'mech isn't equipped with a Stealth Armor system!");

    return;
  }

  if (!mech_has_working_ecm_suite(mech)) {
    mech_notify(mech, MECHALL,
                "Your 'mech doesn't have a working Guardian ECM suite!");

    return;
  }

  if (mech_event_count(mech, EVENT_STEALTH_ARMOR)) {
    mech_notify(
        mech, MECHALL,
        "You are already changing the status of your Stealth Armor system!");

    return;
  }

  if (!mech_condition_summary(mech).stealth_armor_active) {
    mech_notify(mech, MECHALL,
                "Your Stealth Armor system begins to come online.");

    mech_event_schedule(mech, EVENT_STEALTH_ARMOR,
                        mech_stealth_armor_change_event, 30, 1);
  } else {
    mech_notify(mech, MECHALL, "Your Stealth Armor system begins to shutdown.");

    mech_event_schedule(mech, EVENT_STEALTH_ARMOR,
                        mech_stealth_armor_change_event, 30, 0);
  }
}

static void mech_null_signature_change_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long w_type = (long)e->data2;

  if (!mech_is_started(mech))
    return;

  if (mech_condition_summary(mech).null_signature_destroyed)
    return;

  if (w_type) {
    mech_notify(mech, MECHALL, "Null Signature System engaged!");

    mech_null_signature_active_set(mech, true);
    mark_for_los_update(mech);
  } else {
    mech_notify(mech, MECHALL, "Null Signature System disengaged!");

    mech_null_signature_active_set(mech, false);
    mark_for_los_update(mech);
  }
}

void mech_nullsig(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!(mech_technology_flags_secondary(mech) & NULLSIGSYS_TECH)) {
    mech_notify(mech, MECHALL,
                "Your 'mech isn't equipped with a Null Signature System!");

    return;
  }

  if (mech_condition_summary(mech).null_signature_destroyed) {
    mech_notify(mech, MECHALL, "Your Null Signature System is destroyed!");

    return;
  }

  if (mech_event_count(mech, EVENT_NSS)) {
    mech_notify(
        mech, MECHALL,
        "You are already changing the status of your Null Signature System!");

    return;
  }

  if (!mech_condition_summary(mech).null_signature_active) {
    mech_notify(mech, MECHALL,
                "Your Null Signature System begins to come online.");

    mech_event_schedule(mech, EVENT_NSS, mech_null_signature_change_event, 30,
                        1);
  } else {
    mech_notify(mech, MECHALL,
                "Your Null Signature System begins to shutdown.");

    mech_event_schedule(mech, EVENT_NSS, mech_null_signature_change_event, 30,
                        0);
  }
}

void show_narc_pods(DbRef player, Mech *mech, char *buffer) {
  char location[50];
  int i;

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!(mech_has_section_special(mech, NARC_ATTACHED) ||
        mech_has_section_special(mech, INARC_HOMING_ATTACHED) ||
        mech_has_section_special(mech, INARC_HAYWIRE_ATTACHED) ||
        mech_has_section_special(mech, INARC_ECM_ATTACHED) ||
        mech_has_section_special(mech, INARC_NEMESIS_ATTACHED))) {

    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "There are no NARC or iNARC pods attached to this unit.");

    return;
  }

  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "=========================Attached NARC and iNARC "
               "Pods========================");
  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "-- Location ---||- NARC -||- iHoming -||- iHaywire -||- iECM "
               "-||- iNemesis --");

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (mech_section_original_internal(mech, i) > 0) {
      armor_string_from_index(i, location, mech_class(mech),
                              mech_movement_type(mech));

      if (mech_section_is_destroyed(mech, i)) {
        notify_printf(btech_context_evaluation(mech_context(mech)), player,
                      " %-14.13s||********||***********||************||********"
                      "||************* ",
                      location);
      } else {
        notify_printf(
            btech_context_evaluation(mech_context(mech)), player,
            " %-14.13s||....%s...||.....%s.....||......%s.....||....%s...||...."
            "..%s...... ",
            location,
            mech_section_has_special(mech, i, NARC_ATTACHED) ? "X" : ".",
            mech_section_has_special(mech, i, INARC_HOMING_ATTACHED) ? "X"
                                                                     : ".",
            mech_section_has_special(mech, i, INARC_HAYWIRE_ATTACHED) ? "X"
                                                                      : ".",
            mech_section_has_special(mech, i, INARC_ECM_ATTACHED) ? "X" : ".",
            mech_section_has_special(mech, i, INARC_NEMESIS_ATTACHED) ? "X"
                                                                      : ".");
      }
    }
  }
}

static int mech_arm_base_to_hit_modifier(Mech *mech, int w_sec) {
  int w_ret = 0;

  if (mech_critical_is_nonfunctional(mech, w_sec, 1) ||
      mech_critical_part_type(mech, w_sec, 1) !=
          special_equipment_index(UPPER_ACTUATOR))
    w_ret += 2;
  if (mech_critical_is_nonfunctional(mech, w_sec, 2) ||
      mech_critical_part_type(mech, w_sec, 2) !=
          special_equipment_index(LOWER_ACTUATOR))
    w_ret += 2;
  if (mech_critical_is_nonfunctional(mech, w_sec, 3) ||
      mech_critical_part_type(mech, w_sec, 3) !=
          special_equipment_index(HAND_OR_FOOT_ACTUATOR))
    w_ret += 1;

  return w_ret;
}

void remove_inarc_pods_mech(DbRef player, Mech *mech, char *buffer) {
  int w_loc;
  int w_arm_to_use = -1;
  char *args[2];
  char str_location[50];
  char str_punch_with[50];
  int w_bth = 0;
  int w_bth_mod_larm = 0;
  int w_bth_mod_rarm = 0;
  int w_ra_avail = 1;
  int w_la_avail = 1;
  int w_roll;
  int w_self_damage;
  int w_pod_type = INARC_HOMING_ATTACHED;
  char str_pod_type[30];

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (mech_movement_type(mech) == MOVE_QUAD) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Quads can not knock of iNARC pods!");
    return;
  }
  if (mech_parseattributes(buffer, args, 2) != 2) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments!");
    return;
  }

  w_loc = armor_section_from_string(mech_class(mech), mech_movement_type(mech),
                                    args[0]);

  if (w_loc == -1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid section!");
    return;
  }
  if (!mech_section_original_internal(mech, w_loc)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid section!");
    return;
  }
  if (!mech_section_internal(mech, w_loc)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That section is destroyed!");
    return;
  }

  armor_string_from_index(w_loc, str_location, mech_class(mech),
                          mech_movement_type(mech));

  /* Figure out wot type of pods we want to remove */
  switch (ascii_to_upper(*checked_string_suffix(args[1], 0))) {
  case 'Y':
    strcpy(str_pod_type, "Haywire");
    w_pod_type = INARC_HAYWIRE_ATTACHED;
    break;

  case 'E':
    strcpy(str_pod_type, "ECM");
    w_pod_type = INARC_ECM_ATTACHED;
    break;

  default:
    strcpy(str_pod_type, "Homing");
    w_pod_type = INARC_HOMING_ATTACHED;
    break;
  }

  if (!mech_section_has_special(mech, w_loc, w_pod_type)) {
    mecha_notifyf(btech_context_evaluation(mech_context(mech)), player,
                  "There are no iNarc %s pods attached to your %s!",
                  str_pod_type, str_location);
    return;
  }

  if (((!mech_section_internal(mech, RARM)) &&
       (!mech_section_internal(mech, LARM)))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You need at least one functioning arm to remove iNarc pods!");
    return;
  }

  if (w_loc == RARM) {
    if (!mech_section_internal(mech, LARM)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Your Left Arm needs to be intact to take "
                   "iNarc pods off your right arm!");
      return;
    }
    if (mech_section_has_recycling_weapon(mech, LARM)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You have weapons recycling on your Left Arm.");
      return;
    }
    if (mech_section_recycle_ticks(mech, LARM)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Your Left Arm is still recovering from your last attack.");
      return;
    }

    w_arm_to_use = LARM;
  }

  if (w_loc == LARM) {
    if (!mech_section_internal(mech, RARM)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Your Right Arm needs to be intact to "
                   "take iNarc pods off your Left Arm!");
      return;
    }
    if (mech_section_has_recycling_weapon(mech, RARM)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You have weapons recycling on your Right Arm.");
      return;
    }
    if (mech_section_recycle_ticks(mech, RARM)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Your Right Arm is still recovering from your last attack.");
      return;
    }

    w_arm_to_use = RARM;
  }

  if (w_arm_to_use == -1) {
    if (mech_section_has_recycling_weapon(mech, RARM) ||
        mech_section_recycle_ticks(mech, RARM) ||
        (!mech_section_internal(mech, RARM)))
      w_ra_avail = 0;

    if (mech_section_has_recycling_weapon(mech, LARM) ||
        mech_section_recycle_ticks(mech, LARM) ||
        (!mech_section_internal(mech, LARM)))
      w_la_avail = 0;

    if (!(w_la_avail || w_ra_avail)) {
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          "You need at least one arm that is not recycling and does not have "
          "weapons recycling in it!");
      return;
    }

    if (!w_la_avail)
      w_bth_mod_larm = 1000;
    else
      w_bth_mod_larm = mech_arm_base_to_hit_modifier(mech, LARM);

    if (!w_ra_avail)
      w_bth_mod_rarm = 1000;
    else
      w_bth_mod_rarm = mech_arm_base_to_hit_modifier(mech, RARM);

    if (w_bth_mod_rarm < w_bth_mod_larm) {
      w_bth = w_bth_mod_rarm;
      w_arm_to_use = RARM;
    } else {
      w_bth = w_bth_mod_larm;
      w_arm_to_use = LARM;
    }
  } else {
    w_bth = mech_arm_base_to_hit_modifier(mech, w_arm_to_use);
  }

  w_bth += find_pilot_piloting(mech) + 4;
  w_roll = btech_random_roll(mech_context(mech));

  armor_string_from_index(w_arm_to_use, str_punch_with, mech_class(mech),
                          mech_movement_type(mech));

  mech_printf(mech, MECHALL,
              "You try to swat at the iNarc pods attached to your %s with your "
              "%s.  BTH:  %d,\tRoll:  %d",
              str_location, str_punch_with, w_bth, w_roll);

  /* Oops, we failed! */
  if (w_roll < w_bth) {
    mech_notify(mech, MECHALL, "Uh oh. You miss the pod and hit yourself!");
    mech_los_broadcast(
        mech, "tries to swat off an iNarc pod, but misses and hits itself!");

    w_self_damage = (mech_tonnage(mech) + (10 / 2)) / 10;

    if (mech_critical_part_type(mech, w_arm_to_use, 2) !=
            special_equipment_index(LOWER_ACTUATOR) ||
        mech_critical_is_nonfunctional(mech, w_arm_to_use, 2))
      w_self_damage = w_self_damage / 2;

    if (mech_critical_part_type(mech, w_arm_to_use, 1) !=
            special_equipment_index(UPPER_ACTUATOR) ||
        mech_critical_is_nonfunctional(mech, w_arm_to_use, 1))
      w_self_damage = w_self_damage / 2;

    mech_damage_apply(
        &(MechDamageRequest){.target = mech,
                             .attacker = mech,
                             .line_of_sight = 1,
                             .attack_pilot = mech_pilot_dbref(mech),
                             .hit_location = w_loc,
                             .rear = 0,
                             .critical = 0,
                             .armor_damage = w_self_damage,
                             .internal_damage = 0,
                             .transfer = MECH_DAMAGE_NORMAL,
                             .cause = -1,
                             .base_to_hit = 0,
                             .weapon_index = -1,
                             .ammunition_mode = 0,
                             .ignore_swarmers = 0});
  } else {
    mech_section_special_remove(mech, w_loc, w_pod_type);

    mech_printf(mech, MECHALL, "You knock a %s pod off your %s!", str_pod_type,
                str_location);
    mech_los_broadcast(mech, "knocks an iNarc pod off itself.");
  }

  mech_set_recycle_limb(mech, w_arm_to_use, PHYSICAL_RECYCLE_TIME);
}

static void mech_inarc_pods_tank_remove_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int i;

  if (mech_is_destroyed(mech))
    return;

  mech_notify(mech, MECHALL, "You remove all the iNARC pods from your unit.");

  mech_los_broadcast(
      mech, "'s crew climbs out and knocks off all the attached iNarc pods!");

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (mech_section_original_internal(mech, i) > 0) {
      mech_section_special_remove(
          mech, i,
          INARC_HOMING_ATTACHED | INARC_HAYWIRE_ATTACHED | INARC_ECM_ATTACHED |
              INARC_NEMESIS_ATTACHED);
    }
  }
}

void remove_inarc_pods_tank(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALSO))
    return;

  if ((mech_desired_speed(mech) > 0)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can not be moving when attempting to remove iNarc pods!");
    return;
  }
  if ((mech_current_speed(mech) > 0)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can not be moving when attempting to remove iNarc pods!");
    return;
  }

  if (mech_class(mech) == CLASS_VTOL) {
    if (!mech_is_landed(mech)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You must land before attempting to remove iNarc pods!");
      return;
    }
  }

  if (mech_event_count(mech, EVENT_UNSTUN_CREW)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're too stunned to remove iNarc pods!");
    return;
  }
  if (mech_event_count(mech, EVENT_UNJAM_TURRET)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're too busy unjamming your turret to remove iNarc pods!");
    return;
  }
  if (mech_event_count(mech, EVENT_UNJAM_AMMO)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're too busy unjamming a weapon to remove iNarc pods!");
    return;
  }

  if (!(mech_has_section_special(mech, INARC_HOMING_ATTACHED) ||
        mech_has_section_special(mech, INARC_HAYWIRE_ATTACHED) ||
        mech_has_section_special(mech, INARC_ECM_ATTACHED) ||
        mech_has_section_special(mech, INARC_NEMESIS_ATTACHED))) {

    mech_notify(mech, MECHALL,
                "There are no iNarc pods attached to this unit.");

    return;
  }

  mech_notify(
      mech, MECHALL,
      "You begin to systematically remove all the iNarc pods from your unit.");

  mech_event_schedule(mech, EVENT_REMOVE_PODS,
                      mech_inarc_pods_tank_remove_event, 60, 0);
}
