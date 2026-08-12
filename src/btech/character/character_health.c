#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btechstats.h"
#include "btechstats_api.h"
#include "btechstats_global.h"
#include "btechstats_internal.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_combat_misc_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_partnames_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/character_state.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "mux/world/player.h"
#include "registry_api.h"
#include "special_object.h"
#include "weapon_catalogue_api.h"

static int player_character_section(size_t index) {
  switch (index) {
  case 0:
    return HEAD;
  case 1:
    return CTORSO;
  case 2:
    return RARM;
  case 3:
    return RLEG;
  default:
    abort();
  }
}

void character_stats_clear(PSTATS *s) {
  for (int i = 0; i < NUM_CHARVALUES; i++) {
    character_stats_value_set(&(CharacterStatsValueChange){
        .stats = s,
        .code = i,
        .value =
            character_value_definition(i)->type == CHAR_ATTRIBUTE ? 1 : 0});
    character_stats_xp_set(
        &(CharacterStatsExperienceChange){.stats = s, .code = i});
    character_stats_last_use_set(
        &(CharacterStatsLastUseChange){.stats = s, .code = i});
  }
  char_setstatvalue(s, "lives", 1);
}

/************************/

/*      MUSE COMMANDS   */

/************************/

void do_charclear(CommandInvocation *invocation) {
  CommandContext *command = invocation->context;
  GameDatabase *database = command->world->database;
  DbRef player = invocation->player;

  if (!invocation->first || !*invocation->first) {
    mecha_notify(&command->evaluation, player,
                 "Who do you want to clear the stats from?");
    return;
  }

  DbRef thing = lookup_player(command->world, player, invocation->first, 0);
  if (thing == NOTHING) {
    mecha_notify(&command->evaluation, player, "I don't know who that is");
    return;
  }

  character_state_clear(database, thing);
  if (thing == command->btech->cached_target_character)
    command->btech->cached_target_character = -1;
  notify_printf(&command->evaluation, player, "Player #%ld stats cleared",
                thing);
}

DbRef character_lookup(const CharacterLookupRequest *request) {
  WorldContext world = {
      .database = request->context->database,
      .configuration = request->context->configuration,
      .indexes = request->context->world_indexes,
      .access_control = request->context->access_control,
  };
  return lookup_player(&world, request->viewer, request->name, 0);
}

static int loc_mod(int loc) {
  switch (loc) {
  case HEAD:
    return 15;
  case CTORSO:
    return 50;
  case LTORSO:
  case RTORSO:
    return 35;
  case LARM:
  case RARM:
    return 30;
  case LLEG:
  case RLEG:
    return 35;
  }
  return 0;
}

void initialize_pc(DbRef player, Mech *mech) {
  BtechContext *context = mech_context(mech);
  PSTATS stats, *s = &stats;
  int bruise, lethal, player_bld;
  int dam, tot;
  char *c;
  int cnt;
  char buf1[MBUF_SIZE];
  char buf2[MBUF_SIZE];
  char buf3[MBUF_SIZE];
  char buf4[2];
  int ammo1;
  int ammo2;
  int i, id;

  if (!mech_player_character_initialization_begin(mech))
    return;
  buf4[1] = 0;
  character_stats_retrieve(context, player,
                           VALUES_HEALTH | VALUES_ATTRS | VALUES_SKILLS, s);
  player_bld = char_getstatvalue(s, "build");
  bruise = char_getstatvalue(s, "bruise");
  lethal = char_getstatvalue(s, "lethal");
  tot = player_bld * 20;
  dam = bruise + lethal;
  const int MOVEMENT_SCORE = player_bld + char_getstatvalue(s, "reflexes") +
                             char_getstatvalue(s, "running");
  mech_maximum_speed_set(mech, (float)MOVEMENT_SCORE * MP1 / 9.0F);
#define PC_LOCS 4
  for (i = 0; i < NUM_SECTIONS; i++) {
    mech_section_armor_set(mech, i, 0);
    mech_section_original_armor_set(mech, i, 0);
    mech_section_internal_set(mech, i, (loc_mod(i) * (tot - dam)) / 100 + 1);
    mech_section_original_internal_set(mech, i,
                                       (loc_mod(i) * (tot - dam)) / 100 + 1);
  }
  c = btech_attribute_read(context->database, player, A_PCEQUIP,
                           (char[LBUF_SIZE]){0});
  char equipment[LBUF_SIZE];
  (void)snprintf(equipment, sizeof(equipment), "%s", c);
  char *armor = strtok(equipment, " \t\r\n");
  char *weapon_one = strtok(nullptr, " \t\r\n");
  char *weapon_two = strtok(nullptr, " \t\r\n");
  char *first_ammunition = strtok(nullptr, " \t\r\n");
  char *second_ammunition = strtok(nullptr, " \t\r\n");
  cnt = 0;
  if (armor) {
    if (strlen(armor) >= sizeof(buf1))
      return;
    (void)snprintf(buf1, sizeof(buf1), "%s", armor);
    cnt = 1;
  }
  if (weapon_one) {
    if (strlen(weapon_one) >= sizeof(buf2))
      return;
    (void)snprintf(buf2, sizeof(buf2), "%s", weapon_one);
    cnt = 2;
  }
  if (weapon_two) {
    if (strlen(weapon_two) >= sizeof(buf3))
      return;
    (void)snprintf(buf3, sizeof(buf3), "%s", weapon_two);
    cnt = 3;
  }
  if (first_ammunition) {
    if (!parse_int_checked(first_ammunition, &ammo1))
      return;
    cnt = 4;
  }
  if (second_ammunition) {
    if (!parse_int_checked(second_ammunition, &ammo2))
      return;
    cnt = 5;
  }

  switch (cnt) {
  case 5:
  case 4:
  case 3:
    if (strcmp(buf3, "-")) {
      PartMatchResult match =
          part_match_next(&(PartMatchRequest){.context = context,
                                              .pattern = buf3,
                                              .kind = PART_MATCH_VERY_LONG,
                                              .cursor = -1});
      if (!match.found) {
        btech_channel_send(
            context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("Invalid PC weapon #1 for %s(#%ld): %s",
                    game_object_name(mech_context(mech)->database, player),
                    player, buf3));
        return;
      }
      id = match.part.id;
      if (equipment_is_weapon(id)) {
        mech_critical_configure(&(CriticalSlotConfiguration){
            .mech = mech,
            .slot = {.section = LARM, .critical = 0},
            .part_type = id});
        i = weapon_catalogue_ammunition_per_ton(
            weapon_from_equipment_index(id));
        if (i) {
          mech_critical_configure(&(CriticalSlotConfiguration){
              .mech = mech,
              .slot = {.section = LARM, .critical = 1},
              .part_type =
                  ammunition_equipment_index(weapon_from_equipment_index(id)),
              .data = cnt >= 5 ? ammo2 : i});
        }
      }
    }
    [[fallthrough]];
  case 2:
    if (strcmp(buf2, "-")) {
      PartMatchResult match =
          part_match_next(&(PartMatchRequest){.context = context,
                                              .pattern = buf2,
                                              .kind = PART_MATCH_VERY_LONG,
                                              .cursor = -1});
      if (!match.found) {
        btech_channel_send(
            context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("Invalid PC weapon #1 for %s(#%ld): %s",
                    game_object_name(mech_context(mech)->database, player),
                    player, buf2));
        return;
      }
      id = match.part.id;
      if (equipment_is_weapon(id)) {
        mech_critical_configure(&(CriticalSlotConfiguration){
            .mech = mech,
            .slot = {.section = RARM, .critical = 0},
            .part_type = id});
        i = weapon_catalogue_ammunition_per_ton(
            weapon_from_equipment_index(id));
        if (i) {
          mech_critical_configure(&(CriticalSlotConfiguration){
              .mech = mech,
              .slot = {.section = RARM, .critical = 1},
              .part_type =
                  ammunition_equipment_index(weapon_from_equipment_index(id)),
              .data = cnt >= 4 ? ammo1 : i});
        }
      }
    }
    [[fallthrough]];
  case 1:
    if (strlen(buf1) != PC_LOCS) {
      btech_channel_send(
          context, BTECH_CHANNEL_MECH_ERRORS, "%s",
          tprintf("Invalid armor string for %s(#%ld): %s",
                  game_object_name(mech_context(mech)->database, player),
                  player, buf1));
      return;
    }
    for (size_t index = 0; index < strlen(buf1); index++) {
      const char *armor_character =
          checked_storage_at_const(buf1, sizeof(buf1), sizeof(char), index);
      if (*armor_character < '0' || *armor_character > '9') {
        btech_channel_send(
            context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("Invalid armor char for %s(#%ld) in %s (pos %d,%c)",
                    game_object_name(mech_context(mech)->database, player),
                    player, buf1, (int)index + 1, *armor_character));
        return;
      }
    }
    for (size_t index = 0; index < strlen(buf1); index++) {
      const char *armor_character =
          checked_storage_at_const(buf1, sizeof(buf1), sizeof(char), index);
      mech_section_armor_set(mech, player_character_section(index),
                             *armor_character - '0');
    }
  }
}

void fix_pilotdamage(Mech *mech, DbRef player) {
  BtechContext *context = mech_context(mech);
  PSTATS stats, *s = &stats;
  int bruise, lethal, player_bld;

  character_stats_retrieve(context, player, VALUES_HEALTH | VALUES_ATTRS, s);
  bruise = char_getstatvalue(s, "bruise");
  lethal = char_getstatvalue(s, "lethal");
  player_bld = char_getstatvalue(s, "build") * 2;
  if (player_bld < 1 || player_bld > 100)
    player_bld = 10;

  mech_pilot_status_set(mech, (bruise + lethal) / player_bld);
}

static int pilot_status_roll_needed(int status) {
  switch (status) {
  case 0:
    return 0;
  case 1:
    return 3;
  case 2:
    return 5;
  case 3:
    return 7;
  case 4:
    return 10;
  case 5:
    return 11;
  default:
    abort();
  }
}

int mw_ic_bth(Mech *mech) {
  BtechContext *context = mech_context(mech);
  /* Rule Reference: BMR Revised, Page 17 ( Consciousness Table ) */
  /* Rule Reference: Total Warfare, Page 41-42 ( Consciousness Table ) */
  /* Rule Reference: MaxTech Revised, Page 46 ( Pain Resistance = -1 ) */

  int player_bld;
  int bruise, playerhits;
  PSTATS stats, *s = &stats;
  int mod = 0;

  character_stats_retrieve(context, mech_pilot_dbref(mech),
                           VALUES_ATTRS | VALUES_ADVS | VALUES_HEALTH, s);
  player_bld = char_getstatvalue(s, "build");
  bruise = char_getstatvalue(s, "bruise");
  playerhits = 10 * player_bld - bruise;
  if (char_getstatvalue(s, "pain_resistance") == 1)
    mod = -1;
  if (playerhits >= (8 * player_bld))
    return 3 + mod;
  if (playerhits >= (6 * player_bld))
    return 5 + mod;
  if (playerhits >= (4 * player_bld))
    return 7 + mod;
  if (playerhits >= (2 * player_bld))
    return 10 + mod;
  if (playerhits >= -1)
    return 11 + mod;
  return 0;
}

int handlemwconc(Mech *mech, int initial) {
  /* Rule Reference: MechWarrior 2nd Edition RPG, Page 22 (Toughness = Best of
   * 3D6) */
  /* Rule Reference: Old Tactical Handbook, Page 51 (Use MW 2nd Edition) */
  /* Rule Reference: BMR Revised, Page 17 ( >5 Bruise = Death ) */
  /* Rule Reference: Total Warfare, Page 41-42 ( >5 Bruise = Death ) */

  int m, roll;

  if (is_in_character(mech_context(mech)->database, mech_dbref(mech)) &&
      mech_pilot_dbref(mech) > 0) {
    m = mw_ic_bth(mech);
  } else {
    if (initial)
      if (mech_pilot_status(mech) > 5) {
        mech_notify(mech, MECHPILOT, "You are killed from personal injuries!!");

        // This is here to avoid multi-triggers of AMECHDEST.
        if (!mech_is_destroyed(mech))
          mech_destroy(mech, mech, 0, KILL_TYPE_MWDAMAGE);

        mech_pilot_dbref_set(mech, -1);
        mech_movement_stop(mech);
        return 0;
      }
    m = pilot_status_roll_needed(bounded(0, mech_pilot_status(mech), 4));
  }
  if (initial && mech_pilot_is_unconscious(mech))
    return 0;
  if (has_bool_advantage(mech_context(mech), mech_pilot_dbref(mech),
                         "toughness"))
    /*  Gets the saving roll for someone with toughness  */
    roll = char_rollsaving(mech_context(mech));
  else
    roll = char_rollskilled(mech_context(mech));
  if (mech_pilot_dbref(mech) >= 0) {
    if (initial) {
      mech_notify(mech, MECHPILOT, "You attempt to keep consciousness!");
      mech_printf(mech, MECHPILOT, "Retain Conciousness on: %d  \tRoll: %d",
                  abs(m), roll);
    } else {
      mech_notify(mech, MECHPILOT, "You attempt to regain consciousness!");
      mech_printf(mech, MECHPILOT, "Regain Consciousness on: %d  \tRoll: %d",
                  abs(m), roll);
    }
  }
  if (roll < (abs(m))) {
    if (initial)
      mech_notify(mech, MECHPILOT,
                  "Consciousness slips away from you as you enter a sea of "
                  "darkness...");
    mech_unconsciousness_extend(mech, UNCONSCIOUS_TIME);
    return 0;
  }
  return 1;
}

void headhitmwdamage(Mech *mech, Mech *attacker, int dam) {
  BtechContext *context = mech_context(mech);
  PSTATS stats, *s = &stats;
  DbRef player;
  int damage, bruise, lethaldam, player_bld;

  if (mech_dbref(mech) < 0)
    return;
  /* check to see if mech is IC */
  if (!is_in_character(mech_context(mech)->database, mech_dbref(mech)) ||
      !mech_has_pilot(mech)) {
    mech_pilot_status_add(mech, dam);
    handlemwconc(mech, 1);
    return;
  }
  player = mech_pilot_dbref(mech);

  character_stats_retrieve(context, player,
                           VALUES_ATTRS | VALUES_ADVS | VALUES_HEALTH, s);
  /* get the player_stats structure */

  bruise = char_getstatvalue(s, "bruise");
  /* gets the players bruise damage */

  player_bld = char_getstatvalue(s, "build");
  /* get the player's BLD value */

  damage = 2 * player_bld * dam;
  /* the damage we are due */

  bruise += damage;
  /* this part subtracts 10 from players lethal damage */

  if (bruise > player_bld * 10) {
    lethaldam = char_getstatvalue(s, "lethal");
    lethaldam += (bruise - player_bld * 10);
    bruise = player_bld * 10;

    if (lethaldam >= player_bld * 10) {
      char_setstatvalue(s, "lethal", player_bld * 10 - 1);
      char_setstatvalue(s, "bruise", player_bld * 10);
      character_stats_store(context, player, s, VALUES_HEALTH);
      if (!mech_is_destroyed(mech)) {
        mech_destroy(mech, attacker, 0, KILL_TYPE_MWDAMAGE);
      }
      mech_contents_kill_if_in_character(mech);
      return;
    }
    char_setstatvalue(s, "lethal", lethaldam);
  }
  char_setstatvalue(s, "bruise", bruise);
  character_stats_store(context, player, s, VALUES_HEALTH);
  handlemwconc(mech, 1);
  mech_pilot_status_add(mech, dam);
}

void mwlethaldam(Mech *mech, Mech *attacker, int dam) {
  BtechContext *context = mech_context(mech);
  PSTATS stats, *s = &stats;
  DbRef player;
  int lethaldam, player_bld;

  if (mech_dbref(mech) < 0)
    return;
  /* check to see if mech is IC */
  if (!is_in_character(mech_context(mech)->database, mech_dbref(mech)) ||
      !mech_has_pilot(mech)) {
    mech_pilot_status_add(mech, dam);
    handlemwconc(mech, 1);
    return;
  }
  player = mech_pilot_dbref(mech);

  character_stats_retrieve(context, player,
                           VALUES_ATTRS | VALUES_ADVS | VALUES_HEALTH, s);
  /* get the player_stats structure */
  player_bld = char_getstatvalue(s, "build");
  if (!player_bld)
    player_bld++;
  lethaldam = char_getstatvalue(s, "lethal");
  lethaldam += bounded(10, dam * player_bld, 40);
  if (lethaldam >= player_bld * 10) {
    lethaldam = player_bld * 10;
    char_setstatvalue(s, "lethal", lethaldam - 1);
    char_setstatvalue(s, "bruise", lethaldam);
    character_stats_store(context, player, s, VALUES_HEALTH);
    if (!mech_is_destroyed(mech)) {
      mech_destroy(mech, attacker, 0, KILL_TYPE_MWDAMAGE);
    }
    mech_contents_kill_if_in_character(mech);
    return;
  }
  char_setstatvalue(s, "bruise", player_bld * 10 - 5);
  char_setstatvalue(s, "lethal", lethaldam);
  character_stats_store(context, player, s, VALUES_HEALTH);
  handlemwconc(mech, 1);
  mech_pilot_status_add(mech, dam);
}
