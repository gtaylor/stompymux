#include "autopilot.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_text_builder.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_electronics_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_radio_api.h"
#include "mech_radio_render_internal.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"

#include <math.h>
#include <stdarg.h>
#include <stdint.h>
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
  vsnprintf(checked_storage_region(buffer, size, len, size - len), size - len,
            fmt, ap);
  va_end(ap);
}

static void do_scramble(BtechContext *context, char *buffo, int ch, int bth) {
  const size_t length = strlen(buffo);

  for (size_t i = 0; i < length; i++) {
    char *character = checked_storage_at(buffo, length + 1, sizeof(char), i);
    if (btech_random_range(context, 1, 100) > ch &&
        btech_random_roll(context) < (bth + 5)) {
      if (btech_random_range(context, 1, 2) == 1)
        *character -= btech_random_range(context, 1, 10);
      else
        *character += btech_random_range(context, 1, 10);
    }
    *character = (char)(unsigned char)BOUNDED(33, *character, 255);
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

typedef struct RelaySearchStack {
  int items[MAX_MECHS_PER_MAP];
} RelaySearchStack;

static void relay_index_set(int values[MAX_MECHS_PER_MAP], int index,
                            int value) {
  if (index < 0)
    abort();
  int *slot = checked_storage_at(values, MAX_MECHS_PER_MAP, sizeof(*values),
                                 (size_t)index);
  *slot = value;
}

static int relay_index_get(const int values[MAX_MECHS_PER_MAP], int index) {
  if (index < 0)
    abort();
  return *(const int *)checked_storage_at_const(values, MAX_MECHS_PER_MAP,
                                                sizeof(*values), (size_t)index);
}

static void relay_mech_set(CommRelayContext *relay, int index, Mech *mech) {
  if (index < 0)
    abort();
  Mech **slot = checked_storage_at(relay->mechs, MAX_MECHS_PER_MAP,
                                   sizeof(*relay->mechs), (size_t)index);
  *slot = mech;
}

static Mech *relay_mech_get(const CommRelayContext *relay, int index) {
  if (index < 0)
    abort();
  return *(Mech *const *)checked_storage_at_const(
      relay->mechs, MAX_MECHS_PER_MAP, sizeof(*relay->mechs), (size_t)index);
}

static void relay_visited_set(CommRelayContext *relay, int index, bool value) {
  if (index < 0)
    abort();
  bool *slot = checked_storage_at(relay->visited, MAX_MECHS_PER_MAP,
                                  sizeof(*relay->visited), (size_t)index);
  *slot = value;
}

static bool relay_visited_get(const CommRelayContext *relay, int index) {
  if (index < 0)
    abort();
  return *(const bool *)checked_storage_at_const(
      relay->visited, MAX_MECHS_PER_MAP, sizeof(*relay->visited),
      (size_t)index);
}

static void relay_connected_set(CommRelayContext *relay, int from, int to,
                                bool value) {
  if (from < 0 || to < 0)
    abort();
  bool (*row)[MAX_MECHS_PER_MAP] =
      checked_storage_at(relay->connected, MAX_MECHS_PER_MAP,
                         sizeof(*relay->connected), (size_t)from);
  bool *slot =
      checked_storage_at(*row, MAX_MECHS_PER_MAP, sizeof(**row), (size_t)to);
  *slot = value;
}

static bool relay_connected_get(const CommRelayContext *relay, int from,
                                int to) {
  if (from < 0 || to < 0)
    abort();
  const bool (*row)[MAX_MECHS_PER_MAP] =
      checked_storage_at_const(relay->connected, MAX_MECHS_PER_MAP,
                               sizeof(*relay->connected), (size_t)from);
  return *(const bool *)checked_storage_at_const(*row, MAX_MECHS_PER_MAP,
                                                 sizeof(**row), (size_t)to);
}

static char *radio_argument(char *const arguments[3], int index) {
  if (index < 0)
    abort();
  return *(char *const *)checked_storage_at_const(
      arguments, 3, sizeof(*arguments), (size_t)index);
}

static void scramble_message(const CommRelayContext *relay,
                             BtechContext *context, char *buffo, float range,
                             int sendrange, int recvrrange, const char *handle,
                             const char *msg, int bth, int *isxp, int under_ecm,
                             int digmode) {

  int mr, i;
  char *header = nullptr;
  char buf[LBUF_SIZE];

  *isxp = 0;

  if (digmode > 1 && relay != nullptr && relay->best_depth > 1) {
    int bearing;
    BtechTextBuilder path;

    btech_text_builder_initialize(&path, buf, sizeof(buf));
    btech_text_builder_append(&path, "{R-path:");
    for (i = 1; i < relay->best_depth; i++) {
      if (i > 1)
        btech_text_builder_append_character(&path, '/');
      Mech *current =
          relay_mech_get(relay, relay_index_get(relay->best_path, i));
      Mech *previous =
          relay_mech_get(relay, relay_index_get(relay->best_path, i - 1));
      bearing = FindBearing(
          mech_position_real_x(current), mech_position_real_y(current),
          mech_position_real_x(previous), mech_position_real_y(previous));
      MechUnitId id = mech_unit_id(current);
      btech_text_builder_append_format(&path, "[%c%c]-h:%.3d", id.first,
                                       id.second, bearing);
    }
    btech_text_builder_append(&path, "} ");
    header = buf;
  }

  *(char *)checked_storage_at(buffo, LBUF_SIZE, sizeof(char), 0) = '\0';
  append_lbuf(buffo, LBUF_SIZE, "%s", header ? header : "");
  if (handle && *handle)
    append_lbuf(buffo, LBUF_SIZE, "<%s> ", handle);
  append_lbuf(buffo, LBUF_SIZE, "%s", msg);

  const float send_range = (float)sendrange;
  const float receive_range = (float)recvrrange;

  if ((!digmode && (range >= send_range || range >= receive_range)) ||
      under_ecm) {
    if (!digmode) {

      mr = MAX(recvrrange, (sendrange + recvrrange) / 2);
      if (send_range < range) {
        const float signal = 100.0F * send_range / fmaxf(1.0F, range);

        do_scramble(context, buffo, clamp_float_to_int(signal), bth);
        *isxp = 1;
      }

      if ((float)mr < range) {
        const float signal = 100.0F * (float)mr / fmaxf(1.0F, range);

        do_scramble(context, buffo,
                    relay_signal_improve(clamp_float_to_int(signal), 2), bth);
        *isxp = 1;
      }
    }

    if (under_ecm && range >= 1.0F) {
      const long random_chance = btech_random_range(context, 30, 50);

      do_scramble(context, buffo, clamp_intptr_to_int((intptr_t)random_chance),
                  bth);
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
  relay_index_set(relay->path, dep, i);
  for (j = 1; j < relay->node_count; j++)
    if (relay_connected_get(relay, i, j) && !relay_visited_get(relay, j)) {
      if (j == (relay->node_count - 1)) {
        int k;

        relay->best_depth = dep;
        for (k = 0; k < relay->best_depth; k++)
          relay_index_set(relay->best_path, k, relay_index_get(relay->path, k));
      } else {
        relay_visited_set(relay, j, true);
        recursive_commlink(relay, j, dep + 1);
        relay_visited_set(relay, j, false);
      }
    }
}

static void nonrecursive_commlink(CommRelayContext *relay, int i) {
  int dep = 0, j;
  RelaySearchStack comm_loop = {0};
  int iter_c = 0;
  int maxdepth = 0;

  /* May _still_ contain fatal bug ; Ghod knows (I don't) */
  relay_index_set(comm_loop.items, 0, 1);
  relay_index_set(relay->path, 0, i);

  while (dep >= 0) {
    i = relay_index_get(relay->path, dep);
    for (j = relay_index_get(comm_loop.items, dep); j < relay->node_count; j++)
      if (relay_connected_get(relay, i, j) && !relay_visited_get(relay, j)) {
        if (j == (relay->node_count - 1)) {
          int k;

          relay->best_depth = dep + 1;
          for (k = 0; k < relay->best_depth; k++)
            relay_index_set(relay->best_path, k,
                            relay_index_get(relay->path, k));
          j = relay->node_count;
          break;
        } else if ((dep + 1) < relay->best_depth) {
          relay_visited_set(relay, j, true);
          relay_index_set(comm_loop.items, dep++, j + 1);
          relay_index_set(comm_loop.items, dep, 1);
          relay_index_set(relay->path, dep, j);
          if (dep > maxdepth)
            maxdepth = dep;
          break;
        }
      }
    if (j == relay->node_count) {
      if (dep > 0)
        relay_visited_set(relay, relay_index_get(comm_loop.items, --dep) - 1,
                          false);
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
        relay_visited_set(relay, i, false);
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
  relay_mech_set(relay, relay->node_count++, from);
  for (i = 0; i < battle_map_unit_count(map); i++) {
    const DbRef candidate = battle_map_unit_dbref(map, i);
    if (!(t = btech_context_find_object(mech_context(from), candidate)))
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
            relay_mech_set(relay, relay->node_count++, t);
          continue;
        }
  }
  relay_mech_set(relay, relay->node_count++, to);
  if (relay->node_count == 2)
    return false; /* Quickie kludge for the 'standard' case */
  for (i = 0; i < relay->node_count; i++) {
    relay_visited_set(relay, i, false);
    relay_connected_set(relay, i, i, false);
    for (j = i + 1; j < relay->node_count; j++) {
      Mech *source = relay_mech_get(relay, i);
      Mech *target = relay_mech_get(relay, j);
      float range = mech_range_to(source, target);

      const int source_range = mech_radio_range(source);
      const int target_range = mech_radio_range(target);

      relay_connected_set(relay, i, j, range <= (float)source_range);
      relay_connected_set(relay, j, i, range <= (float)target_range);
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
  BtechTextBuilder output;
  btech_text_builder_initialize(&output, buf, LBUF_SIZE);
  btech_text_builder_append(&output, color);
  btech_text_builder_append_format(
      &output, "%c%c:%d%c <%s:%s:%d> <%s> %s[reset]", open_bracket, channel,
      bearing, close_bracket, faction, id, frequency, title, message);
}

static void build_channel_message(char *buf, const char *color,
                                  char open_bracket, char close_bracket,
                                  char channel, int bearing,
                                  const char *message) {
  BtechTextBuilder output;
  btech_text_builder_initialize(&output, buf, LBUF_SIZE);
  btech_text_builder_append(&output, color);
  btech_text_builder_append_format(&output, "%c%c:%.3d%c %s[reset]",
                                   open_bracket, channel, bearing,
                                   close_bracket, message);
}

void sendchannelstuff(Mech *mech, int freq, char *msg) {
  /* The _smart_ code :-) */
  int loop, bearing, i, isxp;
  float range;
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
  for (loop = 0; loop < battle_map_unit_count(mech_map); loop++) {
    const DbRef candidate = battle_map_unit_dbref(mech_map, loop);
    if (candidate != 2) {
      // XXX: The test below is indicative of very bad bookkeeping. Suggesting
      // that a dbref may be indicated as "on the map" without being on the
      // map. I believe this to be a serious problem.
      if (!(tempMech = (Mech *)btech_context_find_object(mech_context(mech),
                                                         candidate)))
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
                int l = clamp_size_to_int(strlen(msg)), t;
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

                const long random_fraction =
                    btech_random_range(mech_context(mech), 1, MIN(99, l));
                const int64_t scaled_fraction =
                    (int64_t)random_fraction * (int64_t)diff / 100;

                t = MAX(1, clamp_intptr_to_int((intptr_t)scaled_fraction));
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
        const int source_range = mech_radio_range(mech);
        if (range > (float)source_range) {
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
                         mech_radio_title(mech, freq), buf2,
                         mech_communication_skill(tempMech), &isxp, 0,
                         (mech_radio_mode(tempMech, i) & FREQ_INFO) ? 2 : 1);

        if (relay != nullptr && relay->best_depth >= 2) {
          const int relay_index =
              relay_index_get(relay->best_path, relay->best_depth - 1);
          Mech *last_relay = relay_mech_get(relay, relay_index);
          bearing = FindBearing(mech_position_real_x(tempMech),
                                mech_position_real_y(tempMech),
                                mech_position_real_x(last_relay),
                                mech_position_real_y(last_relay));
        }
        if (!obs)
          build_channel_message(buf, color_code, '[', ']', (char)('A' + i),
                                bearing, buf3);

      } else {

        scramble_message(relay, mech_context(mech), buf3, range,
                         mech_radio_range(mech), mech_radio_range(tempMech),
                         mech_radio_title(mech, freq), buf2,
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
  int fail = 0;
  char *args[3] = {0};
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
  if (proper_parseattributes(buffer, args, 3) != 3)
    fail = 1;
  char *separator = radio_argument(args, 1);
  char *target_id = radio_argument(args, 0);
  char *message = radio_argument(args, 2);
  if (!fail && (separator == nullptr || strcmp(separator, "=") != 0))
    fail = 1;
  if (!fail && (target_id == nullptr || strlen(target_id) != 2))
    fail = 1;
  if (!fail) {
    target = FindTargetDBREFFromMapNumber(mech, target_id);
    tempMech = btech_context_get_mech(mech_context(mech), target);
    if (!tempMech || !mech_los_check(mech, tempMech, mech_position_x(tempMech),
                                     mech_position_y(tempMech),
                                     mech_range_to(mech, tempMech))) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Target is not in line of sight!");
      return;
    }
    mech_printf(mech, MECHSTARTED, "You radio %s with, '%s'",
                mech_to_mech_display_id(mech, tempMech).text, message);
    mech_printf(tempMech, MECHSTARTED, "%s radios you with, '%s'",
                mech_to_mech_display_id(tempMech, mech).text, message);
    auto_reply(tempMech,
               tprintf("%s radio'ed me '%s'",
                       mech_to_mech_display_id(tempMech, mech).text, message));
  }
  if (fail) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid format! Usage: radio <letter><letter>=<message>");
    return;
  }
  for (i = 0; i < 3; i++) {
    char *argument = radio_argument(args, i);
    if (argument)
      free(argument);
  }
}
