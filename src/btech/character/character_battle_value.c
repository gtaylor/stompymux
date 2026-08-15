#include <math.h>
#include <stdlib.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btechstats.h"
#include "btechstats_api.h"
#include "btechstats_global.h"
#include "btechstats_internal.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_progress_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "script_functions_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
#include "weapon_settings.h"

static const char *function_argument(char *const *arguments, int count,
                                     size_t index) {
  if (count < 0)
    abort();
  char *const *argument = (char *const *)checked_storage_at_const(
      (const void *)arguments, (size_t)count, sizeof(*arguments), index);
  return *argument;
}

static double zero_pilot_base_skill(int skill) {
  switch (skill) {
  case 0:
    return 2.05;
  case 1:
    return 1.85;
  case 2:
    return 1.65;
  case 3:
    return 1.45;
  case 4:
    return 1.25;
  case 5:
    return 1.15;
  case 6:
    return 1.05;
  case 7:
    return 0.95;
  default:
    abort();
  }
}

static int bth_modifier_value(int bth) {
  switch (bth) {
  case 3:
    return 1;
  case 4:
    return 3;
  case 5:
    return 6;
  case 6:
    return 10;
  case 7:
    return 15;
  case 8:
    return 21;
  case 9:
    return 26;
  case 10:
    return 30;
  case 11:
    return 33;
  case 12:
    return 35;
  default:
    return 0;
  }
}

int has_bool_advantage(BtechContext *context, DbRef player, const char *name) {
  PSTATS stats;
  PSTATS *s = &stats;
  char buf[SBUF_SIZE];

  (void)string_copy_bounded(buf, sizeof(buf), name);
  character_stats_retrieve(context, player,
                           VALUES_ATTRS | VALUES_ADVS | VALUES_HEALTH, s);
  if (char_getstatvalue(s, buf) == 1)
    return 1;
  return 0;
}

static int ton_value(const Mech *mech) {
  return max(1, mech_tonnage(mech) /
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

static double get_pilot_bv_mod(Mech *mech, int weapindx) {
  /*
   * What we do is we get the mod as if we had a 0+ piloting (baseline)
   * for the gun skill we want. Each '+' above zero subtracts .05 from
   * the result. Obviously, each '+' below adds .05.
   *
   * The first number in the array below corresponds to a 0+ 0+ person
   * and the last number in the array below corresponds to a 7+ 0+ person
   * (that's <gun skill>+ <pilot skill>+)
   */

  int my_g_skill = find_pilot_gunnery(mech, weapindx);
  int my_p_skill = find_pilot_piloting(mech);
  double base_mod = 0.0;

  /* First we check if we have a totally off the wall GSkill, i.e., below
   * 0 or above 7.
   */
  if (my_g_skill < 0) {
    const int GUNNERY_PENALTY = abs(my_g_skill);
    base_mod = zero_pilot_base_skill(0) + (GUNNERY_PENALTY * 0.20);
  } else if (my_g_skill > 7) {
    base_mod = zero_pilot_base_skill(7) - (my_g_skill * 0.10);
  } else {
    base_mod = zero_pilot_base_skill(my_g_skill);
  }

  return base_mod - (my_p_skill * 0.05);
}

/*
 * Routines and formula for XP gain.
 */
static void
legacy_gunnery_experience_award(const GunneryExperienceAward *award);

void gunnery_experience_award(const GunneryExperienceAward *award) {
  const DbRef PILOT = award->pilot;
  Mech *attacker = award->attacker;
  Mech *wounded = award->target;
  const int DAMAGE = award->damage;
  double multiplier = award->multiplier;
  const int WEAPINDX = award->weapon_index;
  const int BTH = award->base_to_hit;
  BtechContext *context = mech_context(attacker);
  int xp;
  int my_speed;
  int th_speed;
  double my_battle_value;
  double their_battle_value;
  double my_pilot_bv_mod = 1.0;
  double their_pilot_bv_mod = 1.0;
  double weap_type_mod;
  const char *skname;
  char buf[MBUF_SIZE];
  int damagemod;
  double vrtmod;
  int recycle_time;
  int weapon_battle_value;
  int i;
  int j = NUM_SECTIONS;

  weap_type_mod = 1.0;

  if (mech_context(attacker)->configuration->btech_oldxpsystem) {
    legacy_gunnery_experience_award(award);
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

  if (mech_gunner_dbref(attacker) != PILOT)
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
  skname = find_gunnery_skill_name(attacker, WEAPINDX);
  if (!skname)
    return;

  /* No xp for shooting mechwarriors if you not a mechwarrior */
  if (mech_class(wounded) == CLASS_MW && mech_class(attacker) != CLASS_MW)
    return;

  /* bth to high so no way to hit */
  if (!(BTH <= 12))
    return;

  multiplier *= mech_context(attacker)->configuration->btech_xp_modifier;

  if (mech_context(attacker)->configuration->btech_xp_bthmod) {
    if (!(BTH >= 3 && BTH <= 12)) {
      if (mech_context(attacker)->configuration->btech_noisy_xpgain)
        btech_channel_send(context, BTECH_CHANNEL_MECH_XP,
                           "#%ld in #%ld 1 noxp #%ld", PILOT,
                           mech_dbref(attacker), mech_dbref(wounded));
      return; /* sure hits aren't interesting */
    }
    multiplier = 2.0 * multiplier * bth_modifier_value(BTH) / 36.0;
  }

  /* Need to do a BV mod between the mechs */
  const int ATTACKER_BATTLE_VALUE = mech_battle_value(attacker);
  const int WOUNDED_BATTLE_VALUE = mech_battle_value(wounded);
  my_battle_value = ATTACKER_BATTLE_VALUE;
  their_battle_value = WOUNDED_BATTLE_VALUE;

  if (mech_context(attacker)->configuration->btech_xp_use_pilot_bv_mod) {
    my_pilot_bv_mod = get_pilot_bv_mod(attacker, WEAPINDX);
    their_pilot_bv_mod = get_pilot_bv_mod(wounded, WEAPINDX);

    my_battle_value *= my_pilot_bv_mod;
    their_battle_value *= their_pilot_bv_mod;
  }

  my_speed = new_move_value(attacker) + 1;
  th_speed = new_move_value(wounded) + 1;

  if (weapon_catalogue_is_missile(WEAPINDX))
    weap_type_mod = mech_context(attacker)->configuration->btech_xp_missilemod;
  else if (weapon_catalogue_is_ballistic(WEAPINDX))
    weap_type_mod = mech_context(attacker)->configuration->btech_xp_ammomod;

  if (mech_context(attacker)->configuration->btech_defaultweapdam > 1)
    damagemod = DAMAGE;
  else
    damagemod = 1;

  recycle_time =
      btech_weapon_settings_recycle_time(&context->weapon_settings, WEAPINDX);
  weapon_battle_value =
      btech_weapon_settings_battle_value(&context->weapon_settings, WEAPINDX);
  if (mech_context(attacker)->configuration->btech_xp_vrtmod)
    vrtmod = (recycle_time < 30 ? sqrt((double)recycle_time / 30.0) : 1.0);
  else
    vrtmod = 1.0;

  multiplier =
      (vrtmod * weap_type_mod * multiplier *
       sqrt((their_battle_value + 1.0) * th_speed *
            mech_context(attacker)->configuration->btech_defaultweapbv /
            mech_context(attacker)->configuration->btech_defaultweapdam)) /
      (sqrt((my_battle_value + 1.0) * my_speed * weapon_battle_value /
            damagemod));

  if (mech_context(attacker)->configuration->btech_perunit_xpmod) {
    const double EXPERIENCE_MODIFIER = mech_experience_modifier(attacker);
    multiplier *= EXPERIENCE_MODIFIER;
  }

  /* Change the Cap to be variable depending on what a mux wants */

  xp = bounded(1, (int)(multiplier * (double)DAMAGE / 100.0),
               mech_context(attacker)->configuration->btech_xpgain_cap);

  (void)string_copy_bounded(
      buf, sizeof(buf),
      game_object_name(mech_context(attacker)->database, mech_dbref(wounded)));

  // Emit XP gain over MechAttackXP
  if (char_gainxp(context, PILOT, skname, xp)) {
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_ATTACK_XP,
        "%s gained %d gun XP from feat of %f/100 difficulty "
        "(%d damage) against %s",
        game_object_name(mech_context(attacker)->database, PILOT), xp,
        multiplier, DAMAGE, buf);
    if (mech_context(attacker)->configuration->btech_noisy_xpgain)
      btech_channel_send(context, BTECH_CHANNEL_MECH_XP,
                         "#%ld in #%ld %d damage #%ld", PILOT,
                         mech_dbref(attacker), DAMAGE, mech_dbref(wounded));
  }

} // end AccumulateGunXP()

static void
legacy_gunnery_experience_award(const GunneryExperienceAward *award) {
  const DbRef PILOT = award->pilot;
  Mech *attacker = award->attacker;
  Mech *wounded = award->target;
  const int NUM_OCCURENCES = award->damage;
  double multiplier = award->multiplier;
  const int WEAPINDX = award->weapon_index;
  const int BTH = award->base_to_hit;
  BtechContext *context = mech_context(attacker);
  int xp;
  const char *skname;
  char buf[MBUF_SIZE];

  /* Is the attacker in character ie: in simulators */
  if (!is_in_character(mech_context(attacker)->database, mech_dbref(attacker)))
    return;

  if (!mech_has_active_gunner(attacker))
    return;

  if (mech_gunner_dbref(attacker) != PILOT)
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

  skname = find_gunnery_skill_name(attacker, WEAPINDX);
  if (!skname)
    return;

  /* No xp for shooting a mechwarrior unless you a mechwarrior */
  if (mech_class(wounded) == CLASS_MW && mech_class(attacker) != CLASS_MW)
    return;

  if (!(BTH >= 3 && BTH <= 12))
    return; /* sure hits aren't interesting */

  if (mech_tonnage(attacker) > 0) {
    multiplier =
        multiplier *
        bounded(50, 100 * ton_value(wounded) / ton_value(attacker), 150);
  } else {
    /* Bring this to the attention of the admins */
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_ERRORS,
        "AccumulateGunXP: Weird tonnage for IC mech #%ld (%s): %d",
        mech_dbref(attacker),
        game_object_name(mech_context(attacker)->database,
                         mech_dbref(attacker)),
        (short)mech_tonnage(attacker));
    return;
  }

  /* Hmm.. we have to figure the speed differences as well */
  {
    int my_speed = move_value(attacker);
    int th_speed = move_value(wounded);

    multiplier = multiplier * th_speed * th_speed / my_speed / my_speed;
  }

  multiplier = multiplier * bth_modifier_value(BTH) / 36;
  multiplier = multiplier * 2; /* For average shot */
  if (mech_context(attacker)->configuration->btech_perunit_xpmod) {
    const double EXPERIENCE_MODIFIER = mech_experience_modifier(attacker);
    multiplier *= EXPERIENCE_MODIFIER;
  }

  if (btech_random_range_int(mech_context(attacker), 1, 50) >
      (multiplier * NUM_OCCURENCES))
    return; /* Nothing for truly twinky stuff, occasionally */

  xp = bounded(1, (int)(multiplier * NUM_OCCURENCES) / 100,
               50); /*Hardcoded limit */
  (void)string_copy_bounded(
      buf, sizeof(buf),
      game_object_name(mech_context(attacker)->database, mech_dbref(wounded)));
  /* Switching to Exile method of tracking xp, where we split
   * Attacking and Piloting xp into two different channels
   */
  if (char_gainxp(context, PILOT, skname, xp)) {
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_ATTACK_XP,
        "%s gained %d gun XP from feat of %f %% "
        "difficulty (%d occurences) against %s",
        game_object_name(mech_context(attacker)->database, PILOT), xp,
        multiplier, NUM_OCCURENCES, buf);
  }
}

BtechScriptResult fun_btgetcharvalue(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *evaluation = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  BtechContext *context = evaluation->btech;
  PSTATS stats;
  /* fargs[0] = char id (#222)
     fargs[1] = value name / value loc #
     fargs[2] = flaggo (?) */
  DbRef target;
  int targetcode;
  int flaggo;

  target = character_lookup(&(CharacterLookupRequest){
      .context = context,
      .viewer = PLAYER,
      .name = function_argument(fargs, NFARGS, 0),
  });
  if (target == NOTHING) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (!is_wizard(context->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED!");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  const char *value_name = function_argument(fargs, NFARGS, 1);
  if (!parse_int_checked(value_name, &targetcode))
    targetcode = char_getvaluecode(context, value_name);
  if (targetcode < 0 || targetcode >= NUM_CHARVALUES) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID VALUE");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (!parse_int_checked(function_argument(fargs, NFARGS, 2), &flaggo)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID FLAG");
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (character_value_definition(targetcode)->type == CHAR_SKILL &&
      flaggo == 4) {
    safe_tprintf_str(buff, bufc, "%d",
                     character_xp_to_next_level(context, target, targetcode));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (character_value_definition(targetcode)->type == CHAR_SKILL &&
      flaggo == 3) {
    character_stats_retrieve(context, target, VALUES_SKILLS, &stats);
    safe_tprintf_str(buff, bufc, "%d",
                     character_stats_value_get(&stats, targetcode));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (character_value_definition(targetcode)->type == CHAR_SKILL &&
      flaggo == 2) {
    safe_tprintf_str(
        buff, bufc, "%d",
        char_getxpbycode(&(CharacterValueRequest){
            .context = context, .player = target, .code = targetcode}));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  if (character_value_definition(targetcode)->type == CHAR_SKILL && flaggo) {
    safe_tprintf_str(buff, bufc, "%d",
                     char_getskilltargetbycode(context, target, targetcode, 0));
    return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
  }
  safe_tprintf_str(
      buff, bufc, "%d",
      character_value_by_code(&(CharacterValueRequest){
          .context = context, .player = target, .code = targetcode}));

  return btech_script_result_finish(call, BTECH_SCRIPT_NUMBER);
}

BtechScriptResult fun_btsetcharvalue(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *evaluation = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  BtechContext *context = evaluation->btech;
  /* fargs[0] = char id (#222)
     fargs[1] = value name / value loc #
     fargs[2] = value to be set
     fargs[3] = flaggo (?)
   */
  DbRef target;
  int targetcode;
  int targetvalue;
  int flaggo;

  target = character_lookup(&(CharacterLookupRequest){
      .context = context,
      .viewer = PLAYER,
      .name = function_argument(fargs, NFARGS, 0),
  });
  if (target == NOTHING) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID TARGET");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!is_wizard(context->database, PLAYER)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED!");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  const char *value_name = function_argument(fargs, NFARGS, 1);
  if (!parse_int_checked(value_name, &targetcode))
    targetcode = char_getvaluecode(context, value_name);
  if (targetcode < 0 || targetcode >= NUM_CHARVALUES) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID VALUE");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  if (!parse_int_checked(function_argument(fargs, NFARGS, 2), &targetvalue) ||
      !parse_int_checked(function_argument(fargs, NFARGS, 3), &flaggo)) {
    safe_tprintf_str(buff, bufc, "#-1 INVALID VALUE");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }

  /* We supposedly have everything at hand.. */
  if (flaggo) {
    if (character_value_definition(targetcode)->type != CHAR_SKILL) {
      safe_tprintf_str(buff, bufc, "#-1 ONLY SKILLS CAN HAVE FLAG");
      return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
    }
  }
  switch (flaggo) {
  case 0:
    /* this is the # of skill points in said skill
     * Also Known as Level. This is not the + value
     * I.e. Setting someone to Level 2 Gun-Bmech with A Physical Attribute of 7+
     * will give you a 5+ in Gun-Bmech */
    character_value_set_by_code(&(CharacterValueChange){
        .target = {.context = context, .player = target, .code = targetcode},
        .value = targetvalue});
    safe_tprintf_str(
        buff, bufc, "%s's %s set to %d",
        game_object_name(context->database, target),
        character_value_definition(targetcode)->name,
        character_value_by_code(&(CharacterValueRequest){
            .context = context, .player = target, .code = targetcode}));
    break;

  case 1:
    /* This is the + value of said skill
     * Also known as the ToHit Roll. This is not the 'Skill Level'
     * I.e. Setting someone's Gun-Bmech with this to 5 with a Physical Attribute
     * of 7+ will give you Level 2 Gun-Bmech (5+) */

    character_value_set_by_code(&(CharacterValueChange){
        .target = {.context = context, .player = target, .code = targetcode}});
    targetvalue =
        char_getskilltargetbycode(context, target, targetcode, 0) - targetvalue;

    /* Handle a wierd code race issue. target shouldn't be negative in this case
     * anyways */
    if (targetvalue >= 0) {
      character_value_set_by_code(&(CharacterValueChange){
          .target = {.context = context, .player = target, .code = targetcode},
          .value = targetvalue});
    } else {
      character_value_set_by_code(&(CharacterValueChange){
          .target = {
              .context = context, .player = target, .code = targetcode}});
    }

    safe_tprintf_str(
        buff, bufc, "%s's %s set to %d",
        game_object_name(context->database, target),
        character_value_definition(targetcode)->name,
        targetvalue >= 0
            ? character_value_by_code(&(CharacterValueRequest){
                  .context = context, .player = target, .code = targetcode})
            : 0);

    break;

  case 3:
    /* Set the XP Amount for this skill */
    char_gainxpbycode(&(CharacterExperienceChange){
        .target = {.context = context, .player = target, .code = targetcode},
        .amount =
            targetvalue -
            char_getxpbycode(&(CharacterValueRequest){
                .context = context, .player = target, .code = targetcode}),
        .override_interval = true});

    btech_channel_send(
        context, BTECH_CHANNEL_MECH_XP, "%ld set %ld's %s XP to %d", PLAYER,
        target, character_value_definition(targetcode)->name, targetvalue);
    safe_tprintf_str(buff, bufc, "%s's %s XP set to %d.",
                     game_object_name(context->database, target),
                     character_value_definition(targetcode)->name, targetvalue);

    break;

  default:
    /* Any other flaggo value will addxp for the skill */
    char_gainxpbycode(&(CharacterExperienceChange){
        .target = {.context = context, .player = target, .code = targetcode},
        .amount = targetvalue,
        .override_interval = true});
    btech_channel_send(context, BTECH_CHANNEL_MECH_XP,
                       "#%ld added %d more %s XP to #%ld", PLAYER, targetvalue,
                       character_value_definition(targetcode)->name, target);
    safe_tprintf_str(buff, bufc, "%s gained %d more %s XP.",
                     game_object_name(context->database, target), targetvalue,
                     character_value_definition(targetcode)->name);

    break;
  }

  return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
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
BtechScriptResult fun_btcharlist(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *evaluation = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
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
  static const char *const CMDS[] = {"skills", "advantages", "attributes",
                                     nullptr};

  if (!argument_count_in_range("BTCHARLIST", NFARGS, 1, 2, buff, bufc))
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);

  if (NFARGS == 2) {
    target = character_lookup(&(CharacterLookupRequest){
        .context = context,
        .viewer = PLAYER,
        .name = function_argument(fargs, NFARGS, 1),
    });
    if (target == NOTHING) {
      safe_str("#-1 FUNCTION (BTCHARLIST) INVALID TARGET", buff, bufc);
      return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
    }
  }

  switch (listmatch(CMDS, 3, fargs[0])) {
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
    return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
  }

  for (i = 0; i < NUM_CHARVALUES; ++i) {
    const CharacterValue *definition = character_value_definition(i);
    if (type == definition->type) {
      if (NFARGS == 2 && type != CHAR_ATTRIBUTE) {
        int targetcode = char_getvaluecode(context, definition->name);
        if (character_value_by_code(&(CharacterValueRequest){
                .context = context, .player = target, .code = targetcode}) ==
                0 &&
            (type == CHAR_SKILL &&
             char_getxpbycode(&(CharacterValueRequest){
                 .context = context, .player = target, .code = targetcode}) ==
                 0))
          continue;
      }
      if (first)
        first = 0;
      else
        safe_str(" ", buff, bufc);
      safe_str(definition->name, buff, bufc);
    }
  }
  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);

  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}
