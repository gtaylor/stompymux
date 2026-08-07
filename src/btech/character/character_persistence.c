#include "btechstats_internal.h"
#include "registry_api.h"

void debug_xptop(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  BtechContext *context = debug->context;
  int hm, i, j;
  DbRef top[MAX_PLAYERS_ON];
  int topv[MAX_PLAYERS_ON];
  int count = 0, gt = 0;
  CoolMenu *c = NULL;
  PSTATS stats, *s = &stats;

  bzero(top, sizeof(top));
  bzero(topv, sizeof(topv));
  while (buffer && *buffer && isspace((unsigned char)*buffer))
    buffer++;
  if (!buffer || !*buffer) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid argument!");
    return;
  }
  if ((hm = char_getvaluecode(context, buffer)) < 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid value name!");
    return;
  }
  if (char_values[hm].type != CHAR_SKILL) {
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
    if (!s->xp[hm])
      continue;
    top[count] = i;
    topv[count] = s->xp[hm] % XP_MAX;
    gt += topv[count];
    count++;
  }
  for (i = 0; i < (count - 1); i++)
    for (j = i + 1; j < count; j++) {
      if (topv[j] > topv[i]) {
        topv[count] = topv[j];
        topv[j] = topv[i];
        topv[i] = topv[count];

        top[count] = top[j];
        top[j] = top[i];
        top[i] = top[count];
      }
    }
  cool_menu_add_line(&c);
  for (i = 0; i < MIN(16, count); i++) {
    cool_menu_add(&c, tprintf("%3d. %s", i + 1,
                              game_object_name(context->database, top[i])));
    cool_menu_add(&c, tprintf("%d (%.3f %%)", topv[i], (100.0 * topv[i]) / gt));
  }
  cool_menu_add_line(&c);
  if (gt) {
    cool_menu_add(&c, tprintf("Grand total: %d points", gt));
    cool_menu_add_line(&c);
  }
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
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
  if ((!((xpt) = atoi(args[1])) && strcmp((args[1]), "0"))) {
    mecha_notify(btech_context_evaluation(context), player, "Invalid value!");
    return;
  }
  if (xpt < 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Threshold needs to be >=0 (0 = no gains possible)");
    return;
  }
  if ((code = char_getvaluecode(context, args[0])) < 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That isn't any charvalue!");
    return;
  }
  if (char_values[code].type != CHAR_SKILL) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That isn't any skill!");
    return;
  }
  char_values[code].xpthreshold = xpt;
  log_error(context->log, LOG_WIZARD, "WIZ", "CHANGE",
            "Exp threshold for %s changed to %d by #%ld",
            char_values[code].name, xpt, player);
}

int btthreshold_func(BtechContext *context, char *skillname) {
  int code;

  if (!skillname || !*skillname)
    return -1;
  code = char_getvaluecode(context, skillname);
  if (code < 0)
    return -1;
  if (char_values[code].type != CHAR_SKILL)
    return -1;
  return char_values[code].xpthreshold;
}
