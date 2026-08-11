#include <stdlib.h>
#include <string.h>

#include "btech/context.h"
#include "btechstats.h"
#include "btechstats_api.h"
#include "btechstats_global.h"
#include "btechstats_internal.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "mycool.h"
#include "registry_api.h"
#include "special_object.h"

typedef struct CharacterXpRanking {
  DbRef player;
  int experience;
} CharacterXpRanking;

static CharacterXpRanking *ranking_at(CharacterXpRanking *rankings,
                                      size_t count, size_t index) {
  return checked_storage_at(rankings, count, sizeof(*rankings), index);
}

void debug_xptop(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  BtechContext *context = debug->context;
  int hm, i, j;
  CharacterXpRanking rankings[MAX_PLAYERS_ON];
  int count = 0, gt = 0;
  CoolMenu *c = NULL;
  PSTATS stats, *s = &stats;

  memset(rankings, 0, sizeof(rankings));
  const char *skill_name = buffer;
  if (skill_name != nullptr)
    skill_name =
        checked_string_suffix(skill_name, strspn(skill_name, " \t\r\n\f\v"));
  if (skill_name == nullptr || !*skill_name) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid argument!");
    return;
  }
  hm = char_getvaluecode(context, skill_name);
  if (hm < 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid value name!");
    return;
  }
  if (character_value_definition(hm)->type != CHAR_SKILL) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Only skills have XP (for now at least)");
    return;
  }
  DO_WHOLE_DB(context->database, i) {
    if (!is_player(context->database, i))
      continue;
    if (is_wizard(context->database, i))
      continue;
    character_stats_retrieve(context, i, VALUES_SKILLS, s);
    int xp = character_stats_xp_get(s, hm);
    if (!xp)
      continue;
    if (count >= MAX_PLAYERS_ON)
      break;
    CharacterXpRanking *ranking =
        ranking_at(rankings, MAX_PLAYERS_ON, (size_t)count);
    ranking->player = i;
    ranking->experience = xp % XP_MAX;
    gt += ranking->experience;
    count++;
  }
  for (i = 0; i < (count - 1); i++)
    for (j = i + 1; j < count; j++) {
      CharacterXpRanking *left =
          ranking_at(rankings, MAX_PLAYERS_ON, (size_t)i);
      CharacterXpRanking *right =
          ranking_at(rankings, MAX_PLAYERS_ON, (size_t)j);
      if (right->experience > left->experience) {
        const CharacterXpRanking TEMPORARY = *right;
        *right = *left;
        *left = TEMPORARY;
      }
    }
  cool_menu_add_line(&c);
  for (i = 0; i < min(16, count); i++) {
    const CharacterXpRanking *ranking =
        ranking_at(rankings, MAX_PLAYERS_ON, (size_t)i);
    cool_menu_add(
        &c, tprintf("%3d. %s", i + 1,
                    game_object_name(context->database, ranking->player)));
    cool_menu_add(&c, tprintf("%d (%.3f %%)", ranking->experience,
                              (100.0 * ranking->experience) / gt));
  }
  cool_menu_add_line(&c);
  if (gt) {
    cool_menu_add(&c, tprintf("Grand total: %d points", gt));
    cool_menu_add_line(&c);
  }
  show_cool_menu(btech_context_evaluation(context), player, c);
  kill_cool_menu(c);
}

void debug_setxplevel(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  BtechContext *context = debug->context;
  char *args[3];
  int xpt, code;

  if (mech_parseattributes(buffer, args, 3) != 2) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid arguments!");
    return;
  }
  if (!parse_int_checked(args[1], &xpt)) {
    mecha_notify(btech_context_evaluation(context), player, "Invalid value!");
    return;
  }
  if (xpt < 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Threshold needs to be >=0 (0 = no gains possible)");
    return;
  }
  code = char_getvaluecode(context, args[0]);
  if (code < 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That isn't any charvalue!");
    return;
  }
  const CharacterValue *definition = character_value_definition(code);
  if (definition->type != CHAR_SKILL) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That isn't any skill!");
    return;
  }
  character_value_xp_threshold_set(
      &(CharacterValueThreshold){.code = code, .threshold = xpt});
  log_error((LogEntry){.log = context->log,
                       .key = LOG_WIZARD,
                       .primary = "WIZ",
                       .secondary = "CHANGE"},
            "Exp threshold for %s changed to %d by #%ld", definition->name, xpt,
            player);
}

int btthreshold_func(BtechContext *context, char *skillname) {
  int code;

  if (!skillname || !*skillname)
    return -1;
  code = char_getvaluecode(context, skillname);
  if (code < 0)
    return -1;
  if (character_value_definition(code)->type != CHAR_SKILL)
    return -1;
  return character_value_definition(code)->xpthreshold;
}
