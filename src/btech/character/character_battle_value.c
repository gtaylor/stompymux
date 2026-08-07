#include "btechstats_internal.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_move_api.h"
#include "mech_runtime_api.h"

int HasBoolAdvantage(BtechContext *context, DbRef player, const char *name) {
  PSTATS stats, *s = &stats;
  char buf[SBUF_SIZE];

  strcpy(buf, name);
  character_stats_retrieve(context, player,
                           VALUES_ATTRS | VALUES_ADVS | VALUES_HEALTH, s);
  if (char_getstatvalue(s, buf) == 1)
    return 1;
  else
    return 0;
}

const int bth_modifier[] = /* Starts from '3' , in 1/36's */
    {
        /*  3 4 5  6  7  8  9 10 11 12 */
        1, 3, 6, 10, 15, 21, 26, 30, 33, 35, 0, 0, 0, 0 /* pad, just in case */
};

static int ton_value(const Mech *mech) {
  return MAX(1, mech_tonnage(mech) /
                    ((mech_class(mech) != CLASS_MECH) ? 2 : 1) /
                    ((mech_movement_type(mech) == MOVE_NONE) ? 2 : 1));
}

static int t_mod(float sp) {
  if (sp <= MP2)
    return 0;
  if (sp <= MP4)
    return 1;
  if (sp <= MP6)
    return 2;
  if (sp <= MP9)
    return 3;
  return 4; /* No extra mods */
}

static int move_value(Mech *mech) {
  return t_mod(mech_cargo_maximum_speed(mech, mech_maximum_speed(mech))) + 2;
}

static int new_move_value(const Mech *mech) {
  return (int)(mech_maximum_speed(mech) / MP1);
}

float getPilotBVMod(Mech *mech, int weapindx) {
  /*
   * What we do is we get the mod as if we had a 0+ piloting (baseline)
   * for the gun skill we want. Each '+' above zero subtracts .05 from
   * the result. Obviously, each '+' below adds .05.
   *
   * The first number in the array below corresponds to a 0+ 0+ person
   * and the last number in the array below corresponds to a 7+ 0+ person
   * (that's <gun skill>+ <pilot skill>+)
   */

  float zeroPilotBaseSkills[] = {2.05, 1.85, 1.65, 1.45, 1.25, 1.15, 1.05, .95};

  int myGSkill = FindPilotGunnery(mech, weapindx);
  int myPSkill = FindPilotPiloting(mech);
  float baseMod = 0.0;

  /* First we check if we have a totally off the wall GSkill, i.e., below
   * 0 or above 7.
   */
  if (myGSkill < 0) {
    baseMod = zeroPilotBaseSkills[0] + (abs(myGSkill) * 0.20);
  } else if (myGSkill > 7) {
    baseMod = zeroPilotBaseSkills[7] - (myGSkill * 0.10);
  } else {
    baseMod = zeroPilotBaseSkills[myGSkill];
  }

  return (baseMod - ((0 + myPSkill) * 0.05));
}

/*
 * Routines and formula for XP gain.
 */
void AccumulateGunXP(DbRef pilot, Mech *attacker, Mech *wounded, int damage,
                     float multiplier, int weapindx, int bth) {
  BtechContext *context = mech_context(attacker);
  int xp, my_BV, th_BV, my_speed, th_speed;
  float myPilotBVMod = 1.0, theirPilotBVMod = 1.0;
  float weapTypeMod;
  char *skname;
  char buf[MBUF_SIZE];
  int damagemod;
  float vrtmod;
  int recycle_time;
  int weapon_battle_value;
  int i;
  int j = NUM_SECTIONS;

  weapTypeMod = 1;

  if (mech_context(attacker)->configuration->btech_oldxpsystem) {
    AccumulateGunXPold(pilot, attacker, wounded, damage, multiplier, weapindx,
                       bth);
    return;
  }

  /* No XP for zero'd mechas */
  for (i = 0; i < NUM_SECTIONS; i++)
    j -= mech_section_is_destroyed(wounded, i);

  if (j < 1)
    return;

  /* Is attacker in character ie: not in simulator */
  if (!is_in_character(mech_context(attacker)->database, mech_dbref(attacker)))
    return;

  if (mech_suppresses_gunnery_experience(
          wounded)) /* No Gun XP for shooting this (Boxes, etc) */
    return;

  if (!mech_has_active_gunner(attacker))
    return;

  if (mech_gunner_dbref(attacker) != pilot)
    return;

  /* No xp for shooting yourself */
  if (attacker == wounded)
    return;

  /* No xp for shooting destroyed mechs */
  if (mech_is_destroyed(wounded))
    return;

  /* No xp for shooting a teammate */
  if (mech_team(wounded) == mech_team(attacker))
    return;

  /* Is the target in character ie: in simulators */
  if (!is_in_character(mech_context(attacker)->database, mech_dbref(wounded)))
    return;

  /* No skill to match the weapon we're shooting with? */
  if (!(skname = FindGunnerySkillName(attacker, weapindx)))
    return;

  /* No xp for shooting mechwarriors if you not a mechwarrior */
  if (mech_class(wounded) == CLASS_MW && mech_class(attacker) != CLASS_MW)
    return;

  /* bth to high so no way to hit */
  if (!(bth <= 12))
    return;

  multiplier =
      multiplier * mech_context(attacker)->configuration->btech_xp_modifier;

  if (mech_context(attacker)->configuration->btech_xp_bthmod) {
    if (!(bth >= 3 && bth <= 12)) {
      if (mech_context(attacker)->configuration->btech_noisy_xpgain)
        btech_channel_send(context, BTECH_CHANNEL_MECH_XP, "%s",
                           tprintf("#%ld in #%ld 1 noxp #%ld", pilot,
                                   mech_dbref(attacker), mech_dbref(wounded)));
      return; /* sure hits aren't interesting */
    }
    multiplier = 2 * multiplier * bth_modifier[bth - 3] / 36;
  }

  /* Need to do a BV mod between the mechs */
  my_BV = mech_battle_value(attacker);
  th_BV = mech_battle_value(wounded);

  if (mech_context(attacker)->configuration->btech_xp_usePilotBVMod) {
    myPilotBVMod = getPilotBVMod(attacker, weapindx);
    theirPilotBVMod = getPilotBVMod(wounded, weapindx);

    my_BV = my_BV * myPilotBVMod;
    th_BV = th_BV * theirPilotBVMod;

#ifdef XP_DEBUG
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_DEBUG, "%s",
        tprintf("Using skill modified battle value for mechs %ld and %ld "
                "with skill mods of %2.2f and %2.2f",
                mech_dbref(attacker), mech_dbref(wounded), myPilotBVMod,
                theirPilotBVMod));
#endif
  }

  my_speed = new_move_value(attacker) + 1;
  th_speed = new_move_value(wounded) + 1;

  if (MechWeapons[weapindx].type == TMISSILE)
    weapTypeMod = mech_context(attacker)->configuration->btech_xp_missilemod;
  else if (MechWeapons[weapindx].type == TAMMO)
    weapTypeMod = mech_context(attacker)->configuration->btech_xp_ammomod;

  if (mech_context(attacker)->configuration->btech_defaultweapdam > 1)
    damagemod = damage;
  else
    damagemod = 1;

  recycle_time =
      btech_weapon_settings_recycle_time(&context->weapon_settings, weapindx);
  weapon_battle_value =
      btech_weapon_settings_battle_value(&context->weapon_settings, weapindx);
  if (mech_context(attacker)->configuration->btech_xp_vrtmod)
    vrtmod = (recycle_time < 30 ? sqrt((double)recycle_time / 30.0) : 1);
  else
    vrtmod = 1.0;

  multiplier =
      (vrtmod * weapTypeMod * multiplier *
       sqrt((double)(th_BV + 1) * th_speed *
            mech_context(attacker)->configuration->btech_defaultweapbv /
            mech_context(attacker)->configuration->btech_defaultweapdam)) /
      (sqrt((double)(my_BV + 1) * my_speed * weapon_battle_value / damagemod));

  if (mech_context(attacker)->configuration->btech_perunit_xpmod)
    multiplier =
        multiplier * mech_experience_modifier(
                         attacker); /* Per unit XP Mod. Defaults to 1 anyways */

  /* Change the Cap to be variable depending on what a mux wants */

  xp = BOUNDED(1, (int)(multiplier * damage / 100),
               mech_context(attacker)->configuration->btech_xpgain_cap);

  strcpy(buf, game_object_name(mech_context(attacker)->database,
                               mech_dbref(wounded)));

  // Emit XP gain over MechAttackXP
  if (char_gainxp(context, pilot, skname, (int)xp)) {
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_ATTACK_XP, "%s",
        tprintf("%s gained %d gun XP from feat of %f/100 difficulty "
                "(%d damage) against %s",
                game_object_name(mech_context(attacker)->database, pilot),
                (int)xp, multiplier, damage, buf));
    if (mech_context(attacker)->configuration->btech_noisy_xpgain)
      btech_channel_send(context, BTECH_CHANNEL_MECH_XP, "%s",
                         tprintf("#%ld in #%ld %d damage #%ld", pilot,
                                 mech_dbref(attacker), damage,
                                 mech_dbref(wounded)));
  }

} // end AccumulateGunXP()

void AccumulateGunXPold(DbRef pilot, Mech *attacker, Mech *wounded,
                        int numOccurences, float multiplier, int weapindx,
                        int bth) {
  BtechContext *context = mech_context(attacker);
  int xp;
  char *skname;
  char buf[MBUF_SIZE];

  /* Is the attacker in character ie: in simulators */
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

  /* No xp for shooting teammate */
  if (mech_team(wounded) == mech_team(attacker))
    return;

  /* if target is in character ie: in simulators or something */
  if (!is_in_character(mech_context(attacker)->database, mech_dbref(wounded)))
    return;

  if (!(skname = FindGunnerySkillName(attacker, weapindx)))
    return;

  /* No xp for shooting a mechwarrior unless you a mechwarrior */
  if (mech_class(wounded) == CLASS_MW && mech_class(attacker) != CLASS_MW)
    return;

  if (!(bth >= 3 && bth <= 12))
    return; /* sure hits aren't interesting */

  if (mech_tonnage(attacker) > 0)
    multiplier =
        multiplier *
        BOUNDED(50, 100 * ton_value(wounded) / ton_value(attacker), 150);
  else {
    /* Bring this to the attention of the admins */
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("AccumulateGunXP: Weird tonnage for IC mech #%ld (%s): %d",
                mech_dbref(attacker),
                game_object_name(mech_context(attacker)->database,
                                 mech_dbref(attacker)),
                (short)mech_tonnage(attacker)));
    return;
  }

  /* Hmm.. we have to figure the speed differences as well */
  {
    int my_speed = move_value(attacker);
    int th_speed = move_value(wounded);

    multiplier = multiplier * th_speed * th_speed / my_speed / my_speed;
  }

  multiplier = multiplier * bth_modifier[bth - 3] / 36;
  multiplier = multiplier * 2; /* For average shot */
  if (mech_context(attacker)->configuration->btech_perunit_xpmod)
    multiplier =
        multiplier * mech_experience_modifier(
                         attacker); /* Per unit XP Modifier. Defaults to 1 */

  if (btech_random_range(mech_context(attacker), 1, 50) >
      (multiplier * numOccurences))
    return; /* Nothing for truly twinky stuff, occasionally */

  xp = BOUNDED(1, (int)(multiplier * numOccurences) / 100,
               50); /*Hardcoded limit */
  strcpy(buf, game_object_name(mech_context(attacker)->database,
                               mech_dbref(wounded)));
  /* Switching to Exile method of tracking xp, where we split
   * Attacking and Piloting xp into two different channels
   */
  if (char_gainxp(context, pilot, skname, (int)xp))
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_ATTACK_XP, "%s",
        tprintf("%s gained %d gun XP from feat of %f %% "
                "difficulty (%d occurences) against %s",
                game_object_name(mech_context(attacker)->database, pilot),
                (int)xp, multiplier, numOccurences, buf));
}

void fun_btgetcharvalue(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *evaluation) {
  BtechContext *context = evaluation->btech;
  PSTATS stats;
  /* fargs[0] = char id (#222)
     fargs[1] = value name / value loc #
     fargs[2] = flaggo (?) */
  DbRef target;
  int targetcode, flaggo;

  if ((target = char_lookupplayer(context, player, cause, 0, fargs[0])) ==
      NOTHING) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  if (!is_wizard(context->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED!");
    return;
  }
  if ((!((targetcode) = atoi(fargs[1])) && strcmp((fargs[1]), "0")))
    targetcode = char_getvaluecode(context, fargs[1]);
  if (targetcode < 0 || targetcode >= (int)(NUM_CHARVALUES)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID VALUE");
    return;
  }
  flaggo = atoi(fargs[2]);
  if (char_values[targetcode].type == CHAR_SKILL && flaggo == 4) {
    safe_tprintf_str(buff, bufc, "%d",
                     character_xp_to_next_level(context, target, targetcode));
    return;
  }
  if (char_values[targetcode].type == CHAR_SKILL && flaggo == 3) {
    character_stats_retrieve(context, target, VALUES_SKILLS, &stats);
    safe_tprintf_str(buff, bufc, "%d", stats.values[targetcode]);
    return;
  }
  if (char_values[targetcode].type == CHAR_SKILL && flaggo == 2) {
    safe_tprintf_str(buff, bufc, "%d",
                     char_getxpbycode(context, target, targetcode));
    return;
  }
  if (char_values[targetcode].type == CHAR_SKILL && flaggo) {
    safe_tprintf_str(buff, bufc, "%d",
                     char_getskilltargetbycode(context, target, targetcode, 0));
    return;
  }
  safe_tprintf_str(buff, bufc, "%d",
                   character_value_by_code(context, target, targetcode));
}

void fun_btsetcharvalue(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *evaluation) {
  BtechContext *context = evaluation->btech;
  /* fargs[0] = char id (#222)
     fargs[1] = value name / value loc #
     fargs[2] = value to be set
     fargs[3] = flaggo (?)
   */
  DbRef target;
  int targetcode, targetvalue, flaggo;

  if ((target = char_lookupplayer(context, player, cause, 0, fargs[0])) ==
      NOTHING) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return;
  }
  if (!is_wizard(context->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED!");
    return;
  }
  if ((!((targetcode) = atoi(fargs[1])) && strcmp((fargs[1]), "0")))
    targetcode = char_getvaluecode(context, fargs[1]);
  if (targetcode < 0 || targetcode >= (int)(NUM_CHARVALUES)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID VALUE");
    return;
  }
  targetvalue = atoi(fargs[2]);
  flaggo = atoi(fargs[3]);

  /* We supposedly have everything at hand.. */
  if (flaggo) {
    if (char_values[targetcode].type != CHAR_SKILL) {
      safe_tprintf_str(buff, bufc, "#-1 ONLY SKILLS CAN HAVE FLAG");
      return;
    }
  }
  switch (flaggo) {
  case 0:
    /* this is the # of skill points in said skill
     * Also Known as Level. This is not the + value
     * I.e. Setting someone to Level 2 Gun-Bmech with A Physical Attribute of 7+
     * will give you a 5+ in Gun-Bmech */
    character_value_set_by_code(context, target, targetcode, targetvalue);
    safe_tprintf_str(buff, bufc, "%s's %s set to %d",
                     game_object_name(context->database, target),
                     char_values[targetcode].name,
                     character_value_by_code(context, target, targetcode));
    break;

  case 1:
    /* This is the + value of said skill
     * Also known as the ToHit Roll. This is not the 'Skill Level'
     * I.e. Setting someone's Gun-Bmech with this to 5 with a Physical Attribute
     * of 7+ will give you Level 2 Gun-Bmech (5+) */

    character_value_set_by_code(context, target, targetcode, 0);
    targetvalue =
        char_getskilltargetbycode(context, target, targetcode, 0) - targetvalue;

    /* Handle a wierd code race issue. target shouldn't be negative in this case
     * anyways */
    if (targetvalue >= 0) {
      character_value_set_by_code(context, target, targetcode, targetvalue);
    } else {
      character_value_set_by_code(context, target, targetcode, 0);
    }

    safe_tprintf_str(buff, bufc, "%s's %s set to %d",
                     game_object_name(context->database, target),
                     char_values[targetcode].name,
                     targetvalue >= 0
                         ? character_value_by_code(context, target, targetcode)
                         : 0);

    break;

  case 3:
    /* Set the XP Amount for this skill */
    char_gainxpbycode(
        context, target, targetcode,
        targetvalue - char_getxpbycode(context, target, targetcode), 1);

    btech_channel_send(context, BTECH_CHANNEL_MECH_XP, "%s",
                       tprintf("%ld set %ld's %s XP to %d", player, target,
                               char_values[targetcode].name, targetvalue));
    safe_tprintf_str(buff, bufc, "%s's %s XP set to %d.",
                     game_object_name(context->database, target),
                     char_values[targetcode].name, targetvalue);

    break;

  default:
    /* Any other flaggo value will addxp for the skill */
    char_gainxpbycode(context, target, targetcode, targetvalue, 1);
    btech_channel_send(context, BTECH_CHANNEL_MECH_XP, "%s",
                       tprintf("#%ld added %d more %s XP to #%ld", player,
                               targetvalue, char_values[targetcode].name,
                               target));
    safe_tprintf_str(buff, bufc, "%s gained %d more %s XP.",
                     game_object_name(context->database, target), targetvalue,
                     char_values[targetcode].name);

    break;
  }
}

/* ----------------------------------------------------------------------
** Syntax: btcharlist(skills|advantages|attributes[,targetplayer])
**
** Given one of the three arguments above, btcharlist returns the
** listing of each in a space delimited list.  This is basically a
** function version of +show. If the second argument is provided, only
** the skills/advantages that are learned or possessed will
** appear. For attributes the full list will be returned of since
** characters need all of them.
*/
void fun_btcharlist(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *evaluation) {
  BtechContext *context = evaluation->btech;
  int i;
  int type = 0;
  int first = 1;
  DbRef target = 0;
  enum {
    CHSKI,
    CHADV,
    CHATT,
  };
  static char *cmds[] = {"skills", "advantages", "attributes", NULL};

  if (!argument_count_in_range("BTCHARLIST", nfargs, 1, 2, buff, bufc))
    return;

  if (nfargs == 2) {
    target = char_lookupplayer(context, player, cause, 0, fargs[1]);
    if (target == NOTHING) {
      safe_str("#-1 FUNCTION (BTCHARLIST) INVALID TARGET", buff, bufc);
      return;
    }
  }

  switch (listmatch(cmds, fargs[0])) {
  case CHSKI:
    type = CHAR_SKILL;
    break;
  case CHADV:
    type = CHAR_ADVANTAGE;
    break;
  case CHATT:
    type = CHAR_ATTRIBUTE;
    break;
  default:
    safe_str("#-1 FUNCTION (BTCHARLIST) INVALID VALUE", buff, bufc);
    return;
  }

  for (i = 0; i < (int)(NUM_CHARVALUES); ++i)
    if (type == char_values[i].type) {
      if (nfargs == 2 && type != CHAR_ATTRIBUTE) {
        int targetcode = char_getvaluecode(context, char_values[i].name);
        if (character_value_by_code(context, target, targetcode) == 0 &&
            (type == CHAR_SKILL &&
             char_getxpbycode(context, target, targetcode) == 0))
          continue;
      }
      if (first)
        first = 0;
      else
        safe_str(" ", buff, bufc);
      safe_str(char_values[i].name, buff, bufc);
    }
  return;
}
