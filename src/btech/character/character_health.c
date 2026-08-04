#include "btechstats_internal.h"

void character_stats_clear(PSTATS *s) {
  int i;

  for (i = 0; i < (int)(NUM_CHARVALUES); i++) {
    s->values[i] = (char_values[i].type == CHAR_ATTRIBUTE ? 1 : 0);
    s->xp[i] = 0;
  }
  char_slives(s, 1);
}

/************************/

/*      MUSE COMMANDS   */

/************************/

void do_charclear(CommandInvocation *invocation) {
  CommandContext *command = invocation->context;
  GameDatabase *database = command->world->database;
  DbRef player = invocation->player;

  if (!invocation->first || !*invocation->first) {
    notify(&command->evaluation, player,
           "Who do you want to clear the stats from?");
    return;
  }

  DbRef thing = lookup_player(command->world, player, invocation->first, 0);
  if (thing == NOTHING) {
    notify(&command->evaluation, player, "I don't know who that is");
    return;
  }

  silly_atr_set_in(database, thing, A_ATTRS, "");
  silly_atr_set_in(database, thing, A_SKILLS, "");
  silly_atr_set_in(database, thing, A_ADVS, "");
  silly_atr_set_in(database, thing, A_HEALTH, "");
  notify_printf(&command->evaluation, player, "Player #%ld stats cleared",
                thing);
}

DbRef char_lookupplayer(BtechContext *context, DbRef player, DbRef cause,
                        int key, char *arg1) {
  WorldContext world = {
      .database = context->database,
      .configuration = context->configuration,
      .indexes = context->world_indexes,
      .access_control = context->access_control,
  };
  return lookup_player(&world, player, arg1, 0);
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
  int bruise, lethal, playerBLD;
  int dam, tot;
  char *c;
  int cnt;
  char buf1[MBUF_SIZE];
  char buf2[MBUF_SIZE];
  char buf3[MBUF_SIZE];
  char buf4[2];
  int ammo1;
  int ammo2;
  int i, id, brand;
  int pc_loc_to_mech_loc[] = {HEAD, CTORSO, RARM, RLEG};

  if (!mech_player_character_initialization_begin(mech))
    return;
  buf4[1] = 0;
  character_stats_retrieve(context, player,
                           VALUES_HEALTH | VALUES_ATTRS | VALUES_SKILLS, s);
  playerBLD = char_gvalue(s, "build");
  bruise = char_gbruise(s);
  lethal = char_glethal(s);
  tot = playerBLD * 20;
  dam = bruise + lethal;
  mech_maximum_speed_set(mech, (playerBLD + char_gvalue(s, "reflexes") +
                                char_gvalue(s, "running")) *
                                   MP1 / 9.0);
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
  cnt = sscanf(c, "%s %s %s %d %d", buf1, buf2, buf3, &ammo1, &ammo2);

  switch (cnt) {
  case 5:
  case 4:
  case 3:
    if (strcmp(buf3, "-")) {
      if (!find_matching_vlong_part(context, buf3, nullptr, &id, &brand)) {
        btech_channel_send(
            context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("Invalid PC weapon #1 for %s(#%ld): %s",
                    game_object_name(mech_context(mech)->database, player),
                    player, buf3));
        return;
      }
      if (IsWeapon(id)) {
        mech_critical_configure(mech, LARM, 0, id, 0, 0, 0);
        if ((i = MechWeapons[Weapon2I(id)].ammoperton)) {
          mech_critical_configure(mech, LARM, 1, I2Ammo(Weapon2I(id)),
                                  cnt >= 5 ? ammo2 : i, 0, 0);
        }
      }
    }
    [[fallthrough]];
  case 2:
    if (strcmp(buf2, "-")) {
      if (!find_matching_vlong_part(context, buf2, nullptr, &id, &brand)) {
        btech_channel_send(
            context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("Invalid PC weapon #1 for %s(#%ld): %s",
                    game_object_name(mech_context(mech)->database, player),
                    player, buf2));
        return;
      }
      if (IsWeapon(id)) {
        mech_critical_configure(mech, RARM, 0, id, 0, 0, 0);
        if ((i = MechWeapons[Weapon2I(id)].ammoperton)) {
          mech_critical_configure(mech, RARM, 1, I2Ammo(Weapon2I(id)),
                                  cnt >= 4 ? ammo1 : i, 0, 0);
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
    for (i = 0; buf1[i]; i++)
      if (!isdigit(buf1[i])) {
        btech_channel_send(
            context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("Invalid armor char for %s(#%ld) in %s (pos %d,%c)",
                    game_object_name(mech_context(mech)->database, player),
                    player, buf1, i + 1, buf1[i]));
        return;
      }
    for (i = 0; buf1[i]; i++) {
      buf4[0] = buf1[i];
      mech_section_armor_set(mech, pc_loc_to_mech_loc[i], atoi(buf4));
    }
  }
}

void fix_pilotdamage(Mech *mech, DbRef player) {
  BtechContext *context = mech_context(mech);
  PSTATS stats, *s = &stats;
  int bruise, lethal, playerBLD;

  character_stats_retrieve(context, player, VALUES_HEALTH | VALUES_ATTRS, s);
  bruise = char_gbruise(s);
  lethal = char_glethal(s);
  playerBLD = char_gvalue(s, "build") * 2;
  if (playerBLD < 1 || playerBLD > 100)
    playerBLD = 10;

  mech_pilot_status_set(mech, (bruise + lethal) / playerBLD);
}

const int PilotStatusRollNeeded[] = {0, 3, 5, 7, 10, 11};

#define CHDAM(val, ret)                                                        \
  if (playerhits >= ((val)))                                                   \
    return ret * mod;

int mw_ic_bth(Mech *mech) {
  BtechContext *context = mech_context(mech);
  /* Rule Reference: BMR Revised, Page 17 ( Consciousness Table ) */
  /* Rule Reference: Total Warfare, Page 41-42 ( Consciousness Table ) */
  /* Rule Reference: MaxTech Revised, Page 46 ( Pain Resistance = -1 ) */

  int playerBLD;
  int bruise, playerhits;
  PSTATS stats, *s = &stats;
  int mod = 0;

  character_stats_retrieve(context, mech_pilot_dbref(mech),
                           VALUES_ATTRS | VALUES_ADVS | VALUES_HEALTH, s);
  playerBLD = char_gvalue(s, "build");
  bruise = char_gbruise(s);
  playerhits = 10 * playerBLD - bruise;
  if (char_gvalue(s, "pain_resistance") == 1)
    mod = -1;
  if (playerhits >= (8 * playerBLD))
    return 3 + mod;
  else if (playerhits >= (6 * playerBLD))
    return 5 + mod;
  else if (playerhits >= (4 * playerBLD))
    return 7 + mod;
  else if (playerhits >= (2 * playerBLD))
    return 10 + mod;
  else if (playerhits >= -1)
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
      mech_pilot_dbref(mech) > 0)
    m = mw_ic_bth(mech);
  else {
    if (initial)
      if (mech_pilot_status(mech) > 5) {
        mech_notify(mech, MECHPILOT, "You are killed from personal injuries!!");

        // This is here to avoid multi-triggers of AMECHDEST.
        if (!mech_is_destroyed(mech))
          DestroyMech(mech, mech, 0, KILL_TYPE_MWDAMAGE);

        mech_pilot_dbref_set(mech, -1);
        mech_movement_stop(mech);
        return 0;
      }
    m = PilotStatusRollNeeded[BOUNDED(0, mech_pilot_status(mech), 4)];
  }
  if (initial && mech_pilot_is_unconscious(mech))
    return 0;
  if (HasBoolAdvantage(mech_context(mech), mech_pilot_dbref(mech), "toughness"))
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
  int damage, bruise, lethaldam, playerBLD;

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

  bruise = char_gbruise(s);
  /* gets the players bruise damage */

  playerBLD = char_gvalue(s, "build");
  /* get the player's BLD value */

  damage = 2 * playerBLD * dam;
  /* the damage we are due */

  bruise += damage;
  /* this part subtracts 10 from players lethal damage */

  if (bruise > playerBLD * 10) {
    lethaldam = char_glethal(s);
    lethaldam += (bruise - playerBLD * 10);
    bruise = playerBLD * 10;

    if (lethaldam >= playerBLD * 10) {
      lethaldam = playerBLD * 10;
      char_slethal(s, playerBLD * 10 - 1);
      char_sbruise(s, playerBLD * 10);
      character_stats_store(context, player, s, VALUES_HEALTH);
      if (!mech_is_destroyed(mech)) {
        DestroyMech(mech, attacker, 0, KILL_TYPE_MWDAMAGE);
      }
      KillMechContentsIfIC(mech);
      return;
    }
    char_slethal(s, lethaldam);
  }
  char_sbruise(s, bruise);
  character_stats_store(context, player, s, VALUES_HEALTH);
  handlemwconc(mech, 1);
  mech_pilot_status_add(mech, dam);
}

void mwlethaldam(Mech *mech, Mech *attacker, int dam) {
  BtechContext *context = mech_context(mech);
  PSTATS stats, *s = &stats;
  DbRef player;
  int lethaldam, playerBLD;

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
  playerBLD = char_gvalue(s, "build");
  if (!playerBLD)
    playerBLD++;
  lethaldam = char_glethal(s);
  lethaldam += BOUNDED(10, dam * playerBLD, 40);
  if (lethaldam >= playerBLD * 10) {
    lethaldam = playerBLD * 10;
    char_slethal(s, lethaldam - 1);
    char_sbruise(s, lethaldam);
    character_stats_store(context, player, s, VALUES_HEALTH);
    if (!mech_is_destroyed(mech)) {
      DestroyMech(mech, attacker, 0, KILL_TYPE_MWDAMAGE);
    }
    KillMechContentsIfIC(mech);
    return;
  }
  char_sbruise(s, playerBLD * 10 - 5);
  char_slethal(s, lethaldam);
  character_stats_store(context, player, s, VALUES_HEALTH);
  handlemwconc(mech, 1);
  mech_pilot_status_add(mech, dam);
}
