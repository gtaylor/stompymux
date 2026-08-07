#define BTECHSTATS_C
#include "btechstats_internal.h"
#include "registry_api.h"

UptimeText uptime_text(int seconds) {
  UptimeText uptime;
  char *allocated;

  allocated = get_uptime_to_string(seconds);
  snprintf(uptime.text, sizeof(uptime.text), "%s", allocated);
  free_sbuf(allocated);
  return uptime;
}

static int char_getskilltargetbycode_base(BtechContext *context, DbRef player,
                                          PSTATS *s, int code, int modifier,
                                          int use_xp);

static int char_getskilltargetbycode_noxp(BtechContext *context, DbRef player,
                                          int code, int modifier);

int figure_xp_bonus(BtechContext *context, DbRef player, PSTATS *s, int code) {
  int t = char_values[code].xpthreshold;
  int tx, bon, btar;

  if (t <= 0)
    return 0;
  /* KLUDGE */
  s->xp[code] = s->xp[code] %
                XP_MAX; /* reset exp modifier - this probably _was_ cached */
  btar = char_getskilltargetbycode_base(context, player, s, code, 0, 0);
  while (btar > 4) {
    btar--;
    t = t / 3;
  }
  while (btar < 4) {
    btar++;
    t = t * 3;
  }
  if (t < 1)
    t = 1;
  tx = s->xp[code] % XP_MAX;
  bon = 0;
  while (tx > t) {
    bon++;
    tx -= t;
    t = t * 3;
  }
  return bon;
}

int character_xp_to_next_level(BtechContext *context, DbRef target, int code) {
  int xpthresh = char_values[code].xpthreshold;
  int start_skill, target_skill, counter, running_total = 1;

  if (xpthresh <= 0)
    return -1;
  target_skill = char_getskilltargetbycode(context, target, code, 0);
  start_skill = char_getskilltargetbycode_noxp(context, target, code, 0);
  counter = start_skill;
  while (counter > 4) {
    counter--;
    xpthresh /= 3;
  }
  while (counter < 4) {
    counter++;
    xpthresh *= 3;
  }
  if (xpthresh < 1)
    xpthresh = 1;
  while (target_skill <= start_skill) {
    start_skill--;
    running_total += xpthresh;
    xpthresh *= 3;
  }
  return running_total;
}

/* Right now applies to only very few select skills */

static int char_xp_bonus(PSTATS *s, int code) { return s->xp[code] / XP_MAX; }

static int char_getstatvalue_by_code(PSTATS *stats, int code) {
  if (code < 0)
    return -1;
  return stats->values[code] + (char_values[code].type == CHAR_SKILL
                                    ? char_xp_bonus(stats, code)
                                    : 0);
}

static void char_setstatvalue_by_code(PSTATS *stats, int code, int value) {
  if (code < 0)
    return;
  if (code == EE_NUMBER)
    stats->values[LIVES_NUMBER] += value - stats->values[code];
  stats->values[code] = (unsigned char)value;
}

/*****************************/

/*     list commands        */

/*****************************/

void list_charvaluestuff(EvaluationContext *evaluation, DbRef player,
                         int flag) {
  int found = 0, ok, type;
  int i;
  char buf[80] = {0};

  if (flag == -1)
    mecha_notify(evaluation, player, "List of charvalues available:");
  if (flag >= 0) {
    notify_printf(evaluation, player,
                  "List of %s available:", btech_charvaluetype_names[flag]);
  }
  buf[0] = 0;
  for (i = 0; i < (int)(NUM_CHARVALUES); i++) {
    ok = 0;
    type = char_values[i].type;
    if (flag < 0)
      ok = 1;
    else if (type == flag)
      ok = 1;
    if (ok) {
      snprintf(buf + strlen(buf), 80 - strlen(buf), "%-23s ",
               char_values[i].name);
      if (!((++found) % 3)) {
        mecha_notify(evaluation, player, buf);
        strcpy(buf, " ");
      }
    }
  }
  if (found % 3) {
    mecha_notify(evaluation, player, buf);
  }
  mecha_notify(evaluation, player, " ");
  notify_printf(evaluation, player, "Total of %d things found.", found);
}

/*****************************/

/*     get code commands    */

/*****************************/

int char_getvaluecode(BtechContext *context, const char *name) {
  int *ip;
  char *tmpbuf, *tmpc2;
  const char *tmpc1;

  tmpbuf = alloc_sbuf("getvaluecodefind");
  for (tmpc1 = name, tmpc2 = tmpbuf;
       *tmpc1 && ((tmpbuf - tmpc2) < (SBUF_SIZE - 1)); tmpc1++, tmpc2++)
    *tmpc2 = ascii_to_lower(*tmpc1);
  *tmpc2 = 0;
  if ((ip = hash_table_find(tmpbuf, &context->player_value_hashes[0])) == NULL)
    ip = hash_table_find(tmpbuf, &context->player_value_hashes[1]);
  free_sbuf(tmpbuf);
  return ((long)ip) - 1;
}

/********************/

/*   Roll the dice  */

/********************/

int char_rollsaving(BtechContext *context) {
  int r1, r2, r3;
  int r12, r13, r23;

  r1 = char_rolld6(context, 1);
  r2 = char_rolld6(context, 1);
  r3 = char_rolld6(context, 1);

  r12 = r1 + r2;
  r13 = r1 + r3;
  r23 = r2 + r3;

  if (r12 > r13) {
    if (r12 > r23)
      return r12;
    else
      return r23;
  } else {
    if (r13 > r23)
      return r13;
    else
      return r23;
  }
}

int char_rollunskilled(BtechContext *context) {
  int r1, r2, r3;
  int r12, r13, r23;

  r1 = char_rolld6(context, 1);
  r2 = char_rolld6(context, 1);
  r3 = char_rolld6(context, 1);

  r12 = r1 + r2;
  r13 = r1 + r3;
  r23 = r2 + r3;

  if (r12 < r13) {
    if (r12 < r23)
      return r12;
    else
      return r23;
  } else {
    if (r13 < r23)
      return r13;
    else
      return r23;
  }
}

int char_rollskilled(BtechContext *context) { return char_rolld6(context, 2); }

int char_rolld6(BtechContext *context, int num) {
  int i, total = 0;

  for (i = 0; i < num; i++)
    total = total + btech_random_range(context, 1, 6);
  return (total);
}

/*****************************/

/*     DB access commands   */

/*****************************/

int char_getstatvalue(PSTATS *s, char *name) {
  for (size_t i = 0; i < NUM_CHARVALUES; i++)
    if (!strcasecmp(char_values[i].name, name))
      return char_getstatvalue_by_code(s, i);
  return -1;
}

void char_setstatvalue(PSTATS *s, char *name, int value) {
  for (size_t i = 0; i < NUM_CHARVALUES; i++)
    if (!strcasecmp(char_values[i].name, name)) {
      char_setstatvalue_by_code(s, i, value);
      return;
    }
}

int character_value_by_code(BtechContext *context, DbRef player, int code) {
  PSTATS stats;

  character_stats_retrieve(context, player, VALUES_ALL, &stats);
  return char_getstatvalue_by_code((&stats), code);
}

void character_value_set_by_code(BtechContext *context, DbRef player, int code,
                                 int value) {
  PSTATS stats;

  character_stats_retrieve(context, player, VALUES_ALL, &stats);
  char_setstatvalue_by_code((&stats), code, value);
  character_stats_store(context, player, &stats, VALUES_ALL);
}

int char_getvalue(BtechContext *context, DbRef player, char *name) {
  return character_value_by_code(context, player,
                                 char_getvaluecode(context, name));
}

void char_setvalue(BtechContext *context, DbRef player, char *name, int value) {
  character_value_set_by_code(context, player, char_getvaluecode(context, name),
                              value);
}

static int char_getskilltargetbycode_base(BtechContext *context, DbRef player,
                                          PSTATS *s, int code, int modifier,
                                          int use_xp) {
  int val, skill;

  if (code == -1)
    return 18;
  if (char_values[code].type != CHAR_SKILL)
    return 18;
  if (use_xp && context->cached_target_character == player &&
      context->cached_skill == code)
    return context->cached_skill_result + modifier;
  if (char_values[code].flag & CHAR_ATHLETIC)
    val = char_getstatvalue(s, "build") + char_getstatvalue(s, "reflexes");
  else if (char_values[code].flag & CHAR_PHYSICAL)
    val = char_getstatvalue(s, "reflexes") + char_getstatvalue(s, "intuition");
  else if (char_values[code].flag & CHAR_MENTAL)
    val = char_getstatvalue(s, "intuition") + char_getstatvalue(s, "learn");
  else if (char_values[code].flag & CHAR_PHYSICAL)
    val = char_getstatvalue(s, "reflexes") + char_getstatvalue(s, "intuition");
  else if (char_values[code].flag & CHAR_SOCIAL)
    val = char_getstatvalue(s, "intuition") + char_getstatvalue(s, "charisma");
  else
    return 18;
  if (use_xp) {
    skill = char_getstatvalue_by_code(s, code);

    if (skill == -1)
      return 18;
    context->cached_target_character = player;
    context->cached_skill = code;
    context->cached_skill_result = 18 - val - skill;
    return context->cached_skill_result + modifier;
  } else {
    skill = s->values[code];
    if (skill == -1)
      return (18);
    return 18 - val - skill;
  }
}

int char_getskilltargetbycode(BtechContext *context, DbRef player, int code,
                              int modifier) {
  PSTATS stats, *s = &stats;

  character_stats_retrieve(context, player, VALUES_CO, s);
  return char_getskilltargetbycode_base(context, player, s, code, modifier, 1);
}

static int char_getskilltargetbycode_noxp(BtechContext *context, DbRef player,
                                          int code, int modifier) {
  PSTATS stats, *s = &stats;

  character_stats_retrieve(context, player, VALUES_CO, s);
  return char_getskilltargetbycode_base(context, player, s, code, modifier, 0);
}

int char_getskilltarget(BtechContext *context, DbRef player, char *name,
                        int modifier) {
  return char_getskilltargetbycode(context, player,
                                   char_getvaluecode(context, name), modifier);
}

int char_getxpbycode(BtechContext *context, DbRef player, int code) {
  PSTATS stats, *s = &stats;

  if (code < 0)
    return 0;
  character_stats_retrieve(context, player, VALUES_SKILLS, s);
  return s->xp[code] % XP_MAX;
}

int char_gainxpbycode(BtechContext *context, DbRef player, int code, int amount,
                      int override) {
  PSTATS stats, *s = &stats;

  if (code < 0)
    return 0;
  character_stats_retrieve(context, player, VALUES_SKILLS | VALUES_ATTRS, s);
  /* allow override of setting xp quickly. useful in chargen situations and only
   * settable via that Regular skill gains still check SK_XP and last used
   * within 30s to keep from spamming
   */
  if (override == 0)
    if (!((context->clock->now > (s->last_use[code] + 30)) ||
          (char_values[code].flag & SK_XP)))
      return 0;
  s->last_use[code] = context->clock->now;
  s->xp[code] += amount;
  s->xp[code] =
      s->xp[code] % XP_MAX + XP_MAX * figure_xp_bonus(context, player, s, code);
  character_stats_store(context, player, s, VALUES_SKILLS);
  return 1;
}

int char_gainxp(BtechContext *context, DbRef player, char *skill, int amount) {
  return char_gainxpbycode(context, player, char_getvaluecode(context, skill),
                           amount, 0);
}

int char_getskillsuccess(BtechContext *context, DbRef player, char *name,
                         int modifier, int loud) {
  int roll, val;
  int code;

  code = char_getvaluecode(context, name);

  val = char_getskilltargetbycode(context, player, code, modifier);

  if (character_value_by_code(context, player, code) == 0)
    roll = char_rollunskilled(context);
  else
    roll = char_rollskilled(context);
  if (loud) {
    notify_printf(btech_context_evaluation(context), player,
                  "You make a %s skill roll!", name);
    notify_printf(btech_context_evaluation(context), player,
                  "Modified skill BTH : %d Roll : %d", val, roll);
  }

  if (roll >= val)
    return (1); /* Success! */
  else
    return (0); /* Failure */
}

int char_getskillmargsucc(BtechContext *context, DbRef player, char *name,
                          int modifier) {
  int roll, val;
  int code;

  code = char_getvaluecode(context, name);

  val = char_getskilltargetbycode(context, player, code, modifier);

  if (character_value_by_code(context, player, code) == 0)
    roll = char_rollunskilled(context);
  else
    roll = char_rollskilled(context);

  return (roll - val);
}

int char_getopposedskill(BtechContext *context, DbRef first, char *skill1,
                         DbRef second, char *skill2) {
  int per1, per2;

  per1 = char_getskillmargsucc(context, first, skill1, 0);
  per2 = char_getskillmargsucc(context, second, skill2, 0);

  if (per1 > per2)
    return (first);
  else if (per2 == per1)
    return (0);
  else
    return (second);
}

int char_getattrsave(BtechContext *context, DbRef player, char *name) {
  int val = char_getvalue(context, player, name);

  if (val == -1)
    return (-1);
  else if (val > 9)
    return 0;
  else
    return (18 - 2 * val);
}

int char_getattrsavesucc(BtechContext *context, DbRef player, char *name) {
  int roll, val = char_getattrsave(context, player, name);

  if (val == -1)
    return (-1);

  roll = char_rollskilled(context);

  if (roll >= val)
    return (1);
  else
    return (0);
}

/************************/

/*    Database Commands */

/************************/

void init_btechstats(BtechContext *context) {
  char *tmpbuf, *tmpc1, *tmpc2;
  long i;
  int j;

  context->player_value_hashes =
      calloc(2, sizeof(*context->player_value_hashes));
  context->char_value_short_names =
      calloc(NUM_CHARVALUES, sizeof(*context->char_value_short_names));
  if (context->player_value_hashes == nullptr ||
      context->char_value_short_names == nullptr)
    exit(EXIT_FAILURE);
  context->char_value_count = NUM_CHARVALUES;
  hash_table_initialize(&context->player_value_hashes[0], 20 * HASH_FACTOR);
  hash_table_initialize(&context->player_value_hashes[1], 20 * HASH_FACTOR);
  tmpbuf = alloc_sbuf("getvaluecode");
  for (i = 0; i < (int)(NUM_CHARVALUES); i++) {
    for (tmpc1 = char_values[i].name, tmpc2 = tmpbuf; *tmpc1; tmpc1++, tmpc2++)
      *tmpc2 = ascii_to_lower(*tmpc1);
    *tmpc2 = '\0';
    hash_table_add(tmpbuf, (int *)(i + 1), &context->player_value_hashes[0]);
    tmpbuf[0] = '\0';
    tmpc1 = tmpbuf;
    for (j = 0; char_values[i].name[j]; j++) {
      if (!isupper(char_values[i].name[j]))
        continue;
      strncpy(tmpc1, &char_values[i].name[j], 3);
      tmpc1 += 3;
    }
    *tmpc1 = '\0';
    if (strlen(tmpbuf) <= 3) {
      strncpy(tmpbuf, char_values[i].name, 5);
      tmpbuf[5] = '\0';
    }
    context->char_value_short_names[i] = strdup(tmpbuf);
    for (tmpc1 = tmpbuf; *tmpc1; tmpc1++)
      *tmpc1 = ascii_to_lower(*tmpc1);
    hash_table_add(tmpbuf, (int *)(i + 1), &context->player_value_hashes[1]);
  }
  free_sbuf(tmpbuf);
}

void btech_stats_destroy(BtechContext *context) {
  if (context == nullptr)
    return;

  if (context->player_value_hashes != nullptr) {
    hash_table_destroy(&context->player_value_hashes[0]);
    hash_table_destroy(&context->player_value_hashes[1]);
    free(context->player_value_hashes);
    context->player_value_hashes = nullptr;
  }
  for (size_t i = 0; i < context->char_value_count; i++)
    free(context->char_value_short_names[i]);
  free(context->char_value_short_names);
  context->char_value_short_names = nullptr;
  context->char_value_count = 0;
  context->cached_target_character = -1;
}

PSTATS *character_stats_create(void) {
  PSTATS *s;

  Create(s, PSTATS, 1);
  s->DbRef = -1;
  character_stats_clear(s);
  return s;
}
