#include "btech/context.h"
#include "btech_channel.h"
#include "btechstats.h"
#include "btechstats_api.h"
#include "btechstats_global.h"
#include "btechstats_internal.h"
#include "command_handlers_api.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_progress_api.h"
#include "mech_runtime_api.h"
#include "mech_utils_api.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "special_object.h"

void character_experience_reduce(const CharacterExperienceReduction *change) {
  BtechContext *context = change->context;
  const DbRef player = change->character;
  PSTATS stats, *s = &stats;
  int i;

  character_stats_retrieve(context, player, VALUES_ALL, s);
  for (i = 0; i < (int)(NUM_CHARVALUES); i++) {
    int xp = character_stats_xp_get(s, i);
    if (!xp)
      continue;
    if (xp < 0) {
      character_stats_xp_set(
          &(CharacterStatsExperienceChange){.stats = s, .code = i});
      continue;
    }
    character_stats_xp_set(&(CharacterStatsExperienceChange){
        .stats = s,
        .code = i,
        .value = (xp % XP_MAX) * change->per_mille / 1000});
    character_stats_xp_set(&(CharacterStatsExperienceChange){
        .stats = s,
        .code = i,
        .value = character_stats_xp_get(s, i) % XP_MAX +
                 XP_MAX * figure_xp_bonus(context, player, s, i)});
  }
  character_stats_store(context, player, s, VALUES_ALL);
}

void AccumulateTechXP(BtechContext *context, DbRef pilot, Mech *mech,
                      int reason) {
  int xp;
  const char *skname;
  static const char *techw = "technician-weapons";

  if (mech) {
    skname = FindTechSkillName(mech);
    if (!skname)
      return;
  } else
    skname = techw;

  xp = MAX(1, reason);

  // We emit all tech XP gains to the MechTechXP channel.
  if (char_gainxp(context, pilot, skname, xp))
    btech_channel_send(context, BTECH_CHANNEL_MECH_TECH_XP, "%s",
                       tprintf("%s gained %d %s XP (changing mech #%ld)",
                               game_object_name(context->database, pilot), xp,
                               skname, mech ? mech_dbref(mech) : -1));
}

void AccumulateTechWeaponsXP(BtechContext *context, DbRef pilot, Mech *mech,
                             int reason) {
  const char *skname;
  int xp;
  static const char *techw = "technician-weapons";

  skname = techw;
  xp = MAX(1, reason);

  // We emit all tech xp gains to MechTechXP channel.
  if (char_gainxp(context, pilot, skname, xp))
    btech_channel_send(context, BTECH_CHANNEL_MECH_TECH_XP, "%s",
                       tprintf("%s gained %d %s XP (changing mech #%ld)",
                               game_object_name(context->database, pilot), xp,
                               skname, mech ? mech_dbref(mech) : -1));
}

void AccumulateCommXP(DbRef pilot, Mech *mech) {
  BtechContext *context = mech_context(mech);
  int xp;

  xp = 1;
  if (!mech_has_active_pilot(mech))
    return;
  if (!is_in_character(mech_context(mech)->database, mech_dbref(mech)))
    return;
  if (!is_connected(mech_context(mech)->database, pilot))
    return;
  if (char_gainxp(context, pilot, "Comm-Conventional", xp))
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_XP, "%s",
        tprintf("%s gained %d %s XP (in #%ld)",
                game_object_name(mech_context(mech)->database, pilot), xp,
                "Comm-Conventional", mech_dbref(mech)));
}

void piloting_experience_award(const PilotingExperienceAward *award) {
  Mech *mech = award->mech;
  const DbRef pilot = award->pilot;
  BtechContext *context = mech_context(mech);
  const char *skname;
  int xp;

  if (!is_in_character(mech_context(mech)->database, mech_dbref(mech)))
    return;

  if (!mech_has_active_pilot(mech))
    return;

  skname = FindPilotingSkillName(mech);
  if (!skname)
    return;

  if (!award->unconditional) {
    if (!mech_piloting_position_mark_changed(mech))
      return;
  }
  xp = MAX(1, award->reason);

  /* Switching to Exile method of tracking xp, where we split
   * Attacking and Piloting xp into two different channels
   */
  if (char_gainxp(context, pilot, skname, xp))
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_PILOT_XP, "%s",
        tprintf("%s gained %d %s XP",
                game_object_name(mech_context(mech)->database, pilot), xp,
                skname));
  /*
      if (char_gainxp(context, pilot, skname, xp))
              btech_channel_send(context, BTECH_CHANNEL_MECH_XP, tprintf("%s
     gained %d %s XP", game_object_name(mech_context(mech)->database, pilot),
     xp, skname));
  */
}

void AccumulateSpotXP(DbRef pilot, Mech *attacker, Mech *wounded) {
  BtechContext *context = mech_context(attacker);
  int xp = 1;

  if (!is_in_character(mech_context(attacker)->database, mech_dbref(attacker)))
    return;
  if (!mech_has_active_pilot(attacker))
    return;
  if (mech_pilot_dbref(attacker) != pilot)
    return;
  if (attacker == wounded)
    return;
  if (mech_is_destroyed(wounded))
    return;
  if (mech_team(wounded) == mech_team(attacker))
    return;
  if (!is_in_character(mech_context(attacker)->database, mech_dbref(wounded)))
    return;
  if (char_gainxp(context, pilot, "Gunnery-Spotting", xp))
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_XP, "%s",
        tprintf("%s gained spotting XP",
                game_object_name(mech_context(attacker)->database, pilot)));
}

int MadePerceptionRoll(Mech *mech, int modifier) {
  BtechContext *context = mech_context(mech);
  DbRef pilot;

  if (!is_in_character(mech_context(mech)->database, mech_dbref(mech)))
    return 0;
  if (!mech_has_active_gunner(mech))
    return 0;
  pilot = mech_pilot_dbref(mech);
  if (pilot <= 0)
    return 0;
  if (!mech_perception_target(mech))
    mech_perception_target_set(
        mech, char_getskilltarget(context, pilot, "Perception", 2));
  if (btech_random_roll(mech_context(mech)) <
      (mech_perception_target(mech) + modifier))
    return 0;
  if (char_gainxp(context, pilot, "Perception", 1))
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_XP, "%s",
        tprintf("%s gained 1 perception XP",
                game_object_name(mech_context(mech)->database, pilot)));
  return 1;
}

void AccumulateArtyXP(DbRef pilot, Mech *attacker, Mech *wounded) {
  BtechContext *context = mech_context(attacker);
  int xp = 1;

  /* If not in character ie: like in simulator - no xp */
  if (!is_in_character(mech_context(attacker)->database, mech_dbref(attacker)))
    return;

  if (!mech_has_active_gunner(attacker))
    return;

  if (mech_gunner_dbref(attacker) != pilot)
    return;

  /* No xp for shooting yourself */
  if (attacker == wounded)
    return;

  /* No xp for shooting destroyed units */
  if (mech_is_destroyed(wounded))
    return;

  /* No xp if both on same team */
  if (mech_team(wounded) == mech_team(attacker))
    return;

  /* If target not in character ie: in simulator - no xp */
  if (!is_in_character(mech_context(attacker)->database, mech_dbref(wounded)))
    return;

  /* Switching to Exile method of tracking xp, where we split
   * Attacking and Piloting xp into two different channels
   */
  if (char_gainxp(context, pilot, "Gunnery-Artillery", xp))
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_ATTACK_XP, "%s",
        tprintf("%s gained %d artillery XP",
                game_object_name(mech_context(attacker)->database, pilot), xp));
}

void AccumulateComputerXP(DbRef pilot, Mech *mech, int reason) {
  if (!mech)
    return;
  BtechContext *context = mech_context(mech);

  if (mech && is_in_character(mech_context(mech)->database, mech_dbref(mech)) &&
      is_player(mech_context(mech)->database, pilot))
    if (char_gainxp(context, pilot, "computer", MAX(1, reason)))
      btech_channel_send(
          context, BTECH_CHANNEL_MECH_XP, "%s",
          tprintf("%s gained %d computer XP (mech #%ld)",
                  game_object_name(mech_context(mech)->database, pilot), reason,
                  mech ? mech_dbref(mech) : -1));
}
