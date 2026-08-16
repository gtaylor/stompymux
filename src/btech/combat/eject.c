/* Implements BattleTech combat mechanics for eject. */

#include <math.h>
#include <stdio.h>
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
#include "autopilot.h"
#include "autopilot_resume_api.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "econ_cmds_api.h"
#include "eject_api.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_ood_api.h"
#include "mech_position_api.h"
#include "mech_radio_api.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_tech_api.h"
#include "mech_utils_api.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/support/alloc.h"
#include "registry_api.h"
#include "section_types.h"

int contents_teleport(const ContentsTeleportRequest *request) {
  DbRef i;
  DbRef tmpnext;
  int count = 0;
  EvaluationContext *evaluation = btech_context_evaluation(request->context);
  GameDatabase *database = btech_context_database(request->context);

  SAFE_DOLIST(database, i, tmpnext,
              game_object_contents(database, request->source))
  if ((request->options & TELE_ALL) || !is_wizard(database, i)) {
    if (move_via_teleport(&(ObjectMovementRequest){
            .evaluation = evaluation,
            .object = i,
            .destination = request->destination,
            .cause = 1,
            .hush = request->options & TELE_LOUD ? 0 : 7})) {
      if (request->options & TELE_XP && !is_wizard(database, i)) {
        character_experience_reduce(&(CharacterExperienceReduction){
            .context = request->context,
            .character = i,
            .per_mille = btech_context_experience_loss(request->context),
        });
      }
      count++;
    }
  }
  return count;
}

/* Delayed blast event, for various reasons */
static void mech_discard_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  GameDatabase *database = btech_context_database(mech_context(mech));
  DbRef i = mech_dbref(mech);

  /* We'll silently move the mech off, but lets trigger the aleave/oleave/leave
   * of the loc as well before we do anything fancy */
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_LEAVE,
                                .operation = LUA_MESSAGE_OPERATION_MOVE,
                                .object = game_object_location(database, i),
                                .enactor = i,
                                .cause = i,
                                .source = game_object_location(database, i),
                                .destination = NOTHING},
                    .event = LUA_EVENT_LEAVE});
  c_xcode(database, i);
  btech_special_object_flag_changed(mech_context(mech), GOD, i, true, false);
  s_going(database, i);
  s_dark(database, i);
  s_zombie(database, i);
  (void)move_via_teleport(&(ObjectMovementRequest){
      .evaluation = evaluation,
      .object = i,
      .destination = btech_context_used_mech_store_dbref(mech_context(mech)),
      .cause = 1,
      .hush = 7});
}

static bool move_player_into_unit(EvaluationContext *evaluation, DbRef player,
                                  DbRef unit, DbRef destination) {
  const ObjectMovementRequest MOVEMENTS[] = {
      {.evaluation = evaluation,
       .object = unit,
       .destination = destination,
       .cause = 1,
       .hush = 7},
      {.evaluation = evaluation,
       .object = player,
       .destination = unit,
       .cause = 1,
       .hush = 7},
  };
  return move_via_teleport_batch(&(ObjectTeleportBatchRequest){
      .movements = MOVEMENTS, .count = sizeof(MOVEMENTS) / sizeof(*MOVEMENTS)});
}

static void destroy_failed_suit(BtechContext *context,
                                EvaluationContext *evaluation, DbRef suit) {
  btech_special_object_flag_changed(context, GOD, suit, true, false);
  destroy_thing(evaluation, suit);
}

void discard_mw(Mech *mech) {
  if (is_in_character(btech_context_database(mech_context(mech)),
                      mech_dbref(mech)))
    mech_event_schedule(mech, EVENT_NUKEMECH, mech_discard_event, 10, 0);
}

void enter_mw_bay(Mech *mech, DbRef bay) {
  contents_teleport(&(ContentsTeleportRequest){
      .context = mech_context(mech),
      .source = mech_dbref(mech),
      .destination = bay,
      .options = TELE_ALL,
  }); /* Even immortals must get going */
  discard_mw(mech);
}

void pickup_mw(Mech *mech, Mech *target) {
  DbRef mw;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));

  mw = game_object_contents(btech_context_database(mech_context(mech)),
                            mech_dbref(target));
  if ((mech_class(mech) != CLASS_MECH) &&
      (mech_class(mech) != CLASS_VEH_GROUND) &&
      (mech_class(mech) != CLASS_VTOL) &&
      !(mech_technology_flags(mech) & SALVAGE_TECH)) {
    mech_notify(mech, MECHALL, "You can't pick up, period.");
    return;
  }
  if (mw > 0)
    notify_printf(evaluation, mw,
                  "%s scoops you up and brings you into the cockpit.",
                  mech_to_mech_display_id(target, mech).text);
  /* Put the player in the picker uppper and clear him from the map */
  mech_los_broadcastf(mech, "picks up %s.", mech_display_id(target).text);
  mech_printf(mech, MECHALL,
              "You pick up the stray mechwarrior from the field.");
  if (mech_team(target) != mech_team(mech)) {
    if (btech_context_mechwarrior_pickup_triggers_actions(mech_context(mech))) {
      contents_teleport(&(ContentsTeleportRequest){
          .context = mech_context(mech),
          .source = mech_dbref(target),
          .destination = mech_dbref(mech),
          .options = TELE_ALL | TELE_LOUD,
      });
    } else {
      contents_teleport(&(ContentsTeleportRequest){
          .context = mech_context(mech),
          .source = mech_dbref(target),
          .destination = mech_dbref(mech),
          .options = TELE_ALL,
      });
    }
  } else if (btech_context_mechwarrior_pickup_triggers_actions(
                 mech_context(mech))) {
    contents_teleport(&(ContentsTeleportRequest){
        .context = mech_context(mech),
        .source = mech_dbref(target),
        .destination = mech_dbref(mech),
        .options = TELE_ALL | TELE_LOUD,
    });
  } else {
    contents_teleport(&(ContentsTeleportRequest){
        .context = mech_context(mech),
        .source = mech_dbref(target),
        .destination = mech_dbref(mech),
        .options = TELE_ALL,
    });
  }
  discard_mw(target);
}

static void char_eject(DbRef player, Mech *mech) {
  char message_buffer[MBUF_SIZE];
  Mech *m;
  DbRef suit;
  char *d;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  GameDatabase *database = btech_context_database(mech_context(mech));

  (void)snprintf(message_buffer, sizeof(message_buffer), "MechWarrior - %s",
                 game_object_name(database, player));
  suit = create_obj(evaluation, GOD, OBJECT_TYPE_THING, message_buffer);
  silly_atr_set_in(database, suit, A_XTYPE, "MECH");
  s_xcode(database, suit);
  btech_special_object_flag_changed(mech_context(mech), GOD, suit, false, true);
  d = btech_attribute_read(database, player, A_MWTEMPLATE,
                           (char[LBUF_SIZE]){0});
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
  silly_atr_set_in(database, suit, A_MECHNAME, "MechWarrior");
  mech_team_set(m, mech_team(mech));
  if (mech_map_index_set(m, mech_map_dbref(mech), nullptr) != MECH_MAP_SET_OK ||
      !mech_position_set(&(MechPositionSetRequest){
          .mech = m, .x = mech_position_x(mech), .y = mech_position_y(mech)})) {
    destroy_failed_suit(mech_context(mech), evaluation, suit);
    mecha_notify(evaluation, player,
                 "Unable to eject because unit placement failed.");
    return;
  }
  if (!move_player_into_unit(evaluation, player, suit, mech_map_dbref(mech))) {
    destroy_failed_suit(mech_context(mech), evaluation, suit);
    mecha_notify(evaluation, player,
                 "Unable to eject because teleportation was denied.");
    return;
  }
  mech_los_broadcastf(m, "ejected from %s!", mech_display_id(mech).text);
  s_in_character(database, suit);
  initialize_pc(player, m);
  (void)snprintf(message_buffer, sizeof(message_buffer), "#%ld", player);
  silly_atr_set_in(database, mech_dbref(m), A_PILOTNUM, message_buffer);
  mech_pilot_dbref_set(m, player);
  mech_team_set(m, mech_team(mech));
  mech_radio_frequency_set(
      m, 0, (int)(btech_context_random_i31(mech_context(m)) % 1000000));
  notify_printf(evaluation, player, "Emergency radio channel set to %d.",
                mech_radio_frequency(m, 0));
  /* #endif
  #endif
  */
  mecha_notify(evaluation, player, "You eject from the unit!");
  if (mech_class(mech) == CLASS_MECH) {
    mech_critical_destroy(mech, HEAD, 2);
  }
  if (!mech_is_destroyed(mech)) {
    mech_destroy(mech, mech, false, KILL_TYPE_EJECT);
  }
}

void mech_eject(DbRef player, void *data, char *buffer [[maybe_unused]]) {
  Mech *mech = data;

  if (!common_checks(player, mech, MECH_USUALS))
    return;
  if (mech_is_dropship(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Dropships do not support ejection.");
    return;
  }
  if (!((mech_class(mech) == CLASS_MECH) || (mech_class(mech) == CLASS_VTOL) ||
        (mech_class(mech) == CLASS_VEH_GROUND))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit has no ejection seat!");
    return;
  }
  if (mech_is_flying_type(mech) && !mech_is_landed(mech)) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "Regrettably, right now you can only eject when landed, sorry - no "
        "parachute :P");
    return;
  }
  if (!is_in_character(btech_context_database(mech_context(mech)),
                       mech_dbref(mech))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit isn't in character!");
    return;
  }
  if (!btech_context_in_character_enabled(mech_context(mech))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This MUX isn't in character!");
    return;
  }
  if (!is_in_character(
          btech_context_database(mech_context(mech)),
          game_object_location(btech_context_database(mech_context(mech)),
                               mech_dbref(mech)))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your location isn't in character!");
    return;
  }
  if (mech_is_started(mech) && mech_pilot_dbref(mech) != player) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You aren't in da pilot's seat - no ejection for you!");
    return;
  }
  if (!mech_is_started(mech)) {
    if ((character_lookup(&(CharacterLookupRequest){
            .context = mech_context(mech),
            .viewer = GOD,
            .name = btech_attribute_read(
                btech_context_database(mech_context(mech)), mech_dbref(mech),
                A_PILOTNUM, (char[LBUF_SIZE]){0}),
        })) != player) {
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          "You aren't the official pilot of this thing. Try 'disembark'");
      return;
    }
  }
  if (mech_class(mech) == CLASS_MECH) {
    if (mech_critical_is_nonfunctional(mech, HEAD, 2)) {
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          "The parts of cockpit that control ejection are already used. Try "
          "'disembark'");
      return;
    }
  }
  /* Ok.. time to eject ourselves */
  char_eject(player, mech);
}

static void char_disembark(DbRef player, Mech *mech) {
  char message_buffer[MBUF_SIZE];
  Mech *m;
  DbRef suit;
  char *d;
  BattleMap *mymap;
  long initial_speed;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  GameDatabase *database = btech_context_database(mech_context(mech));

  (void)snprintf(message_buffer, sizeof(message_buffer), "MechWarrior - %s",
                 game_object_name(database, player));
  suit = create_obj(evaluation, GOD, OBJECT_TYPE_THING, message_buffer);
  silly_atr_set_in(database, suit, A_XTYPE, "MECH");
  s_xcode(database, suit);
  btech_special_object_flag_changed(mech_context(mech), GOD, suit, false, true);
  d = btech_attribute_read(database, player, A_MWTEMPLATE,
                           (char[LBUF_SIZE]){0});
  m = btech_context_get_mech(mech_context(mech), suit);
  if (!m) {
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS,
        "Unable to create special obj for #%ld's disembarkation.", player);
    destroy_failed_suit(mech_context(mech), evaluation, suit);
    mecha_notify(evaluation, player,
                 "Sorry, something serious went wrong, contact a Wizard "
                 "(can't create RS object)");
    return;
  }
  if (!mech_template_load(
          GOD, m, (!d || !*d || !strcmp(d, "#-1")) ? "MechWarrior" : d)) {
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS,
                       "Unable to load mechwarrior template for #%ld's "
                       "disembarkation. (%s)",
                       player, (!d || !*d) ? "Default template" : d);
    destroy_failed_suit(mech_context(mech), evaluation, suit);
    mecha_notify(evaluation, player,
                 "Sorry, something serious went wrong, contact a Wizard "
                 "(can't load MWTemplate)");
    return;
  }
  silly_atr_set_in(database, suit, A_MECHNAME, "MechWarrior");
  mech_team_set(m, mech_team(mech));
  if (mech_map_index_set(m, mech_map_dbref(mech), nullptr) != MECH_MAP_SET_OK ||
      !mech_position_set(&(MechPositionSetRequest){
          .mech = m, .x = mech_position_x(mech), .y = mech_position_y(mech)})) {
    destroy_failed_suit(mech_context(mech), evaluation, suit);
    mecha_notify(evaluation, player,
                 "Unable to disembark because unit placement failed.");
    return;
  }
  if (!move_player_into_unit(evaluation, player, suit, mech_map_dbref(mech))) {
    destroy_failed_suit(mech_context(mech), evaluation, suit);
    mecha_notify(evaluation, player,
                 "Unable to disembark because teleportation was denied.");
    return;
  }
  mech_position_hex_z_set(m, mech_position_z(mech));
  s_in_character(database, suit);
  initialize_pc(player, m);
  mech_pilot_dbref_set(m, player);
  (void)snprintf(message_buffer, sizeof(message_buffer), "#%ld", player);
  silly_atr_set_in(database, mech_dbref(m), A_PILOTNUM, message_buffer);
  mech_team_set(m, mech_team(mech));
  mech_radio_frequency_set(
      m, 0, (int)(btech_context_random_i31(mech_context(m)) % 1000000));
  notify_printf(evaluation, player, "Emergency radio channel set to %d.",
                mech_radio_frequency(m, 0));
  /* #endif
  #endif
  */
  mymap = btech_context_get_map(mech_context(mech), mech_map_dbref(m));
  if ((mech_position_z(m) > (battle_map_hex_elevation(mymap, mech_position_x(m),
                                                      mech_position_y(m)) +
                             1)) &&
      (mech_position_z(m) > 0)) {
    mecha_notify(
        evaluation, player,
        "You open the hatch and climb out of the unit. Maybe you should "
        "have done this while the thing was closer to the ground...");
    mech_los_broadcastf(m, "jumps out of %s... in mid air !",
                        mech_display_id(mech).text);
    initial_speed =
        (long)((((mech_current_speed(mech) + mech_vertical_speed(mech)) / MP1) /
                2.0F) +
               4.0F);
    mech_event_schedule(m, EVENT_FALL, mech_fall_event, FALL_TICK,
                        -initial_speed);
  } else {
    mech_los_broadcastf(m, "climbs out of %s!", mech_display_id(mech).text);
    mecha_notify(evaluation, player, "You climb out of the unit.");
  }
}

/**
 * Handle the disembarking of pilots from units.
 */
void mech_disembark(DbRef player, Mech *mech, char *buffer [[maybe_unused]]) {
  if (!common_checks(player, mech, MECH_USUALS))
    return;
  if (!((mech_class(mech) == CLASS_MECH) || (mech_class(mech) == CLASS_VTOL) ||
        (mech_class(mech) == CLASS_VEH_GROUND))) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "The door ! The door ? The Door ?!? Where's the exit in this damned "
        "thing ?");
    return;
  }

  /*  if (mech_is_flying_type(mech) &&
   * !mech_is_landed(mech)) {
    mecha_notify(btech_context_evaluation(mech->xcode.context), player, "What,
  in the air ? Are you suicidal ?"); return;
  } */
  if (!is_in_character(btech_context_database(mech_context(mech)),
                       mech_dbref(mech))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit isn't in character!");
    return;
  }
  if (!btech_context_in_character_enabled(mech_context(mech))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This MUX isn't in character!");
    return;
  }
  if (!is_in_character(
          btech_context_database(mech_context(mech)),
          game_object_location(btech_context_database(mech_context(mech)),
                               mech_dbref(mech)))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your location isn't in character!");
    return;
  }
  if ((mech_is_started(mech) || mech_event_count(mech, EVENT_STARTUP)) &&
      (mech_pilot_dbref(mech) == player)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "While it's running!? Don't be daft.");
    return;
  }
  if (fabsf(mech_current_speed(mech)) > 25.0F) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Are you suicidal ? That thing is moving too fast !");
    return;
  }
  /* Ok.. time to disembark ourselves */
  char_disembark(player, mech);
}

/**
 * Handle the disembarking of units from within carriers.
 */
void mech_udisembark(DbRef player, Mech *mech,
                     const char *buffer [[maybe_unused]]) {
  char message_buffer[128];

  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  GameDatabase *database = btech_context_database(mech_context(mech));
  Mech *target;
  DbRef newmech;     /* The carrier. */
  BattleMap *mymap;  /* The map to disembark to */
  int under_repairs; /* Is the unit still under repairs? */
  int i;             /* Used in section recycle for loop. */

  /* Any IN_CHARACTER unit's pilot must match the invoker to disembark.
   * A unit that is not IC can be disembarked by anyone.
   */
  if (is_in_character(database, mech_dbref(mech)) &&
      !is_wizard(database, player) &&
      (character_lookup(&(CharacterLookupRequest){
           .context = mech_context(mech),
           .viewer = GOD,
           .name = btech_attribute_read(database, mech_dbref(mech), A_PILOTNUM,
                                        (char[LBUF_SIZE]){0}),
       }) != player)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This isn't your mech!");
    return;
  }

  /* Find the carrier that the invoker's unit is in and check it for validity.
   */
  newmech = game_object_location(database, mech_dbref(mech));
  if (!(is_good_obj(database, newmech) && is_xcode(database, newmech))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're not being carried!");
    return;
  }
  target = btech_context_get_mech(mech_context(mech), newmech);
  if (!target) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Not being carried!");
    return;
  }
  if (mech_map_dbref(target) == -1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are not on a map.");
    return;
  }

  /* Don't allow repairing units to disembark */
  under_repairs = figure_latest_tech_event(mech);
  if (under_repairs) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "This 'Mech is still under repairs (see checkstatus for more info)");
    return;
  }

  if (fabsf(mech_current_speed(target)) > mech_walking_speed(target)) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "You cannot leave while the carrier is moving faster than walk speed!");
    return;
  }

  const DbRef DESTINATION = mech_map_dbref(target);
  mymap = btech_context_get_map(mech_context(mech), DESTINATION);
  if (!mymap) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Major map error possible. Prolly should contact a wizard.");
    return;
  }
  Mech *const UNITS[] = {mech};
  const MechMapSetBatchRequest MAP_PLACEMENT = {
      .mechs = UNITS, .count = 1, .map = DESTINATION};
  if (mech_map_index_preflight_batch(&MAP_PLACEMENT) != MECH_MAP_SET_OK ||
      !mech_map_position_is_valid(
          &(MechMapPositionRequest){.context = mech_context(mech),
                                    .map = DESTINATION,
                                    .x = mech_position_x(target),
                                    .y = mech_position_y(target)})) {
    mecha_notify(evaluation, player,
                 "Unable to disembark because unit placement failed.");
    return;
  }
  /* Teleport loudly so native enter events and other messages run. */
  if (!move_via_teleport(&(ObjectMovementRequest){.evaluation = evaluation,
                                                  .object = mech_dbref(mech),
                                                  .destination = DESTINATION,
                                                  .cause = 1})) {
    mecha_notify(evaluation, player,
                 "Unable to disembark because teleportation was denied.");
    return;
  }

  /* Carry out the BattleTech side of the disembarking. */
  if (mech_map_index_set_batch(&MAP_PLACEMENT) != MECH_MAP_SET_OK ||
      !mech_position_set(
          &(MechPositionSetRequest){.mech = mech,
                                    .x = mech_position_x(target),
                                    .y = mech_position_y(target)})) {
    mecha_notify(evaluation, player,
                 "Unable to disembark because unit placement failed.");
    return;
  }
  mech_position_z_set(mech, mech_position_z(target));
  const int ELEVATION = mech_position_z(mech);
  mech_position_real_z_set(mech, ZSCALE * (float)ELEVATION);
  /* If we make it safely, start the invoker's unit up once it's on the map. */
  if (!mech_is_destroyed(mech) &&
      game_object_location(database, player) == mech_dbref(mech)) {
    mech_pilot_dbref_set(mech, player);
    mech_power_up(mech);
  }

  mark_for_los_update(mech);
  mech_cargo_weight_recalculate(mech);
  mech_player_killer_set(mech, false);
  mech_los_broadcast(mech, "powers up!");
  mech_sixth_sense_set(
      mech, ((mech_pilot_dbref(mech) > 0 &&
              is_player(database, mech_pilot_dbref(mech)))
                 ? char_getvalue(mech_context(mech), mech_pilot_dbref(mech),
                                 "Sixth_Sense")
                 : 0) != 0);
  mech_communication_skill_set(mech, DEFAULT_COMM);

  if (is_player(database, mech_pilot_dbref(mech))) {
    mech_communication_skill_set(
        mech, char_getskilltarget(mech_context(mech), mech_pilot_dbref(mech),
                                  "Comm-Conventional", 0));
    mech_perception_target_set(mech, char_getskilltarget(mech_context(mech),
                                                         mech_pilot_dbref(mech),
                                                         "Perception", 0));
  } else {
    mech_communication_skill_set(mech, 6);
    mech_perception_target_set(mech, 6);
  }
  mech_communication_last_tick_set(mech, 0);
  autopilot_resume_for_mech(mech);
  mech_cargo_space_add(target, mech_tonnage(mech) * 100);
  mark_for_los_update(target);

  /* A hidden carrier that is disembarked from loses its HIDDEN status */
  if (mech_condition_summary(target).hidden) {
    mech_hidden_set(target, false);
    mech_los_broadcast(target, "becomes visible as it is disembarked from.");
  }

  /* Para-dropping out of units from elevations. */
  if (!mech_is_flying_type(mech) &&
      mech_position_z(mech) > battle_map_hex_elevation(mymap,
                                                       mech_position_x(mech),
                                                       mech_position_y(mech)) &&
      mech_position_z(mech) > 0) {
    mecha_notify(evaluation, player,
                 "You open the hatch and drop out of the unit....");
    mech_los_broadcastf(mech,
                        "drops out of %s and begins falling to the ground.",
                        mech_display_id(target).text);
    (void)snprintf(message_buffer, sizeof(message_buffer), "%d %d %d",
                   mech_position_x(mech), mech_position_y(mech),
                   mech_position_z(mech));
    mech_ood_initiate(player, mech, message_buffer);
  } else {
    if (mech_class(mech) == CLASS_BSUIT) {
      mech_los_broadcastf(mech, "climbs out of %s!",
                          mech_display_id(target).text);
      mecha_notify(evaluation, player, "You climb out of the unit.");
    } else {
      /* If the carrier is destroyed, do damage to the disembarking unit. */
      if (mech_is_destroyed(target) || !mech_is_started(target)) {
        mech_los_broadcastf(mech,
                            "smashes open the ramp door and emerges from %s!",
                            mech_display_id(target).text);
        mecha_notify(evaluation, player,
                     "You smash open the door and break out.");
        mech_fall(mech, 4, false);
      } else {
        /* All is well. */
        mech_los_broadcastf(mech, "emerges from the ramp out of %s!",
                            mech_display_id(target).text);
        mecha_notify(evaluation, player,
                     "You emerge from the unit loading ramp.");
        if (mech_is_landed(mech) &&
            mech_position_z(mech) >
                battle_map_hex_elevation(mymap, mech_position_x(mech),
                                         mech_position_y(mech)) &&
            mech_is_flying_type(mech))
          mech_landed_set(mech, false);
      }
    }
  }

  /* Recycle any weapons/sections they have to prevent munchkin behavior. */
  if (mech_class(mech) == CLASS_BSUIT) {
    bsuit_recycle_start(mech, 20);
  } else if (mech_class(mech) == CLASS_MECH || mech_class(mech) == CLASS_MW) {
    for (i = 0; i < NUM_SECTIONS; i++)
      mech_set_recycle_limb(mech, i, PHYSICAL_RECYCLE_TIME);
  } else if (mech_class(mech) == CLASS_VEH_GROUND ||
             mech_class(mech) == CLASS_VTOL) {
    for (i = 0; i < NUM_SECTIONS; i++)
      if (i == ROTOR)
        continue;
      else
        mech_set_recycle_limb(mech, i, PHYSICAL_RECYCLE_TIME);
  }

  fix_pilotdamage(mech, mech_pilot_dbref(mech));
  mech_speed_correct(target);
} /* end mech_udisembark */
