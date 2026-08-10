#include "ai_api.h"
#include "ai_simulation_api.h"
#include "autopilot.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_coordinates.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
enum {
  SCORE_MOD = 100,
  SAFE_SCORE = SCORE_MOD * 1000,
  MIN_SAFE = 8,
  NORM_SAFE = 32,
  MAX_SIM_PATHS = 40,
  MNORM_COUNT = 37,
  CFAST_COUNT = 9,
  MAGIC_NUM = -123456,
};
typedef enum AiSpeedOption {
  AI_SPEED_NORMAL,
  AI_SPEED_FASTER,
  AI_SPEED_SLOWER,
} AiSpeedOption;
typedef struct AiPathOption {
  int heading_delta;
  AiSpeedOption speed;
} AiPathOption;
static int ai_speed_option_delta(AiSpeedOption option) {
  switch (option) {
  case AI_SPEED_NORMAL:
    return 0;
  case AI_SPEED_FASTER:
    return 1;
  case AI_SPEED_SLOWER:
    return -1;
  }
  return 0;
}
static const AiPathOption *ai_path_option_at(const AiPathOption *options,
                                             size_t count, int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(options, count, sizeof(*options),
                                  (size_t)index);
}
static const AiPathOption move_norm_opt[MNORM_COUNT] = {
    {0, AI_SPEED_FASTER},    {0, AI_SPEED_NORMAL},    {0, AI_SPEED_SLOWER},
    {-1, AI_SPEED_NORMAL},   {1, AI_SPEED_NORMAL},    {-2, AI_SPEED_NORMAL},
    {2, AI_SPEED_NORMAL},    {-3, AI_SPEED_NORMAL},   {3, AI_SPEED_NORMAL},
    {-5, AI_SPEED_NORMAL},   {5, AI_SPEED_NORMAL},    {-10, AI_SPEED_NORMAL},
    {10, AI_SPEED_NORMAL},   {-15, AI_SPEED_NORMAL},  {15, AI_SPEED_NORMAL},
    {-20, AI_SPEED_NORMAL},  {20, AI_SPEED_NORMAL},   {-30, AI_SPEED_NORMAL},
    {30, AI_SPEED_NORMAL},   {-40, AI_SPEED_NORMAL},  {40, AI_SPEED_NORMAL},
    {-60, AI_SPEED_NORMAL},  {60, AI_SPEED_NORMAL},   {-60, AI_SPEED_SLOWER},
    {60, AI_SPEED_SLOWER},   {-60, AI_SPEED_FASTER},  {60, AI_SPEED_FASTER},
    {-80, AI_SPEED_NORMAL},  {80, AI_SPEED_NORMAL},   {-120, AI_SPEED_NORMAL},
    {120, AI_SPEED_NORMAL},  {-160, AI_SPEED_NORMAL}, {160, AI_SPEED_NORMAL},
    {-160, AI_SPEED_SLOWER}, {160, AI_SPEED_SLOWER},  {-160, AI_SPEED_FASTER},
    {160, AI_SPEED_FASTER}};
/* Update: Just do subset if we're in silly mood */
static const AiPathOption combat_fast_opt[CFAST_COUNT] = {
    {0, AI_SPEED_FASTER},   {0, AI_SPEED_NORMAL},   {0, AI_SPEED_SLOWER},
    {-10, AI_SPEED_NORMAL}, {10, AI_SPEED_NORMAL},  {-30, AI_SPEED_NORMAL},
    {30, AI_SPEED_NORMAL},  {-60, AI_SPEED_NORMAL}, {60, AI_SPEED_NORMAL}};
typedef struct AiPathUnitSimulation {
  LocationSimulation location;
  Mech *mech;
  bool out;
} AiPathUnitSimulation;
typedef struct AiPathContext {
  AiPathUnitSimulation enemies[BATTLE_MAP_UNIT_CAPACITY];
  int enemy_count;
  AiPathUnitSimulation friends[BATTLE_MAP_UNIT_CAPACITY];
  int friend_count;
} AiPathContext;
typedef struct AiPathCandidate {
  LocationSimulation location;
  int danger;
  int tick_danger;
  int movement_score;
  int battle_score;
  int out_step;
  int stack_step;
  int best_range;
} AiPathCandidate;
typedef struct AiPathWorkspace {
  AiPathCandidate candidates[MAX_SIM_PATHS];
} AiPathWorkspace;
static AiPathUnitSimulation *ai_path_unit_at(AiPathUnitSimulation *units,
                                             int index) {
  if (index < 0)
    abort();
  return checked_storage_at(units, BATTLE_MAP_UNIT_CAPACITY, sizeof(*units),
                            (size_t)index);
}
static AiPathCandidate *ai_path_candidate_at(AiPathWorkspace *workspace,
                                             int index) {
  if (index < 0)
    abort();
  return checked_storage_at(workspace->candidates, MAX_SIM_PATHS,
                            sizeof(AiPathCandidate), (size_t)index);
}
typedef struct AiInfo {
  char text[MBUF_SIZE];
} AiInfo;
static void ai_score_min_update(int *current, int candidate) {
  if (*current > candidate || *current == MAGIC_NUM)
    *current = candidate;
}
static void ai_score_max_update(int *current, int candidate) {
  if (*current < candidate || *current == MAGIC_NUM)
    *current = candidate;
}
static int ai_score_normalize(int score, int minimum, int maximum, int scale) {
  return minimum == maximum ? 0
                            : scale * (score - minimum) / (maximum - minimum);
}
static void ai_score_range_relax(int *minimum, int *maximum, int divisor) {
  if (*minimum == MAGIC_NUM || *maximum == MAGIC_NUM)
    return;
  const int old_minimum = *minimum;
  const int old_maximum = *maximum;
  *minimum = (old_minimum * (divisor - 1) + old_maximum) / divisor;
  *maximum = (old_minimum + old_maximum * (divisor - 1)) / divisor;
}
static void ai_send_message(Autopilot *a, Mech *m, const char *msg) {
  auto_reply(m, msg);
  btech_channel_send(a->xcode.context, BTECH_CHANNEL_MECH_AI, "%s", msg);
}
static AiInfo ai_info(Mech *m, Autopilot *a) {
  AiInfo info;
  (void)snprintf(info.text, sizeof(info.text),
                 "Unit#%ld on #%ld [A#%ld]:", mech_dbref(m), mech_map_dbref(m),
                 a->mynum);
  return info;
}
static void ai_path_collect_enemies(AiPathContext *path, Mech *mech,
                                    BattleMap *map) {
  Mech *tempMech;
  int i;
  path->enemy_count = 0;
  for (i = 0; i < battle_map_unit_count(map); i++) {
    tempMech = btech_context_get_mech(mech_context(mech),
                                      battle_map_unit_dbref(map, i));
    if (!tempMech)
      continue;
    if (mech_is_destroyed(tempMech))
      continue;
    if (mech_condition_summary(tempMech).combat_safe)
      continue;
    if (mech_team(tempMech) == mech_team(mech))
      continue;
    if (mech_range_to(mech, tempMech) > 50.0F)
      continue; /* Inconsequential */
    if (path->enemy_count >= BATTLE_MAP_UNIT_CAPACITY)
      break;
    AiPathUnitSimulation *enemy =
        ai_path_unit_at(path->enemies, path->enemy_count++);
    location_simulation_initialize(&enemy->location, tempMech);
    enemy->mech = tempMech;
    enemy->out = false;
  }
}
static void ai_path_collect_friends(AiPathContext *path, Mech *mech,
                                    BattleMap *map) {
  Mech *tempMech;
  int i;
  path->friend_count = 0;
  for (i = 0; i < battle_map_unit_count(map); i++) {
    tempMech = btech_context_get_mech(mech_context(mech),
                                      battle_map_unit_dbref(map, i));
    if (!tempMech)
      continue;
    if (mech_is_destroyed(tempMech))
      continue;
    if (mech_team(tempMech) != mech_team(mech))
      continue;
    if (mech_class(tempMech) != CLASS_MECH)
      continue;
    if (mech_range_to(mech, tempMech) > 50.0F)
      continue; /* Inconsequential */
    if (path->friend_count >= BATTLE_MAP_UNIT_CAPACITY)
      break;
    AiPathUnitSimulation *friend =
        ai_path_unit_at(path->friends, path->friend_count++);
    location_simulation_initialize(&friend->location, tempMech);
    friend->mech = tempMech;
    friend->out = false;
  }
}
/* Simulate all candidate states independently for efficient path scoring. */
typedef struct AiPathScoreRequest {
  AiPathContext *path;
  Mech *mech;
  BattleMap *map;
  Autopilot *autopilot;
  const AiPathOption *options;
  int option_count;
  bool has_enemy;
  MapRealPosition target;
  MapRealPosition target_delta;
} AiPathScoreRequest;
typedef struct AiPathScoreResult {
  int selected_option;
  int score;
} AiPathScoreResult;
static AiPathScoreResult ai_path_score(const AiPathScoreRequest *request) {
  AiPathContext *path = request->path;
  Mech *m = request->mech;
  BattleMap *map = request->map;
  Autopilot *a = request->autopilot;
  const AiPathOption *options = request->options;
  const int option_count = request->option_count;
  const bool gotenemy = request->has_enemy;
  float dx = request->target.x;
  float dy = request->target.y;
  const float delx = request->target_delta.x;
  const float dely = request->target_delta.y;
  AiPathScoreResult result = {.selected_option = -1};
  int i, j, k, l, bearing;
  int sd, sc;
  AiPathWorkspace workspace = {0};
  for (i = 0; i < option_count; i++) {
    AiPathCandidate *candidate = ai_path_candidate_at(&workspace, i);
    const AiPathOption *option =
        ai_path_option_at(options, (size_t)option_count, i);
    candidate->best_range = 9999;
    location_simulation_initialize(&candidate->location, m);
    candidate->location.dh =
        AcceptableDegree(candidate->location.dh + option->heading_delta);
    sd = ai_speed_option_delta(option->speed);
    if (sd) {
      if (sd < 0) {
        candidate->location.ds = candidate->location.ds * 2.0F / 3.0F;
      } else if (sd == 1) {
        float ms = mech_effective_maximum_speed(m);
        candidate->location.ds =
            (candidate->location.ds < MP1 ? MP1 : candidate->location.ds) *
            4.0F / 3.0F;
        if (candidate->location.ds > ms)
          candidate->location.ds = ms;
      } else {
        float ms = mech_effective_maximum_speed(m);
        candidate->location.ds = ms;
      }
    }
  }
  for (i = 0; i < NORM_SAFE; i++) {
    dx += delx;
    dy += dely;
    for (k = 0; k < option_count; k++) {
      AiPathCandidate *candidate = ai_path_candidate_at(&workspace, k);
      if (candidate->out_step)
        continue;
      if (ai_crash(map, m, &candidate->location)) { /* Simulate _one_ step */
        candidate->out_step = i + 1;
        continue;
      }
      /* Base target-acquisition stuff */
      const float target_range = map_real_range(&(MapRealSegment){
          .start = {.x = candidate->location.fx, .y = candidate->location.fy},
          .end = {.x = dx, .y = dy},
      });
      l = (int)target_range;
      if (l < candidate->best_range)
        candidate->best_range = l;
      /* Generally speaking we're going to the point spesified */
      candidate->movement_score +=
          4 * (2 * (50 - candidate->best_range) + (100 - l));
      /* Heading change's inherently [slightly] evil */
      if (candidate->location.h != candidate->location.dh)
        candidate->movement_score -= 1;
      /* Moving is a good thing */
      if (candidate->location.x != candidate->location.lx ||
          candidate->location.y != candidate->location.ly) {
        if (candidate->location.t == BATTLE_TERRAIN_WATER)
          candidate->movement_score -= 5;
        candidate->movement_score += 10;
      }
      /* Punish for not utilizing full speed (this is .. hm, flaky) */
      const AiPathOption *option =
          ai_path_option_at(options, (size_t)option_count, k);
      if (option->speed != AI_SPEED_FASTER &&
          mech_effective_maximum_speed(m) > 0.1F) {
        const float speed_percent =
            100.0F * candidate->location.ds / mech_effective_maximum_speed(m);
        sc = BOUNDED(0, (int)speed_percent, 100);
        candidate->movement_score -=
            (100 - sc) / 30; /* Basically, unused speed is bad */
      }
    }
    if (mech_class(m) == CLASS_MECH) {
      /* Simulate friends */
      for (j = 0; j < path->friend_count; j++) {
        AiPathUnitSimulation *friend = ai_path_unit_at(path->friends, j);
        if (friend->out)
          continue;
        if (ai_crash(map, friend->mech, &friend->location))
          friend->out = true;
      }
      for (k = 0; k < option_count; k++) {
        int stack_count = 0;
        AiPathCandidate *candidate = ai_path_candidate_at(&workspace, k);
        if (candidate->out_step || candidate->stack_step)
          continue;
        /* Meaning of stack: Someone moves _into_ the hex */
        for (j = 0; j < path->friend_count; j++) {
          AiPathUnitSimulation *friend = ai_path_unit_at(path->friends, j);
          if (!friend->out)
            if (candidate->location.x == friend->location.x &&
                candidate->location.y == friend->location.y)
              stack_count++;
        }
        if (stack_count > 1) { /* Possible stackage */
          int osc = stack_count;
          for (j = 0; j < path->friend_count; j++) {
            AiPathUnitSimulation *friend = ai_path_unit_at(path->friends, j);
            if (!friend->out)
              if (candidate->location.x == friend->location.x &&
                  candidate->location.y == friend->location.y)
                if ((candidate->location.lx != candidate->location.x ||
                     candidate->location.ly != candidate->location.y) ||
                    (friend->location.lx != friend->location.x ||
                     friend->location.ly != friend->location.y))
                  osc--;
          }
          if (osc != stack_count)
            candidate->stack_step = i + 1;
        }
        if (gotenemy)
          candidate->tick_danger = 0;
      }
    }
    if (gotenemy) {
      /* Update enemy locations as well */
      for (j = 0; j < path->enemy_count; j++) {
        AiPathUnitSimulation *enemy = ai_path_unit_at(path->enemies, j);
        if (enemy->out)
          continue;
        if (ai_crash(map, enemy->mech, &enemy->location))
          enemy->out = true;
        for (k = 0; k < option_count; k++) {
          AiPathCandidate *candidate = ai_path_candidate_at(&workspace, k);
          if (candidate->out_step)
            continue;
          l = map_hex_distance(&(HexDistanceRequest){
              .start = {.x = candidate->location.x, .y = candidate->location.y},
              .end = {.x = enemy->location.x, .y = enemy->location.y},
              .correction = 0,
          });
          if (l >= 100)
            continue;
          switch (a->auto_cmode) {
          case 0: /* Withdraw */
            if (l > a->auto_cdist)
              candidate->battle_score += 5 * a->auto_cdist + l - a->auto_cdist;
            else
              candidate->battle_score += 5 * l;
            break;
          case 1: /* Score  = fulfilling goal (=> distance from cdist) */
            if (l < a->auto_cdist)
              candidate->battle_score -=
                  10 * (a->auto_cdist - l); /* Not too close */
            else
              candidate->battle_score -= 2 * (l - a->auto_cdist);
            break;
          case 2:
            if (l < a->auto_cdist)
              candidate->battle_score -= 2 * (a->auto_cdist - l);
            else
              candidate->battle_score -= 10 * (l - a->auto_cdist);
          }
          if (l > 28)
            continue;
          /* Danger modifier ; it's _always_ dangerous to be close */
          candidate->tick_danger += (40 - MIN(40, l));
          /* Arcs can be .. dangerous */
          if (mech_class(m) == CLASS_MECH) {
            bearing = map_bearing(&(MapRealSegment){
                .start = {.x = candidate->location.fx,
                          .y = candidate->location.fy},
                .end = {.x = enemy->location.fx, .y = enemy->location.fy}});
            bearing = candidate->location.h - bearing;
            if (bearing < 0)
              bearing += 360;
            if (bearing >= 90 && bearing <= 270) {
              /* Sides are moderately dangerous [potential rear arcs] */
              candidate->tick_danger += 5 * (29 - MIN(29, l));
              if (bearing >= 120 && bearing <= 240) {
                /* Rear arc is VERY dangerous */
                candidate->tick_danger += 20 * (29 - MIN(29, l));
              }
            }
          } else if (mech_class(m) == CLASS_VEH_GROUND) {
            bearing = map_bearing(&(MapRealSegment){
                .start = {.x = candidate->location.fx,
                          .y = candidate->location.fy},
                .end = {.x = enemy->location.fx, .y = enemy->location.fy}});
            bearing = candidate->location.h - bearing;
            if (bearing < 0)
              bearing += 360;
            if (bearing >= 45 && bearing <= 315) {
              if (bearing >= 135 && bearing <= 225) {
                /* Rear arc is VERY dangerous */
                candidate->tick_danger +=
                    10 * (29 - MIN(29, l)) *
                    (100 - 100 * mech_section_armor(m, BSIDE) /
                               MAX(1, mech_section_original_armor(m, BSIDE))) /
                    100;
              } else if (bearing < 135) {
                /* right side */
                candidate->tick_danger +=
                    7 * (29 - MIN(29, l)) *
                    (100 - 100 * mech_section_armor(m, RSIDE) /
                               MAX(1, mech_section_original_armor(m, RSIDE))) /
                    100;
              } else {
                candidate->tick_danger +=
                    7 * (29 - MIN(29, l)) *
                    (100 - 100 * mech_section_armor(m, LSIDE) /
                               MAX(1, mech_section_original_armor(m, LSIDE))) /
                    100;
              }
            } else
              candidate->tick_danger +=
                  5 * (29 - MIN(29, l)) *
                  (100 - 100 * mech_section_armor(m, FSIDE) /
                             MAX(1, mech_section_original_armor(m, FSIDE))) /
                  100;
          }
        }
        for (k = 0; k < option_count; k++) {
          AiPathCandidate *candidate = ai_path_candidate_at(&workspace, k);
          if (candidate->out_step)
            continue;
          /* Dangerous to be far from buddy in fight */
          const float target_range = map_real_range(&(MapRealSegment){
              .start = {.x = candidate->location.fx,
                        .y = candidate->location.fy},
              .end = {.x = dx, .y = dy},
          });
          l = (int)target_range;
          if ((delx != 0.0F || dely != 0.0F))
            candidate->tick_danger += MIN(100, l * l);
          if (path->enemy_count)
            candidate->tick_danger /= path->enemy_count;
          /* It's inherently dangerous to move slowly: */
          if (candidate->location.s <= MP2)
            candidate->tick_danger += 400;
          else if (candidate->location.s <= MP4)
            candidate->tick_danger += 300;
          else if (candidate->location.s <= MP6)
            candidate->tick_danger += 200;
          else if (candidate->location.s <= MP9)
            candidate->tick_danger += 100;
          candidate->danger +=
              candidate->tick_danger * (NORM_SAFE - i) / (NORM_SAFE / 2);
        }
      }
    }
  }
  for (i = 0; i < option_count; i++) {
    AiPathCandidate *candidate = ai_path_candidate_at(&workspace, i);
    ai_score_min_update(&a->w_msc, candidate->movement_score);
    ai_score_max_update(&a->b_msc, candidate->movement_score);
    if (gotenemy) {
      ai_score_min_update(&a->w_bsc, candidate->battle_score);
      ai_score_max_update(&a->b_bsc, candidate->battle_score);
      ai_score_min_update(&a->w_dan, candidate->danger);
      ai_score_max_update(&a->b_dan, candidate->danger);
    }
  }
  /* Now we have been.. calibrated */
  /* Find best overall score */
  for (i = 0; i < option_count; i++) {
    AiPathCandidate *candidate = ai_path_candidate_at(&workspace, i);
    const AiPathOption *option =
        ai_path_option_at(options, (size_t)option_count, i);
    if (!candidate->out_step)
      candidate->out_step = NORM_SAFE + 1;
    if (!candidate->stack_step)
      candidate->stack_step = candidate->out_step;
    sc = (candidate->out_step -
          (candidate->out_step - candidate->stack_step) / 2) *
             SAFE_SCORE +
         ai_score_normalize(candidate->movement_score, a->w_msc, a->b_msc,
                            SCORE_MOD * a->auto_goweight);
    if (gotenemy)
      sc +=
          ai_score_normalize(candidate->battle_score, a->w_bsc, a->b_bsc,
                             SCORE_MOD * a->auto_fweight) -
          ai_score_normalize(candidate->danger, a->w_dan, a->b_dan,
                             SCORE_MOD * (a->auto_fweight + a->auto_goweight));
    const AiPathOption *best_option =
        result.selected_option >= 0
            ? ai_path_option_at(options, (size_t)option_count,
                                result.selected_option)
            : nullptr;
    if (sc > result.score || (best_option != nullptr && sc == result.score &&
                              ai_speed_option_delta(option->speed) >
                                  ai_speed_option_delta(best_option->speed))) {
      result.score = sc;
      result.selected_option = i;
    }
  }
  return result;
}
static int ai_opponents(Autopilot *a, Mech *m) {
  if (a->auto_nervous) {
    a->auto_nervous--;
    return 1;
  }
  if (mech_seen_count(m))
    a->auto_nervous = 30; /* We'll stay frisky for awhile even if cons are lost
                             for one reason or another */
  return mech_seen_count(m);
}
static void ai_stop(Mech *mech, Autopilot *a) {
  char buf[128] = {0};
  if (mech_desired_speed(mech) > 0.1F) {
    strncpy(buf, "stop", 128);
    mech_speed(a->mynum, mech, buf);
  }
}
void ai_set_speed(Mech *mech, Autopilot *a, float spd) {
  char buf[SBUF_SIZE] = {0};
  float newspeed;
  if (!mech || !a)
    return;
  newspeed =
      FBOUNDED(0.0F, spd,
               (mech_effective_maximum_speed(mech) * (float)a->speed) / 100.0F);
  if (fabsf(mech_desired_speed(mech) - newspeed) > 0.0001F) {
    (void)snprintf(buf, SBUF_SIZE, "%f", (double)newspeed);
    mech_speed(a->mynum, mech, buf);
  }
}
void ai_set_heading(Mech *mech, Autopilot *a, int dir) {
  char buf[128] = {0};
  if (dir == mech_desired_heading_degrees(mech))
    return;
  (void)snprintf(buf, 128, "%d", dir);
  mech_heading(a->mynum, mech, buf);
}
typedef struct AiMovementAdjustment {
  Autopilot *autopilot;
  Mech *mech;
  const char *description;
  int heading_delta;
  AiSpeedOption speed;
  int score;
} AiMovementAdjustment;
static void ai_adjust_move(const AiMovementAdjustment *adjustment) {
  Autopilot *a = adjustment->autopilot;
  Mech *m = adjustment->mech;
  const char *text = adjustment->description;
  const int hmod = adjustment->heading_delta;
  const AiSpeedOption speed_option = adjustment->speed;
  const int b_score = adjustment->score;
  ai_set_heading(m, a, mech_desired_heading_degrees(m) + hmod);
  switch (speed_option) {
  case AI_SPEED_NORMAL:
    btech_channel_send(a->xcode.context, BTECH_CHANNEL_MECH_AI,
                       "%s state: %s (hmod:%d) sc:%d", ai_info(m, a).text, text,
                       hmod, b_score);
    break;
  case AI_SPEED_FASTER:
    btech_channel_send(a->xcode.context, BTECH_CHANNEL_MECH_AI,
                       "%s state: %s+accelerating (hmod:%d) sc:%d",
                       ai_info(m, a).text, text, hmod, b_score);
    ai_set_speed(m, a,
                 (mech_desired_speed(m) < MP1 ? MP1 : mech_desired_speed(m)) *
                     4.0F / 3.0F);
    break;
  case AI_SPEED_SLOWER:
    btech_channel_send(a->xcode.context, BTECH_CHANNEL_MECH_AI,
                       "%s state: %s+decelerating (hmod:%d) sc:%d",
                       ai_info(m, a).text, text, hmod, b_score);
    ai_set_speed(m, a, mech_desired_speed(m) * 2.0F / 3.0F);
    break;
  }
}
typedef struct AiPathAdjustment {
  Autopilot *autopilot;
  Mech *mech;
  const char *description;
  const AiPathOption *options;
  size_t option_count;
  int selected;
  int score;
} AiPathAdjustment;
static void ai_adjust_path_option(const AiPathAdjustment *adjustment) {
  const AiPathOption *option = ai_path_option_at(
      adjustment->options, adjustment->option_count, adjustment->selected);
  ai_adjust_move(&(AiMovementAdjustment){.autopilot = adjustment->autopilot,
                                         .mech = adjustment->mech,
                                         .description = adjustment->description,
                                         .heading_delta = option->heading_delta,
                                         .speed = option->speed,
                                         .score = adjustment->score});
}
int ai_check_path(Mech *m, Autopilot *a, float dx, float dy, float delx,
                  float dely) {
  int o;
  int b_len, b, b_score;
  AiPathContext path = {0};
  BtechContext *context = mech_context(m);
  BattleMap *map = btech_context_get_map(context, mech_map_dbref(m));
  const time_t now = btech_context_now(context);
  const int event_tick = btech_context_event_tick(context);
  o = ai_opponents(a, m);
  if (a->last_upd > now || (now - a->last_upd) > AUTO_GOET) {
    if ((event_tick - a->last_upd) > AUTO_GOTT) {
      a->b_msc = MAGIC_NUM;
      a->w_msc = MAGIC_NUM;
      a->b_bsc = MAGIC_NUM;
      a->w_bsc = MAGIC_NUM;
      a->b_dan = MAGIC_NUM;
      a->b_dan = (40 + 20 * 29 + 100) * 30; /* To stay focused */
    } else {
      /* Slight update ; Un-refine the goals somewhat */
      ai_score_range_relax(&a->w_msc, &a->b_msc, 3);
      ai_score_range_relax(&a->w_bsc, &a->b_bsc, 5);
      ai_score_range_relax(&a->w_dan, &a->b_dan, 8);
      a->b_dan = MAX(a->b_dan, (40 + 20 * 29 + 100) * 30); /* To stay focused */
    }
    a->last_upd = now;
  }
  /* Got either opponents (nasty) or [possibly] blocked path (slightly nasty),
   * i.e. 12sec */
  if (mech_class(m) == CLASS_MECH)
    ai_path_collect_friends(&path, m, map);
  if (o) {
    ai_path_collect_enemies(&path, m, map);
    if (!((event_tick / AUTOPILOT_GOTO_TICK) %
          4)) { /* Just every fourth tick, i.e. 12sec */
      /* Thorough check */
      AiPathScoreResult result = ai_path_score(
          &(AiPathScoreRequest){.path = &path,
                                .mech = m,
                                .map = map,
                                .autopilot = a,
                                .options = move_norm_opt,
                                .option_count = MNORM_COUNT,
                                .has_enemy = true,
                                .target = {.x = dx, .y = dy},
                                .target_delta = {.x = delx, .y = dely}});
      b = result.selected_option;
      b_score = result.score;
      b_len = b_score / SAFE_SCORE;
      if (b_len >= MIN_SAFE)
        ai_adjust_path_option(
            &(AiPathAdjustment){.autopilot = a,
                                .mech = m,
                                .description = "combat(/twitchy)",
                                .options = move_norm_opt,
                                .option_count = MNORM_COUNT,
                                .selected = b,
                                .score = b_score});
    } else {
      AiPathScoreResult result = ai_path_score(
          &(AiPathScoreRequest){.path = &path,
                                .mech = m,
                                .map = map,
                                .autopilot = a,
                                .options = combat_fast_opt,
                                .option_count = CFAST_COUNT,
                                .has_enemy = true,
                                .target = {.x = dx, .y = dy},
                                .target_delta = {.x = delx, .y = dely}});
      b = result.selected_option;
      b_score = result.score;
      b_len = b_score / SAFE_SCORE;
      if (b_len >= MIN_SAFE)
        ai_adjust_path_option(
            &(AiPathAdjustment){.autopilot = a,
                                .mech = m,
                                .description = "[f]combat(/twitchy)",
                                .options = combat_fast_opt,
                                .option_count = CFAST_COUNT,
                                .selected = b,
                                .score = b_score});
    }
    return 1; /* We want to keep fighting near foes */
  }
  if (!((event_tick / AUTOPILOT_GOTO_TICK) %
        4)) { /* Just every fourth tick, i.e. 12sec */
    /* Thorough check */
    AiPathScoreResult result = ai_path_score(
        &(AiPathScoreRequest){.path = &path,
                              .mech = m,
                              .map = map,
                              .autopilot = a,
                              .options = move_norm_opt,
                              .option_count = MNORM_COUNT,
                              .target = {.x = dx, .y = dy},
                              .target_delta = {.x = delx, .y = dely}});
    b = result.selected_option;
    b_score = result.score;
    b_len = b_score / SAFE_SCORE;
    if (b_len >= MIN_SAFE)
      ai_adjust_path_option(&(AiPathAdjustment){.autopilot = a,
                                                .mech = m,
                                                .description = "moving",
                                                .options = move_norm_opt,
                                                .option_count = MNORM_COUNT,
                                                .selected = b,
                                                .score = b_score});
  } else {
    AiPathScoreResult result = ai_path_score(
        &(AiPathScoreRequest){.path = &path,
                              .mech = m,
                              .map = map,
                              .autopilot = a,
                              .options = combat_fast_opt,
                              .option_count = CFAST_COUNT,
                              .target = {.x = dx, .y = dy},
                              .target_delta = {.x = delx, .y = dely}});
    b = result.selected_option;
    b_score = result.score;
    b_len = b_score / SAFE_SCORE;
    if (b_len >= MIN_SAFE)
      ai_adjust_path_option(&(AiPathAdjustment){.autopilot = a,
                                                .mech = m,
                                                .description = "[f]moving",
                                                .options = combat_fast_opt,
                                                .option_count = CFAST_COUNT,
                                                .selected = b,
                                                .score = b_score});
  }
  if (b_len >= MIN_SAFE)
    return 1;
  /* Slow down + stop - no sense in dying needlessly */
  ai_stop(m, a);
  btech_channel_send(a->xcode.context, BTECH_CHANNEL_MECH_AI, "%s state: panic",
                     ai_info(m, a).text);
  ai_send_message(a, m, "PANIC! Unable to comply with order.");
  return 0;
}
