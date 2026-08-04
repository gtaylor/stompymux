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

static void store_health(BtechContext *context, DbRef player, PSTATS *s) {
  silly_atr_set_in(
      context->database, player, A_HEALTH,
      tprintf("%d,%d", char_gvalue(s, "Bruise"), char_gvalue(s, "Lethal")));
}

static void retrieve_health(BtechContext *context, DbRef player, PSTATS *s) {
  char *c = btech_attribute_read(context->database, player, A_HEALTH,
                                 (char[LBUF_SIZE]){0});
  PSTATS *s1;
  int i1, i2;

  if (sscanf(c, "%d,%d", &i1, &i2) != 2) {
    s1 = character_stats_create();
    memcpy(s, s1, sizeof(PSTATS));
    character_stats_store(context, player, s, VALUES_ALL);
    free((void *)s1);
    return;
  }
  char_svalue(s, "Bruise", i1);
  char_svalue(s, "Lethal", i2);
}

static void store_attrs(BtechContext *context, DbRef player, PSTATS *s) {
  silly_atr_set_in(context->database, player, A_ATTRS,
                   tprintf("%d,%d,%d,%d,%d", char_gvalue(s, "Build"),
                           char_gvalue(s, "Reflexes"),
                           char_gvalue(s, "Intuition"), char_gvalue(s, "Learn"),
                           char_gvalue(s, "Charisma")));
}

static void retrieve_attrs(BtechContext *context, DbRef player, PSTATS *s) {
  char *c = btech_attribute_read(context->database, player, A_ATTRS,
                                 (char[LBUF_SIZE]){0});
  PSTATS *s1;
  int i1, i2, i3, i4, i5;

  if (sscanf(c, "%d,%d,%d,%d,%d", &i1, &i2, &i3, &i4, &i5) != 5) {
    s1 = character_stats_create();
    memcpy(s, s1, sizeof(PSTATS));
    character_stats_store(context, player, s, VALUES_ALL);
    free((void *)s1);
    return;
  }
  char_svalue(s, "Build", i1);
  char_svalue(s, "Reflexes", i2);
  char_svalue(s, "Intuition", i3);
  char_svalue(s, "Learn", i4);
  char_svalue(s, "Charisma", i5);
}

static void generic_retrieve_stuff(BtechContext *context, DbRef player,
                                   PSTATS *s, int attrnum) {
  char *c = btech_attribute_read(context->database, player, attrnum,
                                 (char[LBUF_SIZE]){0}),
       *e;
  char buf[512];
  int i1, i2, i3, sn;

  if (!*c)
    return;
  while (1) {
    i2 = i3 = 0;
    e = strchr(c, '/');
    if (sscanf(c, "%[A-Za-z_-]:%d,%d,%d", buf, &i1, &i2, &i3) < 2)
      return;
    /* Do the magic ;) */
    sn = char_getvaluecode(context, buf);
    if (sn >= 0) {
      s->values[sn] = i1;
      if (i2)
        s->xp[sn] = i2;
      if (i3)
        s->last_use[sn] = i3;
    }
    if (!(c = e))
      return;
    c++;
    if (!(*c))
      return;
  }
}

static void generic_store_stuff(BtechContext *context, DbRef player, PSTATS *s,
                                int attrnum, int flag) {
  char buf[LBUF_SIZE] = {0};
  int i;
  char *c;

  c = buf;
  for (i = 0; i < (int)(NUM_CHARVALUES); i++) {
    if (!s->values[i] && !s->xp[i])
      continue;
    if (flag) {
      if (char_values[i].type != CHAR_SKILL)
        continue;
    } else if (i != 5 && char_values[i].type != CHAR_ADVANTAGE)
      continue;
    if (s->xp[i])
      snprintf(c, buf - c, "%s:%d,%d,%d/", context->char_value_short_names[i],
               s->values[i], s->xp[i], (int)s->last_use[i]);
    else
      snprintf(c, buf - c, "%s:%d/", context->char_value_short_names[i],
               s->values[i]);
    while (*(++c))
      ;
  }
  if (*buf)
    silly_atr_set_in(context->database, player, attrnum, buf);
  else
    silly_atr_set_in(context->database, player, attrnum, "");
}

static void retrieve_skills(BtechContext *context, DbRef player, PSTATS *s) {
  generic_retrieve_stuff(context, player, s, A_SKILLS);
}

static void retrieve_advs(BtechContext *context, DbRef player, PSTATS *s) {
  generic_retrieve_stuff(context, player, s, A_ADVS);
}

static void store_skills(BtechContext *context, DbRef player, PSTATS *s) {
  generic_store_stuff(context, player, s, A_SKILLS, 1);
}

static void store_advs(BtechContext *context, DbRef player, PSTATS *s) {
  generic_store_stuff(context, player, s, A_ADVS, 0);
}

void character_stats_store(BtechContext *context, DbRef player, PSTATS *s,
                           int modes) {
  if (!is_player(context->database, player))
    return;
  if (modes & VALUES_HEALTH)
    store_health(context, player, s);
  if (modes & VALUES_ATTRS)
    store_attrs(context, player, s);
  if (modes & VALUES_ADVS) {
    if (player == context->cached_target_character)
      context->cached_target_character = -1;
    store_advs(context, player, s);
  }
  if (modes & VALUES_SKILLS) {
    if (player == context->cached_target_character)
      context->cached_target_character = -1;
    store_skills(context, player, s);
  }
}

void character_stats_retrieve(BtechContext *context, DbRef player, int modes,
                              PSTATS *stats) {
  bzero(stats, sizeof(*stats));
  if (modes & VALUES_HEALTH)
    retrieve_health(context, player, stats);
  if (modes & VALUES_ADVS)
    retrieve_advs(context, player, stats);
  if (modes & VALUES_ATTRS)
    retrieve_attrs(context, player, stats);
  if (modes & VALUES_SKILLS)
    retrieve_skills(context, player, stats);
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
