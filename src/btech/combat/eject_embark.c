/* Implements BattleTech combat mechanics for eject embark. */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"

void mech_embark(DbRef player, void *data, char *buffer) {

  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  Mech *target, *towee = nullptr;
  int tmp;
  DbRef target_num;
  int argc;
  char *args[4];
  char fail_mesg[SBUF_SIZE];
  LuaLockInvocation lock;
  LuaLockResult lock_result;

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

    if (!lock_test(evaluation, player, player, mech_dbref(mech),
                   mech_dbref(target), LUA_LOCK_ENTER,
                   LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock,
                   &lock_result)) {

      /* Trigger FAIL & AFAIL */
      memset(fail_mesg, 0, sizeof(fail_mesg));
      (void)snprintf(fail_mesg, SBUF_SIZE, "That unit's bay doors are locked.");

      notify_lock_failure(
          &(LockFailureNotification){.evaluation = evaluation,
                                     .invocation = &lock,
                                     .result = &lock_result,
                                     .enactor_default = fail_mesg,
                                     .event = LUA_EVENT_FAIL});

      return;
    }

    if (!lock_result.defined) {

      /* Check their teams */
      if (mech_team(mech) != mech_team(target)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Locked. Damn !");
        return;
      }
    }

    if (fabsf(mech_current_speed(target)) > 15.0F) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Are you suicidal ? That thing is moving too fast !");
      return;
    }

    if (mech_class(target) == CLASS_MECH) {
      if (!mech_section_internal(target, HEAD)) {
        mecha_notify(
            btech_context_evaluation(mech_context(mech)), player,
            "Okay, just climb up to-- Wait... where did the head go??");
        return;
      }
      if (mech_critical_is_destroyed(target, HEAD, 2)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Okay, just climb up and open-- "
                     "WTF ? Someone stole the cockpit!");
        return;
      }
      if (mech_critical_is_nonfunctional(target, HEAD, 2)) {
        mecha_notify(
            btech_context_evaluation(mech_context(mech)), player,
            "Okay, just climb up and open-- hey, this door won't budge!");
        return;
      }
    }
    mech_notify(mech, MECHALL,
                tprintf("You climb into %s.", mech_display_id(target).text));
    mech_los_broadcast(
        mech, tprintf("climbs into %s.", mech_display_id(target).text));
    contents_teleport(&(ContentsTeleportRequest){
        .context = mech_context(mech),
        .source = mech_dbref(mech),
        .destination = mech_dbref(target),
        .options = TELE_ALL,
    });
    discard_mw(mech);
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

  if (!lock_test(evaluation, player, player, mech_dbref(mech),
                 mech_dbref(target), LUA_LOCK_ENTER,
                 LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock, &lock_result)) {

    /* Trigger FAIL & AFAIL */
    memset(fail_mesg, 0, sizeof(fail_mesg));
    (void)snprintf(fail_mesg, SBUF_SIZE, "That unit's bay doors are locked.");

    notify_lock_failure(&(LockFailureNotification){.evaluation = evaluation,
                                                   .invocation = &lock,
                                                   .result = &lock_result,
                                                   .enactor_default = fail_mesg,
                                                   .event = LUA_EVENT_FAIL});

    return;
  }

  if (!lock_result.defined) {

    /* Check their teams */
    if (mech_team(mech) != mech_team(target)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Locked. Damn !");
      return;
    }
  }

  if (fabsf(mech_current_speed(target)) > 0.0F) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Are you suicidal ? That thing is moving too fast !");
    return;
  }
  if (!is_in_character(btech_context_database(mech_context(mech)),
                       mech_dbref(mech)) ||
      !is_in_character(btech_context_database(mech_context(mech)),
                       mech_dbref(target))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You don't really see a way to get in there.");
    return;
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
    return;
  }

  if ((mech_tonnage(mech) * 100) > mech_cargo_space(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Not enough cargospace for you!");
    return;
  }
  if (mech_carried_dbref(mech) > 0) {
    towee =
        btech_context_get_mech(mech_context(mech), mech_carried_dbref(mech));
    if (!towee) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Internal error caused by towed unit! Contact a wizard!");
      return;
    }
    if (mech_tonnage(towee) > mech_carrier_maximum_tonnage(target)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Your towed unit is  too large for that class of carrier.");
      return;
    }
    if (((mech_tonnage(mech) + mech_tonnage(towee)) * 100) >
        mech_cargo_space(target)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Not enough cargospace for you and your towed unit!");
      return;
    }
  }
  if (mech_class(mech) == CLASS_BSUIT) {
    mech_notify(mech, MECHALL,
                tprintf("You climb into %s.", mech_display_id(target).text));
    mech_los_broadcast(
        mech, tprintf("climbs into %s.", mech_display_id(target).text));
  } else {
    mech_notify(mech, MECHALL,
                tprintf("You climb up the entry ramp into %s.",
                        mech_display_id(target).text));
    mech_los_broadcast(mech, tprintf("climbs up the entry ramp into %s.",
                                     mech_display_id(target).text));
    if (towee && mech_carried_dbref(mech) > 0) {
      mech_notify(towee, MECHALL,
                  tprintf("You are drug up the entry ramp into %s.",
                          mech_display_id(target).text));
      mech_los_broadcast(towee, tprintf("is drug up the entry ramp into %s.",
                                        mech_display_id(target).text));
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
    mech_rsetmapindex(GOD, (void *)towee, tprintf("%d", (-1)));
    mech_rsetxy(GOD, (void *)towee, tprintf("%d %d", 0, 0));
    move_via_teleport(
        &(ObjectMovementRequest){.evaluation = evaluation,
                                 .object = mech_dbref(towee),
                                 .destination = mech_dbref(target),
                                 .cause = 1});
    mech_cargo_space_remove(target, mech_tonnage(towee) * 100);
    mech_power_down(towee);
    mech_carried_dbref_set(mech, -1);
    mech_towed_clear(towee);
  }

  /* Now handle the unit itself */
  mech_rsetmapindex(GOD, (void *)mech, tprintf("%d", (-1)));
  mech_rsetxy(GOD, (void *)mech, tprintf("%d %d", 0, 0));
  move_via_teleport(&(ObjectMovementRequest){.evaluation = evaluation,
                                             .object = mech_dbref(mech),
                                             .destination = mech_dbref(target),
                                             .cause = 1});
  mech_cargo_space_remove(target, mech_tonnage(mech) * 100);
  mech_power_down(mech);

  mech_speed_correct(target);
}

void autoeject(DbRef player, Mech *mech, int t_is_b_suit) {
  Mech *m;
  DbRef suit;
  char *d;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  GameDatabase *database = btech_context_database(mech_context(mech));

  /* If we're not IC, return */
  if (!player || !is_in_character(database, mech_dbref(mech)) ||
      !btech_context_in_character_enabled(mech_context(mech)) ||
      !is_in_character(database,
                       game_object_location(database, mech_dbref(mech))))
    return;

  /* Create the MW object */
  suit = create_obj(
      evaluation, GOD, OBJECT_TYPE_THING,
      tprintf("MechWarrior - %s", game_object_name(database, player)));
  silly_atr_set_in(database, suit, A_XTYPE, "MECH");
  s_xcode(database, suit);
  btech_special_object_flag_changed(mech_context(mech), GOD, suit, 0, 1);
  d = btech_attribute_read(database, player, A_MWTEMPLATE,
                           (char[LBUF_SIZE]){0});
  m = btech_context_get_mech(mech_context(mech), suit);
  if (!m) {
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Unable to create special obj for #%ld's ejection.", player));
    destroy_thing(evaluation, suit);
    mecha_notify(evaluation, player,
                 "Sorry, something serious went wrong, contact a Wizard "
                 "(can't create RS object)");
    return;
  }
  if (!mech_template_load(
          GOD, m, (!d || !*d || !strcmp(d, "#-1")) ? "MechWarrior" : d)) {
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Unable to load mechwarrior template for #%ld's ejection. (%s)",
                player, (!d || !*d) ? "Default template" : d));
    destroy_thing(evaluation, suit);
    mecha_notify(evaluation, player,
                 "Sorry, something serious went wrong, contact a Wizard "
                 "(can't load MWTemplate)");
    return;
  }
  silly_atr_set_in(database, suit, A_MECHNAME, "MechWarrior");
  mech_team_set(m, mech_team(mech));
  mech_rsetmapindex(GOD, (void *)m, tprintf("%ld", mech_map_dbref(mech)));
  mech_rsetxy(GOD, (void *)m,
              tprintf("%d %d", mech_position_x(mech), mech_position_y(mech)));
  mech_rsetteam(GOD, (void *)m, tprintf("%d", mech_team(mech)));

  /* Tele the MW to the map and player to the MW */
  move_via_teleport(
      &(ObjectMovementRequest){.evaluation = evaluation,
                               .object = suit,
                               .destination = mech_map_dbref(mech),
                               .cause = 1,
                               .hush = 7});
  move_via_teleport(&(ObjectMovementRequest){.evaluation = evaluation,
                                             .object = player,
                                             .destination = suit,
                                             .cause = 1,
                                             .hush = 7});

  /* Init the sucker */
  s_in_character(database, suit);
  initialize_pc(player, m);
  mech_pilot_dbref_set(m, player);
  mech_team_set(m, mech_team(mech));
  /* MUDCONF THIS LATER (and fix not copying digital)
  #ifdef COPY_CHANS_ON_EJECT
          memcpy(m->freq, mech->freq, FREQS * sizeof(m->freq[0]));
          memcpy(m->freqmodes, mech->freqmodes, FREQS *
  sizeof(m->freqmodes[0])); #else #ifdef RANDOM_CHAN_ON_EJECT
  */
  mech_radio_frequency_set(m, 0, random() % 1000000);
  mecha_notify(evaluation, player,
               tprintf("Emergency radio channel set to %d.",
                       mech_radio_frequency(m, 0)));
  /* #endif
  #endif
  */

  if (t_is_b_suit) {
    mech_los_broadcast(m, "climbs out of one of the destroyed suits!");
    mecha_notify(evaluation, player, "You climb out of the unit!");
  } else {
    mech_los_broadcast(m,
                       tprintf("ejected from %s!", mech_display_id(mech).text));
    mech_ood_initiate(
        player, m,
        tprintf("%d %d %d", mech_position_x(m), mech_position_y(m), 150));
    mecha_notify(evaluation, player, "You eject from the unit!");
  }
}
