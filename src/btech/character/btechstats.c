#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <time.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btechstats.h"
#include "btechstats_api.h"
#include "btechstats_global.h"
#include "btechstats_internal.h"
#include "character_value_settings.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "mech_utils_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "special_object.h"

static size_t character_value_index(int code) {
  if (code < 0 || code >= NUM_CHARVALUES)
    abort();
  return (size_t)code;
}

unsigned char character_stats_value_get(const PSTATS *stats, int code) {
  const unsigned char *value = checked_storage_at_const(
      stats->value_storage, NUM_CHARVALUES, sizeof(*stats->value_storage),
      character_value_index(code));
  return *value;
}

void character_stats_value_set(const CharacterStatsValueChange *change) {
  unsigned char *destination =
      checked_storage_at(change->stats->value_storage, NUM_CHARVALUES,
                         sizeof(*change->stats->value_storage),
                         character_value_index(change->code));
  *destination = clamp_int_to_unsigned_char(change->value);
}

int character_stats_xp_get(const PSTATS *stats, int code) {
  const int *value = checked_storage_at_const(stats->xp_storage, NUM_CHARVALUES,
                                              sizeof(*stats->xp_storage),
                                              character_value_index(code));
  return *value;
}

void character_stats_xp_set(const CharacterStatsExperienceChange *change) {
  int *destination = checked_storage_at(
      change->stats->xp_storage, NUM_CHARVALUES,
      sizeof(*change->stats->xp_storage), character_value_index(change->code));
  *destination = change->value;
}

time_t character_stats_last_use_get(const PSTATS *stats, int code) {
  const time_t *value = checked_storage_at_const(
      stats->last_use_storage, NUM_CHARVALUES, sizeof(*stats->last_use_storage),
      character_value_index(code));
  return *value;
}

void character_stats_last_use_set(const CharacterStatsLastUseChange *change) {
  time_t *destination =
      checked_storage_at(change->stats->last_use_storage, NUM_CHARVALUES,
                         sizeof(*change->stats->last_use_storage),
                         character_value_index(change->code));
  *destination = change->value;
}

static HashTable *character_value_hash(BtechContext *context, size_t index) {
  return checked_storage_at(context->player_value_hashes, 2,
                            sizeof(*context->player_value_hashes), index);
}

static char **character_short_name_slot(BtechContext *context, int code) {
  return (char **)checked_storage_at(
      (void *)context->char_value_short_names, context->char_value_count,
      sizeof(*context->char_value_short_names), character_value_index(code));
}

static void lowercase_copy(char *destination, size_t capacity,
                           const char *source) {
  const size_t SOURCE_LENGTH = strlen(source);
  const size_t COPY_LENGTH =
      SOURCE_LENGTH < capacity - 1 ? SOURCE_LENGTH : capacity - 1;
  for (size_t index = 0; index < COPY_LENGTH; index++) {
    char *output =
        checked_storage_at(destination, capacity, sizeof(char), index);
    const char *input = checked_storage_at_const(source, SOURCE_LENGTH + 1,
                                                 sizeof(char), index);
    *output = ascii_to_lower(*input);
  }
  char *terminator =
      checked_storage_at(destination, capacity, sizeof(char), COPY_LENGTH);
  *terminator = '\0';
}

UptimeText uptime_text(int seconds) {
  UptimeText uptime;
  char *allocated;

  allocated = get_uptime_to_string(seconds);
  (void)snprintf(uptime.text, sizeof(uptime.text), "%s", allocated);
  free_sbuf(allocated);
  return uptime;
}

typedef struct CharacterSkillTargetCall {
  BtechContext *context;
  DbRef player;
  PSTATS *stats;
  int code;
  int modifier;
  bool use_experience;
} CharacterSkillTargetCall;

static int char_getskilltargetbycode_base(const CharacterSkillTargetCall *call);

static int char_getskilltargetbycode_noxp(BtechContext *context, DbRef player,
                                          int code, int modifier);

int figure_xp_bonus(BtechContext *context, DbRef player, PSTATS *s, int code) {
  int t = character_value_xp_threshold(context, code);
  int tx;
  int bon;
  int btar;

  if (t <= 0)
    return 0;
  /* KLUDGE */
  character_stats_xp_set(&(CharacterStatsExperienceChange){
      .stats = s,
      .code = code,
      .value = character_stats_xp_get(s, code) % XP_MAX});
  btar = char_getskilltargetbycode_base(&(CharacterSkillTargetCall){
      .context = context, .player = player, .stats = s, .code = code});
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
  tx = character_stats_xp_get(s, code) % XP_MAX;
  bon = 0;
  while (tx > t) {
    bon++;
    tx -= t;
    t = t * 3;
  }
  return bon;
}

int character_xp_to_next_level(BtechContext *context, DbRef target, int code) {
  int xpthresh = character_value_xp_threshold(context, code);
  int start_skill;
  int target_skill;
  int counter;
  int running_total = 1;

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

static int char_xp_bonus(PSTATS *s, int code) {
  return character_stats_xp_get(s, code) / XP_MAX;
}

static int char_getstatvalue_by_code(PSTATS *stats, int code) {
  if (code < 0)
    return -1;
  return character_stats_value_get(stats, code) +
         (character_value_definition(code)->type == CHAR_SKILL
              ? char_xp_bonus(stats, code)
              : 0);
}

static void char_setstatvalue_by_code(PSTATS *stats, int code, int value) {
  if (code < 0)
    return;
  if (code == EE_NUMBER) {
    character_stats_value_set(&(CharacterStatsValueChange){
        .stats = stats,
        .code = LIVES_NUMBER,
        .value = character_stats_value_get(stats, LIVES_NUMBER) + value -
                 character_stats_value_get(stats, code)});
  }
  character_stats_value_set(&(CharacterStatsValueChange){
      .stats = stats, .code = code, .value = value});
}

/*****************************/

/*     list commands        */

/*****************************/

void list_charvaluestuff(EvaluationContext *evaluation, DbRef player,
                         int flag) {
  int found = 0;
  int ok;
  int type;
  int i;
  char buf[80] = {0};

  if (flag == -1)
    mecha_notify(evaluation, player, "List of charvalues available:");
  if (flag >= 0) {
    notify_printf(evaluation, player,
                  "List of %s available:", character_value_type_name(flag));
  }
  buf[0] = 0;
  for (i = 0; i < NUM_CHARVALUES; i++) {
    ok = 0;
    type = (unsigned char)character_value_definition(i)->type;
    if (flag < 0)
      ok = 1;
    else if (type == flag)
      ok = 1;
    if (ok) {
      char entry[25];
      (void)snprintf(entry, sizeof(entry), "%-23s ",
                     character_value_definition(i)->name);
      (void)string_append_bounded(buf, sizeof(buf), entry);
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
  char *tmpbuf;

  tmpbuf = alloc_sbuf("getvaluecodefind");
  lowercase_copy(tmpbuf, SBUF_SIZE, name);
  ip = hash_table_find(tmpbuf, character_value_hash(context, 0));
  if (!ip)
    ip = hash_table_find(tmpbuf, character_value_hash(context, 1));
  free_sbuf(tmpbuf);
  return (int)(intptr_t)ip - 1;
}

/********************/

/*   Roll the dice  */

/********************/

int char_rollsaving(BtechContext *context) {
  int r1;
  int r2;
  int r3;
  int r12;
  int r13;
  int r23;

  r1 = char_rolld6(context, 1);
  r2 = char_rolld6(context, 1);
  r3 = char_rolld6(context, 1);

  r12 = r1 + r2;
  r13 = r1 + r3;
  r23 = r2 + r3;

  if (r12 > r13) {
    if (r12 > r23)
      return r12;
    return r23;
  }
  if (r13 > r23)
    return r13;
  return r23;
}

int char_rollunskilled(BtechContext *context) {
  int r1;
  int r2;
  int r3;
  int r12;
  int r13;
  int r23;

  r1 = char_rolld6(context, 1);
  r2 = char_rolld6(context, 1);
  r3 = char_rolld6(context, 1);

  r12 = r1 + r2;
  r13 = r1 + r3;
  r23 = r2 + r3;

  if (r12 < r13) {
    if (r12 < r23)
      return r12;
    return r23;
  }
  if (r13 < r23)
    return r13;
  return r23;
}

int char_rollskilled(BtechContext *context) { return char_rolld6(context, 2); }

int char_rolld6(BtechContext *context, int num) {
  int i;
  int total = 0;

  for (i = 0; i < num; i++)
    total += btech_random_range_int(context, 1, 6);
  return (total);
}

/*****************************/

/*     DB access commands   */

/*****************************/

int char_getstatvalue(PSTATS *s, const char *name) {
  for (int i = 0; i < NUM_CHARVALUES; i++)
    if (!strcasecmp(character_value_definition(i)->name, name))
      return char_getstatvalue_by_code(s, i);
  return -1;
}

void char_setstatvalue(PSTATS *s, const char *name, int value) {
  for (int i = 0; i < NUM_CHARVALUES; i++)
    if (!strcasecmp(character_value_definition(i)->name, name)) {
      char_setstatvalue_by_code(s, i, value);
      return;
    }
}

int character_value_by_code(const CharacterValueRequest *request) {
  PSTATS stats;

  character_stats_retrieve(request->context, request->player, VALUES_ALL,
                           &stats);
  return char_getstatvalue_by_code((&stats), request->code);
}

void character_value_set_by_code(const CharacterValueChange *change) {
  PSTATS stats;

  character_stats_retrieve(change->target.context, change->target.player,
                           VALUES_ALL, &stats);
  char_setstatvalue_by_code((&stats), change->target.code, change->value);
  character_stats_store(change->target.context, change->target.player, &stats,
                        VALUES_ALL);
}

int char_getvalue(BtechContext *context, DbRef player, const char *name) {
  return character_value_by_code(
      &(CharacterValueRequest){.context = context,
                               .player = player,
                               .code = char_getvaluecode(context, name)});
}

void char_setvalue(BtechContext *context, DbRef player, const char *name,
                   int value) {
  character_value_set_by_code(&(CharacterValueChange){
      .target = {.context = context,
                 .player = player,
                 .code = char_getvaluecode(context, name)},
      .value = value});
}

static int
char_getskilltargetbycode_base(const CharacterSkillTargetCall *call) {
  BtechContext *context = call->context;
  DbRef player = call->player;
  PSTATS *s = call->stats;
  int code = call->code;
  int modifier = call->modifier;
  bool use_xp = call->use_experience;
  int val;
  int skill;

  if (code == -1)
    return 18;
  const CharacterValue *definition = character_value_definition(code);
  if (definition->type != CHAR_SKILL)
    return 18;
  if (use_xp && context->cached_target_character == player &&
      context->cached_skill == code)
    return context->cached_skill_result + modifier;
  if (definition->flag & CHAR_ATHLETIC)
    val = char_getstatvalue(s, "build") + char_getstatvalue(s, "reflexes");
  else if (definition->flag & CHAR_PHYSICAL)
    val = char_getstatvalue(s, "reflexes") + char_getstatvalue(s, "intuition");
  else if (definition->flag & CHAR_MENTAL)
    val = char_getstatvalue(s, "intuition") + char_getstatvalue(s, "learn");
  else if (definition->flag & CHAR_PHYSICAL)
    val = char_getstatvalue(s, "reflexes") + char_getstatvalue(s, "intuition");
  else if (definition->flag & CHAR_SOCIAL)
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
  }
  skill = character_stats_value_get(s, code);
  if (skill == -1)
    return (18);
  return 18 - val - skill;
}

int char_getskilltargetbycode(BtechContext *context, DbRef player, int code,
                              int modifier) {
  PSTATS stats;
  PSTATS *s = &stats;

  character_stats_retrieve(context, player, VALUES_CO, s);
  return char_getskilltargetbycode_base(
      &(CharacterSkillTargetCall){.context = context,
                                  .player = player,
                                  .stats = s,
                                  .code = code,
                                  .modifier = modifier,
                                  .use_experience = true});
}

static int char_getskilltargetbycode_noxp(BtechContext *context, DbRef player,
                                          int code, int modifier) {
  PSTATS stats;
  PSTATS *s = &stats;

  character_stats_retrieve(context, player, VALUES_CO, s);
  return char_getskilltargetbycode_base(
      &(CharacterSkillTargetCall){.context = context,
                                  .player = player,
                                  .stats = s,
                                  .code = code,
                                  .modifier = modifier});
}

int char_getskilltarget(BtechContext *context, DbRef player, const char *name,
                        int modifier) {
  return char_getskilltargetbycode(context, player,
                                   char_getvaluecode(context, name), modifier);
}

int char_getxpbycode(const CharacterValueRequest *request) {
  PSTATS stats;
  PSTATS *s = &stats;

  if (request->code < 0)
    return 0;
  character_stats_retrieve(request->context, request->player, VALUES_SKILLS, s);
  return character_stats_xp_get(s, request->code) % XP_MAX;
}

int char_gainxpbycode(const CharacterExperienceChange *change) {
  BtechContext *context = change->target.context;
  DbRef player = change->target.player;
  int code = change->target.code;
  PSTATS stats;
  PSTATS *s = &stats;

  if (code < 0)
    return 0;
  character_stats_retrieve(context, player, VALUES_SKILLS | VALUES_ATTRS, s);
  /* allow override of setting xp quickly. useful in chargen situations and only
   * settable via that Regular skill gains still check SK_XP and last used
   * within 30s to keep from spamming
   */
  if (!change->override_interval)
    if (!((context->clock->now >
           (character_stats_last_use_get(s, code) + 30)) ||
          (character_value_definition(code)->flag & SK_XP)))
      return 0;
  character_stats_last_use_set(&(CharacterStatsLastUseChange){
      .stats = s, .code = code, .value = context->clock->now});
  character_stats_xp_set(&(CharacterStatsExperienceChange){
      .stats = s,
      .code = code,
      .value = character_stats_xp_get(s, code) + change->amount});
  character_stats_xp_set(&(CharacterStatsExperienceChange){
      .stats = s,
      .code = code,
      .value = (character_stats_xp_get(s, code) % XP_MAX) +
               (XP_MAX * figure_xp_bonus(context, player, s, code))});
  character_stats_store(context, player, s, VALUES_SKILLS);
  return 1;
}

int char_gainxp(BtechContext *context, DbRef player, const char *skill,
                int amount) {
  return char_gainxpbycode(&(CharacterExperienceChange){
      .target = {.context = context,
                 .player = player,
                 .code = char_getvaluecode(context, skill)},
      .amount = amount});
}

int char_getskillsuccess(const CharacterSkillCheck *check) {
  BtechContext *context = check->context;
  DbRef player = check->player;
  const char *name = check->name;
  int roll;
  int val;
  int code;

  code = char_getvaluecode(context, name);

  val = char_getskilltargetbycode(context, player, code, check->modifier);

  if (character_value_by_code(&(CharacterValueRequest){
          .context = context, .player = player, .code = code}) == 0)
    roll = char_rollunskilled(context);
  else
    roll = char_rollskilled(context);
  if (check->loud) {
    notify_printf(btech_context_evaluation(context), player,
                  "You make a %s skill roll!", name);
    notify_printf(btech_context_evaluation(context), player,
                  "Modified skill BTH : %d Roll : %d", val, roll);
  }

  if (roll >= val)
    return (1); /* Success! */
  return (0);   /* Failure */
}

int char_getskillmargsucc(BtechContext *context, DbRef player, const char *name,
                          int modifier) {
  int roll;
  int val;
  int code;

  code = char_getvaluecode(context, name);

  val = char_getskilltargetbycode(context, player, code, modifier);

  if (character_value_by_code(&(CharacterValueRequest){
          .context = context, .player = player, .code = code}) == 0)
    roll = char_rollunskilled(context);
  else
    roll = char_rollskilled(context);

  return (roll - val);
}

DbRef char_getopposedskill(BtechContext *context, DbRef first,
                           const char *skill1, DbRef second,
                           const char *skill2) {
  int per1;
  int per2;

  per1 = char_getskillmargsucc(context, first, skill1, 0);
  per2 = char_getskillmargsucc(context, second, skill2, 0);

  if (per1 > per2)
    return (first);
  if (per2 == per1)
    return (0);
  return (second);
}

int char_getattrsave(BtechContext *context, DbRef player, const char *name) {
  int val = char_getvalue(context, player, name);

  if (val == -1)
    return (-1);
  if (val > 9)
    return 0;
  return (18 - (2 * val));
}

int char_getattrsavesucc(BtechContext *context, DbRef player,
                         const char *name) {
  int roll;
  int val = char_getattrsave(context, player, name);

  if (val == -1)
    return (-1);

  roll = char_rollskilled(context);

  if (roll >= val)
    return (1);
  return (0);
}

/************************/

/*    Database Commands */

/************************/

void init_btechstats(BtechContext *context) {
  char *tmpbuf;

  context->player_value_hashes = checked_storage_try_allocate_array(
      2, sizeof(*context->player_value_hashes));
  context->char_value_short_names = (char **)checked_storage_try_allocate_array(
      NUM_CHARVALUES, sizeof(*context->char_value_short_names));
  if (context->player_value_hashes == nullptr ||
      context->char_value_short_names == nullptr)
    exit(EXIT_FAILURE);
  context->char_value_count = NUM_CHARVALUES;
  hash_table_initialize(character_value_hash(context, 0), 20 * HASH_FACTOR);
  hash_table_initialize(character_value_hash(context, 1), 20 * HASH_FACTOR);
  tmpbuf = alloc_sbuf("getvaluecode");
  for (int i = 0; i < NUM_CHARVALUES; i++) {
    const char *name = character_value_definition(i)->name;
    lowercase_copy(tmpbuf, SBUF_SIZE, name);
    hash_table_add(tmpbuf, (int *)(intptr_t)(i + 1),
                   character_value_hash(context, 0));
    *(char *)checked_storage_at(tmpbuf, SBUF_SIZE, sizeof(char), 0) = '\0';
    const size_t NAME_LENGTH = strlen(name);
    for (size_t j = 0; j < NAME_LENGTH; j++) {
      const char *character =
          checked_storage_at_const(name, NAME_LENGTH + 1, sizeof(char), j);
      if (*character < 'A' || *character > 'Z')
        continue;
      char fragment[4];
      (void)snprintf(fragment, sizeof(fragment), "%.3s", character);
      (void)string_append_bounded(tmpbuf, SBUF_SIZE, fragment);
    }
    if (strlen(tmpbuf) <= 3) {
      (void)snprintf(tmpbuf, SBUF_SIZE, "%.5s", name);
    }
    *character_short_name_slot(context, i) = strdup(tmpbuf);
    lowercase_copy(tmpbuf, SBUF_SIZE, tmpbuf);
    hash_table_add(tmpbuf, (int *)(intptr_t)(i + 1),
                   character_value_hash(context, 1));
  }
  free_sbuf(tmpbuf);
}

void btech_stats_destroy(BtechContext *context) {
  if (context == nullptr)
    return;

  if (context->player_value_hashes != nullptr) {
    hash_table_destroy(character_value_hash(context, 0));
    hash_table_destroy(character_value_hash(context, 1));
    free(context->player_value_hashes);
    context->player_value_hashes = nullptr;
  }
  for (size_t i = 0; i < context->char_value_count; i++)
    free(*character_short_name_slot(context, (int)i));
  free((void *)context->char_value_short_names);
  context->char_value_short_names = nullptr;
  context->char_value_count = 0;
  context->cached_target_character = -1;
}

PSTATS *character_stats_create(void) {
  PSTATS *s;

  s = checked_storage_allocate(sizeof(*s));
  s->db_ref = -1;
  character_stats_clear(s);
  return s;
}
