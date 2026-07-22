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
#include <time.h>

#include "mech.h"
#include "mux/commands/command_invocation.h"
#include "mux/objects/db.h"
#include "mux/server/server_api.h"
#include "p.glue.h"

#include "macros.h"
void init_stat(BtechContext *context) {
  btech_random_seed(&context->random, (unsigned long)time(nullptr));
}

static const int chances[11] = {1, 2, 3, 4, 5, 6, 5, 4, 3, 2, 1};

void do_show_stat(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const BtechRollStatistics *statistics =
      &invocation->context->btech->random.statistics;
  DbRef player = invocation->player;
  int i, j, chancetotal;
  float f1, f2, chanceperc, optimalrolls;

  if (!statistics->total_rolls) {
    notify(evaluation, player, "No rolls to show statistics for!");
    return;
  }
  for (i = 0; i < 11; i++) {
    if (i == 0) {
      notify(evaluation, player,
             "#    Rolls %Current  Optimal Rolls %Optimal  %Hit Chance "
             " %Miss Chance");
    }
    f1 = (float)chances[i] * 100.0 / 36.0;
    f2 = (float)statistics->rolls[i] * 100.0 / statistics->total_rolls;
    chancetotal = 0;
    for (j = i; j < 11; j++) {
      chancetotal = chancetotal + chances[j];
    }
    chanceperc = (float)chancetotal / 36.0 * 100;
    optimalrolls = f1 / 100 * statistics->total_rolls;
    notify_printf(evaluation, player, "%-3d %6d %8.3f %14d %8.3f %12.3f %13.3f",
                  i + 2, statistics->rolls[i], f2, (int)optimalrolls, f1,
                  chanceperc, 100.0 - chanceperc);
  }
  notify_printf(evaluation, player, "Total rolls: %d", statistics->total_rolls);
}

/*
 * Returns an integer chosen randomly from the interval [low,high].
 *
 * To eliminate bias from rounding error, this routine repeatedly takes some
 * number of high order bits from the Mersenne Twister, until it finds a value
 * <= (high - low).  If we take n bits, such that 2^n is the smallest power of
 * two greater than (high - low), then this procedure should only require
 * another iteration 50% or less of the time. (The actual value would be
 * (2^n - (high - low)) / (high - low).) It also always terminates due to the
 * statistical qualities of the Mersenne Twister, although possibly only after
 * several (but generally very few) iterations.
 *
 * For example, computing a D6 should require a second iteration 33% (1/3rd) of
 * the time, a third iteration 11% (1/9th) of the time, a fourth iteration 3.7%
 * (1/27th) of the time, a fifth iteration 1.2% of the time (1/81st) of the
 * time, a sixth iteration 0.4% (1/243rd) of the time, and so on.  Or in other
 * words, this will require fewer than six iterations 99.6% of the time, while
 * completely eliminating rounding bias.
 *
 * This code is on the critical path, but modern processors can compute this
 * stuff really fast.  There's really no need to have the compiler inline it to
 * perform further optimization.
 */
long btech_random_range(BtechContext *context, long low, long high) {
  const unsigned long int range = (unsigned long int)(high - low);

  unsigned long value;
  unsigned int nn;

  assert(high >= low);

  /*
   * Compute n, the shift value.  We're using the 32-bit version of the
   * Mersenne Twister, so we only need shifts up to 32. (If we did need a
   * larger value, we would also need to expand our random number size.)
   *
   * We can special case some of the common values (such as n = 8 for
   * range = 5, for the D6) if this loop becomes a concern.
   */
  for (nn = 0; nn < 32; nn++) {
    if ((range >> nn) == 0)
      break;
  }

  nn = 32 - nn;

  /* Shifts >= bit width are undefined in C.  At least on x86, they
   * apparently do nothing, which causes the following do-while loop to
   * run until the generator returns 0.  */
  if (nn == 32) {
    return 0;
  }

  assert(nn < 32);

  /* Repeatedly select random numbers until we get an acceptable one.  */
  do {
    value = btech_random_u32(&context->random) >> nn;
  } while (value > range);

  return low + value;
}
