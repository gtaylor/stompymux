#include "btechstats_internal.h"

void lower_xp(BtechContext *context, DbRef player, int promillage) {
  PSTATS stats, *s = &stats;
  int i;

  character_stats_retrieve(context, player, VALUES_ALL, s);
  for (i = 0; i < (int)(NUM_CHARVALUES); i++) {
    if (!s->xp[i])
      continue;
    if (s->xp[i] < 0) {
      s->xp[i] = 0;
      continue;
    }
    s->xp[i] = (s->xp[i] % XP_MAX) * promillage / 1000;
    s->xp[i] =
        s->xp[i] % XP_MAX + XP_MAX * figure_xp_bonus(context, player, s, i);
  }
  character_stats_store(context, player, s, VALUES_ALL);
}

void AccumulateTechXP(BtechContext *context, DbRef pilot, Mech *mech,
                      int reason) {
  int xp;
  char *skname;
  static char *techw = "technician-weapons";

  if (mech) {
    if (!(skname = FindTechSkillName(mech)))
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
  char *skname;
  int xp;
  static char *techw = "technician-weapons";

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

void AccumulatePilXP(DbRef pilot, Mech *mech, int reason, int addanyway) {
  BtechContext *context = mech_context(mech);
  char *skname;
  int xp;

  if (!is_in_character(mech_context(mech)->database, mech_dbref(mech)))
    return;

  if (!mech_has_active_pilot(mech))
    return;

  if (!(skname = FindPilotingSkillName(mech)))
    return;

  if (!addanyway) {
    if (MechLX(mech) != MechX(mech) || MechLY(mech) != MechY(mech)) {
      MechLX(mech) = MechX(mech);
      MechLY(mech) = MechY(mech);
    } else
      return;
  }
  xp = MAX(1, reason);

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
  if (MechPilot(attacker) != pilot)
    return;
  if (attacker == wounded)
    return;
  if (Destroyed(wounded))
    return;
  if (MechTeam(wounded) == MechTeam(attacker))
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
  int pilot;

  if (!is_in_character(mech_context(mech)->database, mech_dbref(mech)))
    return 0;
  if (!mech_has_active_gunner(mech))
    return 0;
  pilot = MechPilot(mech);
  if (pilot <= 0)
    return 0;
  if (!MechPer(mech))
    MechPer(mech) = char_getskilltarget(context, pilot, "Perception", 2);
  if (btech_random_roll(mech_context(mech)) < (MechPer(mech) + modifier))
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

  if (GunPilot(attacker) != pilot)
    return;

  /* No xp for shooting yourself */
  if (attacker == wounded)
    return;

  /* No xp for shooting destroyed units */
  if (Destroyed(wounded))
    return;

  /* No xp if both on same team */
  if (MechTeam(wounded) == MechTeam(attacker))
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
