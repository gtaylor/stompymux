/* Implements BattleTech combat mechanics for eject embark. */

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "btech/configuration.h"
#include "btech/special_objects.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_lifecycle.h"
#include "mech_template_api.h"
#include "mux/commands/action_messages.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/world/access.h"
#include "mux/world/move.h"
#include "mux/world/object.h"
/* Ejection code */
#include "btech/context.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "eject_api.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_ood_api.h"
#include "mech_position_api.h"
#include "mech_radio_api.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_tech_commands_api.h"
#include "mech_utils_api.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/support/alloc.h"
#include "registry_api.h"
#include "section_types.h"

static void destroy_failed_suit(BtechContext *context,
                                EvaluationContext *evaluation, DbRef suit) {
  btech_object_forget(context, suit);
  destroy_thing(evaluation, suit);
}

void mech_embark(DbRef player, Mech *mech, char *buffer) {
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  Mech *target;
  Mech *towee = nullptr;
  int tmp;
  DbRef target_num;
  int argc;
  char *args[4];
  char fail_mesg[SBUF_SIZE];
  LuaLockInvocation lock;
  LuaLockResult *lock_result = nullptr;

  if (player != GOD)
    if (!common_checks(player, mech, MECH_USUAL))
      return;
  if (mech_class(mech) == CLASS_MW) {
    argc = mech_parseattributes(buffer, args, 1);
    if (argc != 1) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid number of arguements.");
      return;
    }
    target_num = find_target_dbref_from_map_number(mech, args[0]);
    if (target_num == -1) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "That target is not in your line of sight.");
      return;
    }
    target = btech_context_get_mech(mech_context(mech), target_num);
    if (!target ||
        !mech_los_check(mech, target, mech_position_x(target),
                        mech_position_y(target), mech_range_to(mech, target))) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "That target is not in your line of sight.");
      return;
    }
    if (mech_cocoon_integrity(target)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You should wait for your target to land first");
      return;
    }
    if (mech_position_z(mech) > (mech_position_z(target) + 1)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You are too high above the target.");
      return;
    }
    if (mech_position_z(mech) < (mech_position_z(target) - 1)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You can't reach that high !");
      return;
    }
    if (mech_position_x(mech) != mech_position_x(target) ||
        mech_position_y(mech) != mech_position_y(target)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You need to be in the same hex!");
      return;
    }
    if ((!is_in_character(btech_context_database(mech_context(mech)),
                          mech_dbref(mech))) ||
        (!is_in_character(btech_context_database(mech_context(mech)),
                          mech_dbref(target)))) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You don't really see a way to get in there.");
      return;
    }
    if ((mech_class(target) == CLASS_VEH_GROUND ||
         mech_class(target) == CLASS_VTOL) &&
        !unit_is_fixable(target)) {
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          "You can't find and entrance amid the mass of twisted metal.");
      return;
    }

    lock_result = checked_storage_allocate(sizeof(*lock_result));
    if (!lock_test(evaluation, player, player, mech_dbref(mech),
                   mech_dbref(target), LUA_LOCK_ENTER, false, &lock,
                   lock_result)) {
      /* Trigger FAIL & AFAIL */
      memset(fail_mesg, 0, sizeof(fail_mesg));
      (void)snprintf(fail_mesg, SBUF_SIZE, "That unit's bay doors are locked.");

      notify_lock_failure(
          &(LockFailureNotification){.evaluation = evaluation,
                                     .invocation = &lock,
                                     .result = lock_result,
                                     .enactor_default = fail_mesg,
                                     .event = LUA_EVENT_FAIL});

      goto cleanup_mw;
    }

    if (!lock_result->defined) {
      /* Check their teams */
      if (mech_team(mech) != mech_team(target)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Locked. Damn !");
        goto cleanup_mw;
      }
    }

    if (fabsf(mech_current_speed(target)) > 15.0F) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Are you suicidal ? That thing is moving too fast !");
      goto cleanup_mw;
    }

    if (mech_class(target) == CLASS_MECH) {
      if (!mech_section_internal(target, HEAD)) {
        mecha_notify(
            btech_context_evaluation(mech_context(mech)), player,
            "Okay, just climb up to-- Wait... where did the head go??");
        goto cleanup_mw;
      }
      if (mech_critical_is_destroyed(target, HEAD, 2)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Okay, just climb up and open-- "
                     "WTF ? Someone stole the cockpit!");
        goto cleanup_mw;
      }
      if (mech_critical_is_nonfunctional(target, HEAD, 2)) {
        mecha_notify(
            btech_context_evaluation(mech_context(mech)), player,
            "Okay, just climb up and open-- hey, this door won't budge!");
        goto cleanup_mw;
      }
    }
    mech_printf(mech, MECHALL, "You climb into %s.",
                mech_display_id(target).text);
    mech_los_broadcastf(mech, "climbs into %s.", mech_display_id(target).text);
    contents_teleport(&(ContentsTeleportRequest){
        .context = mech_context(mech),
        .source = mech_dbref(mech),
        .destination = mech_dbref(target),
        .options = TELE_ALL,
    });
    discard_mw(mech);
  cleanup_mw:
    free_buf(lock_result);
    return;
  }
  /* What heppens with a Bsuit squad? */
  /* Check if the vechile has cargo capacity, or is an Omni Mech */
  argc = mech_parseattributes(buffer, args, 1);
  if (argc != 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguements.");
    return;
  }
  target_num = find_target_dbref_from_map_number(mech, args[0]);
  if (target_num == -1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That target is not in your line of sight.");
    return;
  }
  target = btech_context_get_mech(mech_context(mech), target_num);
  if (!target ||
      !mech_los_check(mech, target, mech_position_x(target),
                      mech_position_y(target), mech_range_to(mech, target))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That target is not in your line of sight.");
    return;
  }
  if (mech_carried_dbref(mech) == target_num) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot embark what your towing!");
    return;
  }
  if (mech_condition_summary(mech).fallen ||
      mech_event_count(mech, EVENT_STAND)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Help! I've fallen and I can't get up!");
    return;
  }
  if (!mech_is_started(mech) || mech_is_destroyed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Ha Ha Ha.");
    return;
  }
  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot do that while jumping!");
    return;
  }
  if (mech_is_jumping(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot do that while it is jumping!");
    return;
  }
  if (mech_technology_flags_secondary(mech) & CARRIER_TECH &&
      (mech_is_dropship(target) ? mech_is_dropship(mech) : 1)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're a bit bulky to do that yourself.");
    return;
  }
  if (mech_condition_summary(mech).hidden) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot embark while hidden.");
    return;
  }
  if (mech_tonnage(mech) > mech_carrier_maximum_tonnage(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are too large for that class of carrier.");
    return;
  }
  if (mech_class(mech) != CLASS_BSUIT &&
      !(mech_technology_flags_secondary(target) & CARRIER_TECH)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit can't handle your mass.");
    return;
  }
  if (mech_maximum_speed(mech) < MP1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are to overloaded to enter.");
    return;
  }
  if (mech_position_z(mech) > (mech_position_z(target) + 1)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are too high above the target.");
    return;
  }
  if (mech_position_z(mech) < (mech_position_z(target) - 1)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't reach that high !");
    return;
  }
  if (mech_position_x(mech) != mech_position_x(target) ||
      mech_position_y(mech) != mech_position_y(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You need to be in the same hex!");
    return;
  }

  lock_result = checked_storage_allocate(sizeof(*lock_result));
  if (!lock_test(evaluation, player, player, mech_dbref(mech),
                 mech_dbref(target), LUA_LOCK_ENTER, false, &lock,
                 lock_result)) {
    /* Trigger FAIL & AFAIL */
    memset(fail_mesg, 0, sizeof(fail_mesg));
    (void)snprintf(fail_mesg, SBUF_SIZE, "That unit's bay doors are locked.");

    notify_lock_failure(&(LockFailureNotification){.evaluation = evaluation,
                                                   .invocation = &lock,
                                                   .result = lock_result,
                                                   .enactor_default = fail_mesg,
                                                   .event = LUA_EVENT_FAIL});

    goto cleanup_embark;
  }

  if (!lock_result->defined) {
    /* Check their teams */
    if (mech_team(mech) != mech_team(target)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Locked. Damn !");
      goto cleanup_embark;
    }
  }

  if (fabsf(mech_current_speed(target)) > 0.0F) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Are you suicidal ? That thing is moving too fast !");
    goto cleanup_embark;
  }
  if (!is_in_character(btech_context_database(mech_context(mech)),
                       mech_dbref(mech)) ||
      !is_in_character(btech_context_database(mech_context(mech)),
                       mech_dbref(target))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You don't really see a way to get in there.");
    goto cleanup_embark;
  }

  /* New message system for when someone tries to embark
   * but their sections are still cycling (or weapons) */
  tmp = mech_recycling_state(mech, CHECK_BOTH);
  if (tmp) {
    if (tmp == 1) {
      mecha_notify(evaluation, player, "You have weapons recycling!");
    } else if (tmp == 2) {
      mecha_notify(evaluation, player,
                   "You are still recovering from your previous action!");
    } else {
      mecha_notify(evaluation, player, "error");
    }
    goto cleanup_embark;
  }

  if ((mech_tonnage(mech) * 100) > mech_cargo_space(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Not enough cargospace for you!");
    goto cleanup_embark;
  }
  if (mech_carried_dbref(mech) > 0) {
    towee =
        btech_context_get_mech(mech_context(mech), mech_carried_dbref(mech));
    if (!towee) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Internal error caused by towed unit! Contact a wizard!");
      goto cleanup_embark;
    }
    if (mech_tonnage(towee) > mech_carrier_maximum_tonnage(target)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Your towed unit is  too large for that class of carrier.");
      goto cleanup_embark;
    }
    if (((mech_tonnage(mech) + mech_tonnage(towee)) * 100) >
        mech_cargo_space(target)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Not enough cargospace for you and your towed unit!");
      goto cleanup_embark;
    }
  }
  const bool HAS_TOWEE =
      (towee != nullptr && mech_carried_dbref(mech) > 0) != 0;
  bool moved;
  if (HAS_TOWEE) {
    const ObjectMovementRequest MOVEMENTS[] = {
        {.evaluation = evaluation,
         .object = mech_dbref(towee),
         .destination = mech_dbref(target),
         .cause = 1},
        {.evaluation = evaluation,
         .object = mech_dbref(mech),
         .destination = mech_dbref(target),
         .cause = 1},
    };
    moved = move_via_teleport_batch(&(ObjectTeleportBatchRequest){
        .movements = MOVEMENTS,
        .count = sizeof(MOVEMENTS) / sizeof(*MOVEMENTS)});
  } else {
    moved = move_via_teleport(
        &(ObjectMovementRequest){.evaluation = evaluation,
                                 .object = mech_dbref(mech),
                                 .destination = mech_dbref(target),
                                 .cause = 1});
  }
  if (!moved) {
    mech_notify(mech, MECHALL, "Unable to embark: teleportation was denied.");
    goto cleanup_embark;
  }
  if (mech_class(mech) == CLASS_BSUIT) {
    mech_printf(mech, MECHALL, "You climb into %s.",
                mech_display_id(target).text);
    mech_los_broadcastf(mech, "climbs into %s.", mech_display_id(target).text);
  } else {
    mech_printf(mech, MECHALL, "You climb up the entry ramp into %s.",
                mech_display_id(target).text);
    mech_los_broadcastf(mech, "climbs up the entry ramp into %s.",
                        mech_display_id(target).text);
    if (towee && mech_carried_dbref(mech) > 0) {
      mech_printf(towee, MECHALL, "You are drug up the entry ramp into %s.",
                  mech_display_id(target).text);
      mech_los_broadcastf(towee, "is drug up the entry ramp into %s.",
                          mech_display_id(target).text);
    }
  }
  mark_for_los_update(mech);
  mark_for_los_update(target);

  if (mech_condition_summary(target).hidden) {
    mech_hidden_set(target, false);
    mech_los_broadcast(target, "becomes visible as it is embarked into.");
  }

  /* Check if the unit is towing something so the towed unit
   * is handled first because mech_power_down() will cause it to drop
   * whatever its towing */
  if (towee && mech_carried_dbref(mech) > 0) {
    mark_for_los_update(towee);
    if (mech_map_index_set(towee, -1, nullptr) != MECH_MAP_SET_OK) {
      mech_notify(mech, MECHALL, "Unable to embark: unit placement failed.");
      goto cleanup_embark;
    }
    mech_cargo_space_remove(target, mech_tonnage(towee) * 100);
    mech_power_down(towee);
    mech_carried_dbref_set(mech, -1);
    mech_towed_clear(towee);
  }

  /* Now handle the unit itself */
  if (mech_map_index_set(mech, -1, nullptr) != MECH_MAP_SET_OK) {
    mech_notify(mech, MECHALL, "Unable to embark: unit placement failed.");
    goto cleanup_embark;
  }
  mech_cargo_space_remove(target, mech_tonnage(mech) * 100);
  mech_power_down(mech);

  mech_speed_correct(target);
cleanup_embark:
  free_buf(lock_result);
}

void autoeject(DbRef player, Mech *mech, int t_is_b_suit) {
  char message_buffer[MBUF_SIZE];
  Mech *m;
  DbRef suit;
  const char *d;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  GameDatabase *database = btech_context_database(mech_context(mech));

  /* If we're not IC, return */
  if (!player || !is_in_character(database, mech_dbref(mech)) ||
      !btech_context_in_character_enabled(mech_context(mech)) ||
      !is_in_character(database,
                       game_object_location(database, mech_dbref(mech))))
    return;

  (void)snprintf(message_buffer, sizeof(message_buffer), "MechWarrior - %s",
                 game_object_name(database, player));
  /* Create the MW object */
  suit = create_obj(evaluation, GOD, OBJECT_TYPE_THING, message_buffer);
  char registration_error[128];
  if (!btech_special_object_register(mech_context(mech), GOD, suit, "MECH",
                                     registration_error,
                                     sizeof(registration_error))) {
    destroy_thing(evaluation, suit);
    mecha_notify(evaluation, player, registration_error);
    return;
  }
  d = btech_player_mechwarrior_template(mech_context(mech), player);
  m = btech_context_get_mech(mech_context(mech), suit);
  if (!m) {
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS,
                       "Unable to create special obj for #%ld's ejection.",
                       player);
    destroy_failed_suit(mech_context(mech), evaluation, suit);
    mecha_notify(evaluation, player,
                 "Sorry, something serious went wrong, contact a Wizard "
                 "(can't create RS object)");
    return;
  }
  if (!mech_template_load(
          GOD, m, (!d || !*d || !strcmp(d, "#-1")) ? "MechWarrior" : d)) {
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS,
        "Unable to load mechwarrior template for #%ld's ejection. (%s)", player,
        (!d || !*d) ? "Default template" : d);
    destroy_failed_suit(mech_context(mech), evaluation, suit);
    mecha_notify(evaluation, player,
                 "Sorry, something serious went wrong, contact a Wizard "
                 "(can't load MWTemplate)");
    return;
  }
  (void)btech_unit_display_name_set(mech_context(mech), suit, "MechWarrior");
  mech_team_set(m, mech_team(mech));
  if (mech_map_index_set(m, mech_map_dbref(mech), nullptr) != MECH_MAP_SET_OK ||
      !mech_position_set(&(MechPositionSetRequest){
          .mech = m, .x = mech_position_x(mech), .y = mech_position_y(mech)})) {
    destroy_failed_suit(mech_context(mech), evaluation, suit);
    mecha_notify(evaluation, player,
                 "Unable to eject because unit placement failed.");
    return;
  }
  const ObjectMovementRequest MOVEMENTS[] = {
      {.evaluation = evaluation,
       .object = suit,
       .destination = mech_map_dbref(mech),
       .cause = 1,
       .hush = 7},
      {.evaluation = evaluation,
       .object = player,
       .destination = suit,
       .cause = 1,
       .hush = 7},
  };
  if (!move_via_teleport_batch(&(ObjectTeleportBatchRequest){
          .movements = MOVEMENTS,
          .count = sizeof(MOVEMENTS) / sizeof(*MOVEMENTS)})) {
    destroy_failed_suit(mech_context(mech), evaluation, suit);
    mecha_notify(evaluation, player,
                 "Unable to eject because teleportation was denied.");
    return;
  }
  /* Init the sucker */
  s_in_character(database, suit);
  initialize_pc(player, m);
  mech_pilot_dbref_set(m, player);
  mech_team_set(m, mech_team(mech));
  mech_radio_frequency_set(
      m, 0, (int)(btech_context_random_i31(mech_context(m)) % 1000000));
  mecha_notifyf(evaluation, player, "Emergency radio channel set to %d.",
                mech_radio_frequency(m, 0));
  /* #endif
  #endif
  */

  if (t_is_b_suit) {
    mech_los_broadcast(m, "climbs out of one of the destroyed suits!");
    mecha_notify(evaluation, player, "You climb out of the unit!");
  } else {
    mech_los_broadcastf(m, "ejected from %s!", mech_display_id(mech).text);
    (void)snprintf(message_buffer, sizeof(message_buffer), "%d %d %d",
                   mech_position_x(m), mech_position_y(m), 150);
    mech_ood_initiate(player, m, message_buffer);
    mecha_notify(evaluation, player, "You eject from the unit!");
  }
}
