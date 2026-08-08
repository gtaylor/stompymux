#include "autopilot.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "map.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_electronics_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_radio_api.h"
#include "mech_radio_render_internal.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/support/formatting.h"
#include "random.h"
#include "registry_api.h"

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void sendchannelstuff(Mech *mech, int freq, char *msg);

static void append_lbuf(char *buffer, size_t size, const char *fmt, ...)
    __attribute__((format(printf, 3, 4)));

static void append_lbuf(char *buffer, size_t size, const char *fmt, ...) {
  size_t len = strlen(buffer);
  va_list ap;

  if (len >= size)
    return;

  va_start(ap, fmt);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  vsnprintf(buffer + len, size - len, fmt, ap);
  va_end(ap);
}

static void do_scramble(BtechContext *context, char *buffo, int ch, int bth) {
  int i;

  for (i = 0; buffo[i]; i++) {
    if (btech_random_range(context, 1, 100) > ch &&
        btech_random_roll(context) < (bth + 5)) {
      if (btech_random_range(context, 1, 2) == 1)
        buffo[i] -= btech_random_range(context, 1, 10);
      else
        buffo[i] += btech_random_range(context, 1, 10);
    }
    buffo[i] = (unsigned char)BOUNDED(33, buffo[i], 255);
  }
}

static int relay_signal_improve(int signal, int factor) {
  return 100 - (100 - signal) / factor;
}

typedef struct CommRelayContext CommRelayContext;
struct CommRelayContext {
  bool connected[MAX_MECHS_PER_MAP][MAX_MECHS_PER_MAP];
  bool visited[MAX_MECHS_PER_MAP];
  Mech *mechs[MAX_MECHS_PER_MAP];
  int best_path[MAX_MECHS_PER_MAP];
  int path[MAX_MECHS_PER_MAP];
  int node_count;
  int best_depth;
  int recursion_iterations;
};

static void scramble_message(const CommRelayContext *relay,
                             BtechContext *context, char *buffo, int range,
                             int sendrange, int recvrrange, char *handle,
                             char *msg, int bth, int *isxp, int under_ecm,
                             int digmode) {

  int mr, i;
  char *header = nullptr;
  char buf[LBUF_SIZE];

  *isxp = 0;

  if (digmode > 1 && relay != nullptr && relay->best_depth > 1) {
    int bearing;

    strncpy(buf, "{R-path:", LBUF_SIZE);
    for (i = 1; i < relay->best_depth; i++) {
      if (i > 1)
        strcat(buf, "/");
      bearing = FindBearing(
          mech_position_real_x(relay->mechs[relay->best_path[i]]),
          mech_position_real_y(relay->mechs[relay->best_path[i]]),
          mech_position_real_x(relay->mechs[relay->best_path[i - 1]]),
          mech_position_real_y(relay->mechs[relay->best_path[i - 1]]));
      MechUnitId id = mech_unit_id(relay->mechs[relay->best_path[i]]);
      snprintf(buf + strlen(buf), sizeof(buf) - strlen(buf), "[%c%c]-h:%.3d",
               id.first, id.second, bearing);
    }
    strcat(buf, "} ");
    header = buf;
  }

  buffo[0] = '\0';
  append_lbuf(buffo, LBUF_SIZE, "%s", header ? header : "");
  if (handle && *handle)
    append_lbuf(buffo, LBUF_SIZE, "<%s> ", handle);
  append_lbuf(buffo, LBUF_SIZE, "%s", msg);

  if ((!digmode && (range >= sendrange || range >= recvrrange)) || under_ecm) {
    if (!digmode) {

      mr = MAX(recvrrange, (sendrange + recvrrange) / 2);
      if (sendrange < range) {
        do_scramble(context, buffo, (100 * sendrange) / MAX(1, range), bth);
        *isxp = 1;
      }

      if (mr < range) {
        do_scramble(context, buffo,
                    relay_signal_improve((100 * mr) / MAX(1, range), 2), bth);
        *isxp = 1;
      }
    }

    if (under_ecm && range >= 1) {
      do_scramble(context, buffo, btech_random_range(context, 30, 50), bth);
      *isxp = 1;
    }
  }
}

static void recursive_commlink(CommRelayContext *relay, int i, int dep) {
  int j;

  if (relay->recursion_iterations++ >= 10000)
    return;
  if (dep >= relay->best_depth)
    return;
  relay->path[dep] = i;
  for (j = 1; j < relay->node_count; j++)
    if (relay->connected[i][j] && !relay->visited[j]) {
      if (j == (relay->node_count - 1)) {
        int k;

        relay->best_depth = dep;
        for (k = 0; k < relay->best_depth; k++)
          relay->best_path[k] = relay->path[k];
      } else {
        relay->visited[j] = true;
        recursive_commlink(relay, j, dep + 1);
        relay->visited[j] = false;
      }
    }
}

static void nonrecursive_commlink(CommRelayContext *relay, int i) {
  int dep = 0, j;
  int comm_loop[MAX_MECHS_PER_MAP];
  int iter_c = 0;
  int maxdepth = 0;

  /* May _still_ contain fatal bug ; Ghod knows (I don't) */
  comm_loop[0] = 1;
  relay->path[0] = i;

  while (dep >= 0) {
    i = relay->path[dep];
    for (j = comm_loop[dep]; j < relay->node_count; j++)
      if (relay->connected[i][j] && !relay->visited[j]) {
        if (j == (relay->node_count - 1)) {
          int k;

          relay->best_depth = dep + 1;
          for (k = 0; k < relay->best_depth; k++)
            relay->best_path[k] = relay->path[k];
          j = relay->node_count;
          break;
        } else if ((dep + 1) < relay->best_depth) {
          relay->visited[j] = true;
          comm_loop[dep++] = j + 1;
          comm_loop[dep] = 1;
          relay->path[dep] = j;
          if (dep > maxdepth)
            maxdepth = dep;
          break;
        }
      }
    if (j == relay->node_count) {
      if (dep > 0)
        relay->visited[comm_loop[--dep] - 1] = false;
      else
        dep--; /* We're finished! */
    }
    if (iter_c++ == 100000) {
      /* Lets not spam MechErrors with this.. */
      /*
                              btech_channel_send(mech->xcode.context,
         BTECH_CHANNEL_MECH_ERRORS, tprintf
                                                ("#%d: Infinite loop in relay
         code (?) ; using backup recursive code (num_mechs:%d, maxdepth:%d,
         nowdepth:%d)", relay->mechs[0]->mynum, relay->node_count, maxdepth,
         dep));
      */
      relay->best_depth = 9999;
      for (i = 0; i < relay->node_count; i++)
        relay->visited[i] = false;
      relay->recursion_iterations = 0;
      recursive_commlink(relay, 0, 0);
      return;
    }
  }
}

static bool find_comm_link(CommRelayContext *relay, BattleMap *map, Mech *from,
                           Mech *to, int freq) {
  int i, j;
  Mech *t;

  relay->node_count = 0;
  relay->mechs[relay->node_count++] = from;
  for (i = 0; i < map->first_free; i++) {
    if (!(t = btech_context_find_object(mech_context(from),
                                        map->mechsOnMap[i])))
      continue;
    if (t == from || t == to)
      continue;
    if (mech_team(from) != mech_team(t))
      continue;
    if ((mech_movement_type(t) != MOVE_NONE && !mech_is_started(t)) ||
        (mech_movement_type(t) == MOVE_NONE && mech_is_destroyed(t)))
      continue;
    if (!(mech_radio_capabilities(t) & RADIO_RELAY))
      continue;
    for (j = 0; j < mech_radio_channel_count(t); j++)
      if (mech_radio_frequency(t, j) == freq)
        if (mech_radio_mode(t, j) & FREQ_RELAY) {
          if (relay->node_count < MAX_MECHS_PER_MAP - 1)
            relay->mechs[relay->node_count++] = t;
          continue;
        }
  }
  relay->mechs[relay->node_count++] = to;
  if (relay->node_count == 2)
    return false; /* Quickie kludge for the 'standard' case */
  for (i = 0; i < relay->node_count; i++) {
    relay->visited[i] = false;
    relay->connected[i][i] = false;
    for (j = i + 1; j < relay->node_count; j++) {
      float range = mech_range_to(relay->mechs[i], relay->mechs[j]);

      relay->connected[i][j] = range <= mech_radio_range(relay->mechs[i]);
      relay->connected[j][i] = range <= mech_radio_range(relay->mechs[j]);
    }
  }
  relay->best_depth = 9999;
  nonrecursive_commlink(relay, 0); /* better _pray_ this works */
  return relay->best_depth != 9999;
}

/* The code that does the actual sending of radio messages whenever
 * someone speaks on a given frequency */
static void build_observer_channel_message(
    char *buf, const char *color, char open_bracket, char close_bracket,
    char channel, int bearing, const char *faction, const char *id,
    int frequency, const char *title, const char *message) {
  char *bp = buf;
  char numbuf[32];

  safe_str((char *)color, buf, &bp);
  safe_chr(open_bracket, buf, &bp);
  safe_chr(channel, buf, &bp);
  safe_chr(':', buf, &bp);
  snprintf(numbuf, sizeof(numbuf), "%d", bearing);
  safe_str(numbuf, buf, &bp);
  safe_chr(close_bracket, buf, &bp);
  safe_str(" <", buf, &bp);
  safe_str((char *)faction, buf, &bp);
  safe_chr(':', buf, &bp);
  safe_str((char *)id, buf, &bp);
  safe_chr(':', buf, &bp);
  snprintf(numbuf, sizeof(numbuf), "%d", frequency);
  safe_str(numbuf, buf, &bp);
  safe_str("> <", buf, &bp);
  safe_str((char *)title, buf, &bp);
  safe_str("> ", buf, &bp);
  safe_str((char *)message, buf, &bp);
  safe_str("[reset]", buf, &bp);
  *bp = '\0';
}

static void build_channel_message(char *buf, const char *color,
                                  char open_bracket, char close_bracket,
                                  char channel, int bearing,
                                  const char *message) {
  char *bp = buf;
  char numbuf[32];

  safe_str((char *)color, buf, &bp);
  safe_chr(open_bracket, buf, &bp);
  safe_chr(channel, buf, &bp);
  safe_chr(':', buf, &bp);
  snprintf(numbuf, sizeof(numbuf), "%.3d", bearing);
  safe_str(numbuf, buf, &bp);
  safe_chr(close_bracket, buf, &bp);
  safe_chr(' ', buf, &bp);
  safe_str((char *)message, buf, &bp);
  safe_str("[reset]", buf, &bp);
  *bp = '\0';
}

void sendchannelstuff(Mech *mech, int freq, char *msg) {
  /* The _smart_ code :-) */
  int loop, range, bearing, i, isxp;
  Mech *tempMech;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char buf[LBUF_SIZE];
  char buf2[LBUF_SIZE];
  char buf3[LBUF_SIZE];
  char color_code[32];
  int obs = 0;
  CommRelayContext *relay;

  char ai_buf[LBUF_SIZE];

  /* Removed the Radio Failing stuff cause it annoys me - Dany
     mech_generic_failure_check(mech, -2, &sfail_type, &sfail_mod);
   */
  if (!mech_radio_range(mech))
    return;
  relay = calloc(1, sizeof(*relay));

  /* Loop through all the units on the map */
  for (loop = 0; loop < mech_map->first_free; loop++) {
    if (mech_map->mechsOnMap[loop] != 2) {
      // XXX: The test below is indicative of very bad bookkeeping. Suggesting
      // that a dbref may be indicated as "on the map" without being on the
      // map. I believe this to be a serious problem.
      if (!(tempMech = (Mech *)btech_context_find_object(
                mech_context(mech), mech_map->mechsOnMap[loop])))
        continue;
      if (mech_is_destroyed(tempMech))
        continue;
      obs = mech_is_observer(tempMech);
      range = mech_range_to(mech, tempMech);
      bearing = FindBearing(
          mech_position_real_x(tempMech), mech_position_real_y(tempMech),
          mech_position_real_x(mech), mech_position_real_y(mech));
      for (i = 0; i < mech_radio_channel_count(tempMech); i++) {
        if (mech_radio_frequency(tempMech, i) ==
                mech_radio_frequency(mech, freq) ||
            obs) {
          if ((mech_radio_mode(tempMech, i) & FREQ_MUTE) ||
              ((mech_radio_mode(mech, freq) & FREQ_DIGITAL) &&
               (mech_radio_capabilities(tempMech) & RADIO_NODIGITAL)))
            continue;
          break;
        }
      }
      if (i >= mech_radio_channel_count(tempMech)) {
        /* Possible scanner check */
        if (!(mech_radio_mode(mech, freq) & FREQ_DIGITAL))
          if ((mech_radio_capabilities(tempMech) & RADIO_SCAN) &&
              mech_radio_frequency(mech, freq)) {
            int tnc = 0;

            for (i = 0; i < mech_radio_channel_count(tempMech); i++)
              if (mech_radio_mode(tempMech, i) & FREQ_SCAN) {
                int l = strlen(msg), t;
                int mod, diff;
                int pr;

                /* Possible skill check here? Nah. */

                /* Chance of detection: 1 in MIN(80,l) out of 100 */
                if (btech_random_range(mech_context(mech), 1, 100) > MIN(80, l))
                  continue;

                if (!tnc++)
                  mech_notify(tempMech, MECHALL,
                              "You notice a "
                              "unknown transmission your scanner.. ");
                if (mech_radio_frequency(tempMech, i) <
                    mech_radio_frequency(mech, freq)) {
                  diff = mech_radio_frequency(mech, freq) -
                         mech_radio_frequency(tempMech, i);
                  mod = 1;
                } else {
                  diff = mech_radio_frequency(tempMech, i) -
                         mech_radio_frequency(mech, freq);
                  mod = -1;
                }

                t = MAX(1,
                        btech_random_range(mech_context(mech), 1, MIN(99, l)) *
                            diff / 100);
                pr = t * 100 / diff;
                mech_printf(tempMech, MECHALL,
                            "Your systems "
                            "manage to zero on it %s on channel %c.",
                            pr < 30   ? "somewhat"
                            : pr < 60 ? "fairly well"
                            : pr < 95 ? "precisely"
                                      : "exactly",
                            i + 'A');
                mech_radio_frequency_add(tempMech, i, mod * t);
              }
          }

        continue;
      }

      snprintf(buf2, LBUF_SIZE, "%s", msg);
      radio_color_code(color_code, tempMech, i, obs, mech_team(mech));

      /* Let's just do the OBSERVERIC Stuff here. No sense checking
       * elsewhere. We'll compose the message and send it now since
       * it should technically hear everything */

      if (obs) {
        if (mech_radio_mode(mech, freq) & FREQ_DIGITAL) {
          build_observer_channel_message(
              buf, color_code, '[', ']', (char)('A' + i), bearing,
              btech_attribute_read(btech_context_database(mech_context(mech)),
                                   mech_dbref(mech), A_FACTION,
                                   (char[LBUF_SIZE]){0}),
              mech_id(mech, false).text, mech_radio_frequency(mech, freq),
              mech_radio_title(mech, freq), buf2);
        } else {
          build_observer_channel_message(
              buf, color_code, '(', ')', (char)('A' + i), bearing,
              btech_attribute_read(btech_context_database(mech_context(mech)),
                                   mech_dbref(mech), A_FACTION,
                                   (char[LBUF_SIZE]){0}),
              mech_id(mech, false).text, mech_radio_frequency(mech, freq),
              mech_radio_title(mech, freq), buf2);
        }
        mech_notify(tempMech, MECHALL, buf);
      }

      /* This is where we check to see if the mech has an AI and
       * then we give the radio commands to the AI */
      if (mech_autopilot_dbref(tempMech) > 0 &&
          mech_radio_frequency(tempMech, i)) {
        Autopilot *a = (Autopilot *)btech_context_find_object(
            mech_context(mech), mech_autopilot_dbref(tempMech));

        /* First check to make sure the AI is still there */
        if (!a) {
          /* No AI there so reset the AI value on the mech */
          mech_autopilot_dbref_set(tempMech, -1);
        } else if (a && game_object_location(
                            btech_context_database(mech_context(mech)),
                            a->mynum) != mech_dbref(tempMech)) {
          /* Check to see if the AI is still in the same mech */
          snprintf(ai_buf, LBUF_SIZE,
                   "Autopilot #%ld (Location: #%ld) "
                   "reported on Mech #%ld but not in the proper location",
                   a->mynum,
                   game_object_location(
                       btech_context_database(mech_context(mech)), a->mynum),
                   mech_dbref(tempMech));
          btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_AI, "%s",
                             ai_buf);
        } else if (a && !mech_is_ecm_disturbed(tempMech)) {
          /* Ok send the command to the AI provided its not ECM'd */
          snprintf(buf3, LBUF_SIZE, "%s", msg);
          auto_parse_command(a, tempMech, i, buf3);
        }
      }
      /* Removed the Radio fail stuff because it annoys me - Dany
         mech_generic_failure_check(tempMech, -2, &rfail_type, &rfail_mod);
       */
      if (!mech_radio_range(tempMech))
        continue;
      if (mech_radio_mode(mech, freq) & FREQ_DIGITAL) {
        if (relay != nullptr)
          relay->best_depth = 1;
        if (range > mech_radio_range(mech)) {
          if (relay == nullptr ||
              !find_comm_link(relay, mech_map, mech, tempMech,
                              mech_radio_frequency(mech, freq)))
            continue;
        }

        if (tempMech != mech) {
          if (mech_is_any_ecm_disturbed(mech))
            continue;
          else if (mech_is_any_ecm_disturbed(tempMech))
            continue;
        }

        scramble_message(relay, mech_context(mech), buf3, range,
                         mech_radio_range(mech), mech_radio_range(mech),
                         (char *)mech_radio_title(mech, freq), buf2,
                         mech_communication_skill(tempMech), &isxp, 0,
                         (mech_radio_mode(tempMech, i) & FREQ_INFO) ? 2 : 1);

        if (relay != nullptr && relay->best_depth >= 2)
          bearing = FindBearing(
              mech_position_real_x(tempMech), mech_position_real_y(tempMech),
              mech_position_real_x(
                  relay->mechs[relay->best_path[relay->best_depth - 1]]),
              mech_position_real_y(
                  relay->mechs[relay->best_path[relay->best_depth - 1]]));
        if (!obs)
          build_channel_message(buf, color_code, '[', ']', (char)('A' + i),
                                bearing, buf3);

      } else {

        scramble_message(relay, mech_context(mech), buf3, range,
                         mech_radio_range(mech), mech_radio_range(tempMech),
                         (char *)mech_radio_title(mech, freq), buf2,
                         mech_communication_skill(tempMech), &isxp,
                         (mech_is_any_ecm_disturbed(mech) ||
                          mech_is_any_ecm_disturbed(tempMech)
                          /*
                             || sfail_type == FAIL_STATIC ||
                             rfail_type == FAIL_STATIC
                           */
                          ) &&
                             mech != tempMech,
                         0);
        if (!obs)
          build_channel_message(buf, color_code, '(', ')', (char)('A' + i),
                                bearing, buf3);
      }

      if (!obs)
        mech_notify(tempMech, MECHALL, buf);
      if (isxp && is_in_character(btech_context_database(mech_context(mech)),
                                  mech_dbref(tempMech)))
        if ((mech_communication_last_tick(tempMech) + 60) <
            btech_context_event_tick(mech_context(mech))) {
          AccumulateCommXP(mech_pilot_dbref(tempMech), tempMech);
          mech_communication_last_tick_set(
              tempMech, btech_context_event_tick(mech_context(mech)));
        }
    }
  } /* End of looping through all the units on the map */
  free(relay);
}

void mech_radio(DbRef player, void *data, char *buffer) {
  int argc;
  int fail = 0;
  char *args[3];
  int i;
  Mech *mech = (Mech *)data;
  DbRef target;
  Mech *tempMech;

  /* radio <id>=message */
  /* Quick clone :-) */
  /* This is silly, but who cares. */
  if (!common_checks(player, mech, MECH_USUAL))
    return;

  if (mech_is_observer(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't radio anyone.");
    return;
  }
  if ((argc = proper_parseattributes(buffer, args, 3)) != 3)
    fail = 1;
  if (!fail && (!args[1] || args[1][0] != '=' || args[1][1] != 0))
    fail = 1;
  if (!fail &&
      (!args[0] || args[0][0] == 0 || args[0][1] == 0 || args[0][2] != 0))
    fail = 1;
  if (!fail) {
    target = FindTargetDBREFFromMapNumber(mech, args[0]);
    tempMech = btech_context_get_mech(mech_context(mech), target);
    if (!tempMech || !mech_los_check(mech, tempMech, mech_position_x(tempMech),
                                     mech_position_y(tempMech),
                                     mech_range_to(mech, tempMech))) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Target is not in line of sight!");
      return;
    }
    mech_printf(mech, MECHSTARTED, "You radio %s with, '%s'",
                mech_to_mech_display_id(mech, tempMech).text, args[2]);
    mech_printf(tempMech, MECHSTARTED, "%s radios you with, '%s'",
                mech_to_mech_display_id(tempMech, mech).text, args[2]);
    auto_reply(tempMech,
               tprintf("%s radio'ed me '%s'",
                       mech_to_mech_display_id(tempMech, mech).text, args[2]));
  }
  if (fail) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid format! Usage: radio <letter><letter>=<message>");
    return;
  }
  for (i = 0; i < 3; i++) {
    if (args[i])
      free(args[i]);
  }
}
