#include "btechstats_internal.h"

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
  skipws(buffer);
  DOCHECK_CONTEXT(context, !*buffer, "Invalid argument!");
  DOCHECK_CONTEXT(context, (hm = char_getvaluecode(context, buffer)) < 0,
                  "Invalid value name!");
  DOCHECK_CONTEXT(context, char_values[hm].type != CHAR_SKILL,
                  "Only skills have XP (for now at least)");
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
  addline();
  for (i = 0; i < MIN(16, count); i++) {
    addmenu(
        tprintf("%3d. %s", i + 1, game_object_name(context->database, top[i])));
    addmenu(tprintf("%d (%.3f %%)", topv[i], (100.0 * topv[i]) / gt));
  }
  addline();
  if (gt) {
    addmenu(tprintf("Grand total: %d points", gt));
    addline();
  }
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}

void debug_setxplevel(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  BtechContext *context = debug->context;
  char *args[3];
  int xpt, code;

  DOCHECK_CONTEXT(context, mech_parseattributes(buffer, args, 3) != 2,
                  "Invalid arguments!");
  DOCHECK_CONTEXT(context, Readnum(xpt, args[1]), "Invalid value!");
  DOCHECK_CONTEXT(context, xpt < 0,
                  "Threshold needs to be >=0 (0 = no gains possible)");
  DOCHECK_CONTEXT(context, (code = char_getvaluecode(context, args[0])) < 0,
                  "That isn't any charvalue!");
  DOCHECK_CONTEXT(context, char_values[code].type != CHAR_SKILL,
                  "That isn't any skill!");
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
