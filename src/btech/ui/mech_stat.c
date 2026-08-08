/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Tue Aug 12 19:06:48 1997 fingon
 * Last modified: Tue Aug 12 20:04:59 1997 fingon
 */

/* Make statistics 'bout what we do.. whatever it is we _do_ */

#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "mech_stat_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/commands/command_handlers.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "random.h"
#include "registry_api.h"

void init_stat(BtechContext *context) {
  if (!btech_random_seed_from_system(&context->random)) {
    perror("getrandom");
    exit(EXIT_FAILURE);
  }
}

static const int chances[11] = {1, 2, 3, 4, 5, 6, 5, 4, 3, 2, 1};

static int roll_chance(int index) {
  return *(const int *)checked_storage_at_const(chances, 11, sizeof(*chances),
                                                (size_t)index);
}

static int roll_count(const BtechRollStatistics *statistics, int index) {
  return *(const int *)checked_storage_at_const(
      statistics->rolls, 11, sizeof(*statistics->rolls), (size_t)index);
}

void do_show_stat(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const BtechRollStatistics *statistics =
      &invocation->context->btech->random.statistics;
  DbRef player = invocation->player;
  int i, j, chancetotal;
  float f1, f2, chanceperc, optimalrolls;

  if (!statistics->total_rolls) {
    mecha_notify(evaluation, player, "No rolls to show statistics for!");
    return;
  }
  for (i = 0; i < 11; i++) {
    if (i == 0) {
      mecha_notify(evaluation, player,
                   "#    Rolls %Current  Optimal Rolls %Optimal  %Hit Chance "
                   " %Miss Chance");
    }
    const int chance = roll_chance(i);
    const int count = roll_count(statistics, i);
    f1 = (float)chance * 100.0F / 36.0F;
    f2 = (float)count * 100.0F / (float)statistics->total_rolls;
    chancetotal = 0;
    for (j = i; j < 11; j++) {
      chancetotal = chancetotal + roll_chance(j);
    }
    chanceperc = (float)chancetotal / 36.0F * 100.0F;
    optimalrolls = f1 / 100.0F * (float)statistics->total_rolls;
    notify_printf(evaluation, player, "%-3d %6d %8.3f %14d %8.3f %12.3f %13.3f",
                  i + 2, count, (double)f2, (int)optimalrolls, (double)f1,
                  (double)chanceperc, (double)(100.0F - chanceperc));
  }
  notify_printf(evaluation, player, "Total rolls: %d", statistics->total_rolls);
}

/*
 * Returns an integer chosen randomly from the interval [low,high].
 *
 * To eliminate modulo bias, this routine repeatedly draws from xoshiro256**
 * until it finds a value in the largest multiple of the interval width that
 * fits in a uint64_t. This requires at most one additional draw on average.
 *
 * This code is on the critical path, but modern processors can compute this
 * stuff really fast.  There's really no need to have the compiler inline it to
 * perform further optimization.
 */
long btech_random_range(BtechContext *context, long low, long high) {
  uint64_t width;
  uint64_t limit;
  uint64_t value;

  assert(context != nullptr);
  assert(high >= low);

  width = (uint64_t)high - (uint64_t)low + UINT64_C(1);
  if (width == 0) {
    return (long)btech_random_u64(&context->random);
  }

  limit = UINT64_MAX - UINT64_MAX % width;
  do {
    value = btech_random_u64(&context->random);
  } while (value >= limit);

  return (long)((uint64_t)low + value % width);
}

int btech_random_range_int(BtechContext *context, int low, int high) {
  return (int)btech_random_range(context, low, high);
}
