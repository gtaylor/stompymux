#include "mech_notify_internal.h"

void sendchannelstuff(Mech *mech, int freq, char *msg);

static void append_lbuf(char *buffer, size_t size, const char *fmt, ...)
    __attribute__((format(printf, 3, 4)));

static void append_lbuf(char *buffer, size_t size, const char *fmt, ...) {
  size_t len = strlen(buffer);
  va_list ap;

  if (len >= size)
    return;

  va_start(ap, fmt);
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

#define my_modify(n, fact) (100 - (100 - (n)) / (fact))

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
  char *header = NULL;
  char buf[LBUF_SIZE];

  *isxp = 0;

  if (digmode > 1 && relay != nullptr && relay->best_depth > 1) {
    int bearing;

    strncpy(buf, "{R-path:", LBUF_SIZE);
    for (i = 1; i < relay->best_depth; i++) {
      if (i > 1)
        strcat(buf, "/");
      bearing = FindBearing(MechFX(relay->mechs[relay->best_path[i]]),
                            MechFY(relay->mechs[relay->best_path[i]]),
                            MechFX(relay->mechs[relay->best_path[i - 1]]),
                            MechFY(relay->mechs[relay->best_path[i - 1]]));
      snprintf(buf + strlen(buf), sizeof(buf) - strlen(buf), "[%c%c]-h:%.3d",
               MechID(relay->mechs[relay->best_path[i]])[0],
               MechID(relay->mechs[relay->best_path[i]])[1], bearing);
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
        do_scramble(context, buffo, my_modify((100 * mr) / MAX(1, range), 2),
                    bth);
        *isxp = 1;
      }
    }

    if (under_ecm && range >= 1) {
      do_scramble(context, buffo, btech_random_range(context, 30, 50), bth);
      *isxp = 1;
    }
  }
}

int common_checks(DbRef player, Mech *mech, int flag) {
  BattleMap *mech_map;

  if (!mech)
    return 0;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);

  if (!is_wizard(mech->xcode.context->database, player)) {
    /* ----------------------------- */
    /* INSERT UNSUPPORTED TYPES HERE */
    /* ----------------------------- */

    /*
                    if(MechType(mech) == CLASS_AERO)
                            return 0;
    */
    /* ----------------------------- */
  }

  /*
      if (MechAuto(mech) > 0)
          if (is_player(mech->xcode.context->database,
     MechPilot(mech))) MechAuto(mech) = -1;
  */
  MechLastUse(mech) = 0;

  if (flag & MECH_STARTED) {
    DOCHECK0_CONTEXT(mech->xcode.context, Destroyed(mech),
                     "You are destroyed!");
    DOCHECK0_CONTEXT(mech->xcode.context, !(MechStatus(mech) & STARTED),
                     "Reactor is not online!");
  }

  if (flag & MECH_PILOT) {
    DOCHECK0_CONTEXT(mech->xcode.context, Blinded(mech),
                     "You are momentarily blinded!");
  }

  if (flag & MECH_PILOT_CON)
    DOCHECK0_CONTEXT(mech->xcode.context,
                     Uncon(mech) &&
                         (!Started(mech) || player == MechPilot(mech)),
                     "You are unconscious....zzzzzzz");

  if (flag & MECH_PILOTONLY)
    DOCHECK0_CONTEXT(
        mech->xcode.context,
        !is_wizard(mech->xcode.context->database, player) &&
            is_in_character(mech->xcode.context->database, mech->mynum) &&
            MechPilot(mech) != player,
        "Now now, only the pilot can push that button.");

  if (flag & MECH_MAP) {
    DOCHECK0_CONTEXT(mech->xcode.context, mech->mapindex < 0,
                     "You are on no map!");
    mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);
    if (!mech_map) {
      notify(evaluation, player, "You are on an invalid map! Map index reset!");
      mech_shutdown(player, (void *)mech, "");
      mech->mapindex = -1;
      return 0;
    }
  }
  return 1;
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
    if (!(t = btech_context_find_object(from->xcode.context,
                                        map->mechsOnMap[i])))
      continue;
    if (t == from || t == to)
      continue;
    if (MechTeam(from) != MechTeam(t))
      continue;
    if ((MechMove(t) != MOVE_NONE && !Started(t)) ||
        (MechMove(t) == MOVE_NONE && Destroyed(t)))
      continue;
    if (!(MechRadioInfo(t) & RADIO_RELAY))
      continue;
    for (j = 0; j < MFreqs(t); j++)
      if (t->freq[j] == freq)
        if (t->freqmodes[j] & FREQ_RELAY) {
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
      float range = FlMechRange(map, relay->mechs[i], relay->mechs[j]);

      relay->connected[i][j] = (range <= MechRadioRange(relay->mechs[i]));
      relay->connected[j][i] = (range <= MechRadioRange(relay->mechs[j]));
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
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  char buf[LBUF_SIZE];
  char buf2[LBUF_SIZE];
  char buf3[LBUF_SIZE];
  char color_code[32];
  int obs = 0;
  CommRelayContext *relay;

  char ai_buf[LBUF_SIZE];

  /* Removed the Radio Failing stuff cause it annoys me - Dany
     CheckGenericFail(mech, -2, &sfail_type, &sfail_mod);
   */
  if (!MechRadioRange(mech))
    return;
  relay = calloc(1, sizeof(*relay));

  /* Loop through all the units on the map */
  for (loop = 0; loop < mech_map->first_free; loop++) {
    if (mech_map->mechsOnMap[loop] != 2) {
      // XXX: The test below is indicative of very bad bookkeeping. Suggesting
      // that a dbref may be indicated as "on the map" without being on the
      // map. I believe this to be a serious problem.
      if (!(tempMech = (Mech *)btech_context_find_object(
                mech->xcode.context, mech_map->mechsOnMap[loop])))
        continue;
      if (Destroyed(tempMech))
        continue;
      obs = (MechCritStatus(tempMech) & OBSERVATORIC);
      range = FaMechRange(mech, tempMech);
      bearing = FindBearing(MechFX(tempMech), MechFY(tempMech), MechFX(mech),
                            MechFY(mech));
      for (i = 0; i < MFreqs(tempMech); i++) {
        if (tempMech->freq[i] == mech->freq[freq] || obs) {
          if ((tempMech->freqmodes[i] & FREQ_MUTE) ||
              ((mech->freqmodes[freq] & FREQ_DIGITAL) &&
               (MechRadioInfo(tempMech) & RADIO_NODIGITAL)))
            continue;
          break;
        }
      }
      if (i >= MFreqs(tempMech)) {
        /* Possible scanner check */
        if (!(mech->freqmodes[freq] & FREQ_DIGITAL))
          if ((MechRadioInfo(tempMech) & RADIO_SCAN) && mech->freq[freq]) {
            int tnc = 0;

            for (i = 0; i < MFreqs(tempMech); i++)
              if (tempMech->freqmodes[i] & FREQ_SCAN) {
                int l = strlen(msg), t;
                int mod, diff;
                int pr;

                /* Possible skill check here? Nah. */

                /* Chance of detection: 1 in MIN(80,l) out of 100 */
                if (btech_random_range(mech->xcode.context, 1, 100) >
                    MIN(80, l))
                  continue;

                if (!tnc++)
                  mech_notify(tempMech, MECHALL,
                              "You notice a "
                              "unknown transmission your scanner.. ");
                if (tempMech->freq[i] < mech->freq[freq]) {
                  diff = mech->freq[freq] - tempMech->freq[i];
                  mod = 1;
                } else {
                  diff = tempMech->freq[i] - mech->freq[freq];
                  mod = -1;
                }

                t = MAX(1,
                        btech_random_range(mech->xcode.context, 1, MIN(99, l)) *
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
                tempMech->freq[i] += mod * t;
              }
          }

        continue;
      }

      snprintf(buf2, LBUF_SIZE, "%s", msg);
      radio_color_code(color_code, tempMech, i, obs, MechTeam(mech));

      /* Let's just do the OBSERVERIC Stuff here. No sense checking
       * elsewhere. We'll compose the message and send it now since
       * it should technically hear everything */

      if (obs) {
        if (mech->freqmodes[freq] & FREQ_DIGITAL) {
          build_observer_channel_message(
              buf, color_code, '[', ']', (char)('A' + i), bearing,
              btech_attribute_read(mech->xcode.context->database, mech->mynum,
                                   A_FACTION, (char[LBUF_SIZE]){0}),
              mech_id(mech, false).text, mech->freq[freq],
              mech->chantitle[freq], buf2);
        } else {
          build_observer_channel_message(
              buf, color_code, '(', ')', (char)('A' + i), bearing,
              btech_attribute_read(mech->xcode.context->database, mech->mynum,
                                   A_FACTION, (char[LBUF_SIZE]){0}),
              mech_id(mech, false).text, mech->freq[freq],
              mech->chantitle[freq], buf2);
        }
        mech_notify(tempMech, MECHALL, buf);
      }

      /* This is where we check to see if the mech has an AI and
       * then we give the radio commands to the AI */
      if (MechAuto(tempMech) > 0 && tempMech->freq[i]) {
        Autopilot *a = (Autopilot *)btech_context_find_object(
            mech->xcode.context, MechAuto(tempMech));

        /* First check to make sure the AI is still there */
        if (!a) {
          /* No AI there so reset the AI value on the mech */
          MechAuto(tempMech) = -1;
        } else if (a && game_object_location(mech->xcode.context->database,
                                             a->mynum) != tempMech->mynum) {
          /* Check to see if the AI is still in the same mech */
          snprintf(
              ai_buf, LBUF_SIZE,
              "Autopilot #%ld (Location: #%ld) "
              "reported on Mech #%ld but not in the proper location",
              a->mynum,
              game_object_location(mech->xcode.context->database, a->mynum),
              tempMech->mynum);
          btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                             ai_buf);
        } else if (a && !ECMDisturbed(tempMech)) {
          /* Ok send the command to the AI provided its not ECM'd */
          snprintf(buf3, LBUF_SIZE, "%s", msg);
          auto_parse_command(a, tempMech, i, buf3);
        }
      }
      /* Removed the Radio fail stuff because it annoys me - Dany
         CheckGenericFail(tempMech, -2, &rfail_type, &rfail_mod);
       */
      if (!MechRadioRange(tempMech))
        continue;
      if (mech->freqmodes[freq] & FREQ_DIGITAL) {
        if (relay != nullptr)
          relay->best_depth = 1;
        if (range > MechRadioRange(mech)) {
          if (relay == nullptr || !find_comm_link(relay, mech_map, mech,
                                                  tempMech, mech->freq[freq]))
            continue;
        }

        if (tempMech != mech) {
          if (AnyECMDisturbed(mech))
            continue;
          else if (AnyECMDisturbed(tempMech))
            continue;
        }

        scramble_message(relay, mech->xcode.context, buf3, range,
                         MechRadioRange(mech), MechRadioRange(mech),
                         mech->chantitle[freq], buf2, MechComm(tempMech), &isxp,
                         0, (tempMech->freqmodes[i] & FREQ_INFO) ? 2 : 1);

        if (relay != nullptr && relay->best_depth >= 2)
          bearing = FindBearing(
              MechFX(tempMech), MechFY(tempMech),
              MechFX(relay->mechs[relay->best_path[relay->best_depth - 1]]),
              MechFY(relay->mechs[relay->best_path[relay->best_depth - 1]]));
        if (!obs)
          build_channel_message(buf, color_code, '[', ']', (char)('A' + i),
                                bearing, buf3);

      } else {

        scramble_message(relay, mech->xcode.context, buf3, range,
                         MechRadioRange(mech), MechRadioRange(tempMech),
                         mech->chantitle[freq], buf2, MechComm(tempMech), &isxp,
                         (AnyECMDisturbed(mech) || AnyECMDisturbed(tempMech)
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
      if (isxp &&
          is_in_character(mech->xcode.context->database, tempMech->mynum))
        if ((MechCommLast(tempMech) + 60) < mech->xcode.context->events->tick) {
          AccumulateCommXP(MechPilot(tempMech), tempMech);
          MechCommLast(tempMech) = mech->xcode.context->events->tick;
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
  cch(MECH_USUAL);

  DOCHECK_CONTEXT(mech->xcode.context, MechIsObservator(mech),
                  "You can't radio anyone.");
  if ((argc = proper_parseattributes(buffer, args, 3)) != 3)
    fail = 1;
  if (!fail && (!args[1] || args[1][0] != '=' || args[1][1] != 0))
    fail = 1;
  if (!fail &&
      (!args[0] || args[0][0] == 0 || args[0][1] == 0 || args[0][2] != 0))
    fail = 1;
  if (!fail) {
    target = FindTargetDBREFFromMapNumber(mech, args[0]);
    tempMech = btech_context_get_mech(mech->xcode.context, target);
    DOCHECK_CONTEXT(mech->xcode.context,
                    !tempMech ||
                        !InLineOfSight(mech, tempMech, MechX(tempMech),
                                       MechY(tempMech),
                                       FlMechRange(map, mech, tempMech)),
                    "Target is not in line of sight!");
    mech_printf(mech, MECHSTARTED, "You radio %s with, '%s'",
                mech_to_mech_display_id(mech, tempMech).text, args[2]);
    mech_printf(tempMech, MECHSTARTED, "%s radios you with, '%s'",
                mech_to_mech_display_id(tempMech, mech).text, args[2]);
    auto_reply(tempMech,
               tprintf("%s radio'ed me '%s'",
                       mech_to_mech_display_id(tempMech, mech).text, args[2]));
  }
  DOCHECK_CONTEXT(mech->xcode.context, fail,
                  "Invalid format! Usage: radio <letter><letter>=<message>");
  for (i = 0; i < 3; i++) {
    if (args[i])
      free(args[i]);
  }
}

int MapLimitedBroadcast2d(BattleMap *map, float x, float y, float range,
                          char *message) {
  int loop, count = 0;
  Mech *mech;

  for (loop = 0; loop < map->first_free; loop++) {
    if (map->mechsOnMap[loop] < 0)
      continue;
    mech = btech_context_get_mech(map->xcode.context, map->mechsOnMap[loop]);

    if (mech && FindXYRange(x, y, MechFX(mech), MechFY(mech)) <= range) {
      mech_notify(mech, MECHSTARTED, message);
      count++;
    }
  }
  return count;
}

int MapLimitedBroadcast3d(BattleMap *map, float x, float y, float z,
                          float range, char *message) {
  int loop, count = 0;
  Mech *mech;

  for (loop = 0; loop < map->first_free; loop++) {
    if (map->mechsOnMap[loop] == -1)
      continue;
    mech = btech_context_get_mech(map->xcode.context, map->mechsOnMap[loop]);
    if (mech &&
        FindRange(x, y, z, MechFX(mech), MechFY(mech), MechFZ(mech)) <= range) {
      count++;
      mech_notify(mech, MECHSTARTED, message);
    }
  }
  return count;
}

void MechBroadcast(Mech *mech, Mech *target, BattleMap *mech_map,
                   char *buffer) {
  int loop;
  Mech *tempMech;

  if (target) {
    for (loop = 0; loop < mech_map->first_free; loop++) {
      if (mech_map->mechsOnMap[loop] != mech->mynum &&
          mech_map->mechsOnMap[loop] != -1 &&
          mech_map->mechsOnMap[loop] != target->mynum) {
        tempMech = (Mech *)btech_context_find_object(
            mech->xcode.context, mech_map->mechsOnMap[loop]);
        if (tempMech)
          mech_notify(tempMech, MECHSTARTED, buffer);
      }
    }
  } else {
    for (loop = 0; loop < mech_map->first_free; loop++) {
      if (mech_map->mechsOnMap[loop] != mech->mynum &&
          mech_map->mechsOnMap[loop] != -1) {
        tempMech = (Mech *)btech_context_find_object(
            mech->xcode.context, mech_map->mechsOnMap[loop]);
        if (tempMech)
          mech_notify(tempMech, MECHSTARTED, buffer);
      }
    }
  }
}
