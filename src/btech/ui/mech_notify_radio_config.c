#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "mech_classification_api.h"
#include "mech_electronics_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify.h"
#include "mech_radio_api.h"
#include "mech_radio_render_internal.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mine_api.h"
#include "mux/communication/comsys.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "registry_api.h"

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void mech_set_channelfreq(DbRef player, void *data, char *buffer) {
  int chn = -1;
  int freq;
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  Mech *t;
  // map pointer is NULL if in a carrier. Careful.
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int i, j;

  /* UH, this is code that _pretends_ it works :-) */
  skipws(buffer);
  DOCHECK_CONTEXT(mech_context(mech), !*buffer, "Invalid input!");
  chn = toupper(*buffer) - 'A';
  DOCHECK_CONTEXT(mech_context(mech),
                  chn < 0 || chn >= mech_radio_channel_count(mech),
                  "Invalid channel-letter!");
  buffer++;
  skipws(buffer);
  DOCHECK_CONTEXT(mech_context(mech), !*buffer, "Invalid input!");
  DOCHECK_CONTEXT(mech_context(mech), *buffer != '=', "Missing =!");
  buffer++;
  skipws(buffer);
  DOCHECK_CONTEXT(mech_context(mech), !*buffer, "Invalid input!");
  freq = atoi(buffer);
  DOCHECK_CONTEXT(mech_context(mech), !freq && strcmp(buffer, "0"),
                  "Invalid frequency!");
  DOCHECK_CONTEXT(mech_context(mech), freq < 0, "Are you trying to kid me?");
  DOCHECK_CONTEXT(mech_context(mech), freq > 999999,
                  "Invalid frequency - range is from 0 to 999999.");
  notify_printf(evaluation, player, "Channel %c set to %d.", 'A' + chn, freq);
  mech_radio_frequency_set(mech, chn, freq);

  /* Code added from Exile to check for possible cheat freq acquring.
   * When a player sets a freq, it loops through all the mechs on the
   * map that do not belong to the same team and checks their freqs
   * against the one set. If it matches it emits message
   */
  if (freq > 0 && map) {
    for (i = 0; i < map->first_free; i++) {
      if (!(t = btech_context_find_object(mech_context(mech),
                                          map->mechsOnMap[i])))
        continue;
      if (t == mech)
        continue;
      if (mech_team(t) == mech_team(mech))
        continue;
      for (j = 0; j < mech_radio_channel_count(t); j++) {
        if (mech_radio_frequency(t, j) == freq &&
            !(mech_radio_mode(t, j) & FREQ_SCAN))
          btech_channel_send(
              mech_context(mech), BTECH_CHANNEL_MECH_FREQS, "%s",
              tprintf("ALERT: Possible abuse by #%ld (Team %d)"
                      " setting freq %d matching #%ld (Team %d)!",
                      mech_dbref(mech), mech_team(mech), freq, mech_dbref(t),
                      mech_team(t)));
      }
    }
  }
}

void mech_set_channeltitle(DbRef player, void *data, char *buffer) {
  int chn = -1;
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));

  skipws(buffer);
  DOCHECK_CONTEXT(mech_context(mech), !*buffer, "Invalid input!");
  chn = toupper(*buffer) - 'A';
  DOCHECK_CONTEXT(mech_context(mech),
                  chn < 0 || chn >= mech_radio_channel_count(mech),
                  "Invalid channel-letter!");
  buffer++;
  skipws(buffer);
  DOCHECK_CONTEXT(mech_context(mech), !*buffer, "Invalid input!");
  DOCHECK_CONTEXT(mech_context(mech), *buffer != '=', "Missing =!");
  buffer++;
  skipws(buffer);
  if (!*buffer) {
    mech_radio_title_set(mech, chn, "");
    notify_printf(evaluation, player, "Channel %c title cleared.", 'A' + chn);
    return;
  }
  mech_radio_title_set(mech, chn, buffer);
  notify_printf(evaluation, player, "Channel %c title set to set to %s.",
                'A' + chn, buffer);
}

/*                    1234567890123456 */
const char radio_colorstr[] = "xrgybmcwXRGYBMCW";

static const struct {
  int team;
  const char *color_code;
} OBSERVER_TEAM_COLORS[] = {
    {1, "[fg=white]"},        {2, "[fg=cyan]"},          {3, "[fg=magenta]"},
    {4, "[fg=blue]"},         {5, "[fg=yellow]"},        {6, "[fg=green]"},
    {7, "[fg=red]"},          {8, "[fg=black bold]"},    {9, "[fg=white bold]"},
    {10, "[fg=cyan bold]"},   {11, "[fg=magenta bold]"}, {12, "[fg=blue bold]"},
    {13, "[fg=yellow bold]"}, {14, "[fg=green bold]"},   {15, "[fg=red bold]"},
    {0, "[fg=white bold]"}};

static const char *const radio_color_styles[] = {
    "[fg=black]",      "[fg=red]",          "[fg=green]",
    "[fg=yellow]",     "[fg=blue]",         "[fg=magenta]",
    "[fg=cyan]",       "[fg=white]",        "[fg=black bold]",
    "[fg=red bold]",   "[fg=green bold]",   "[fg=yellow bold]",
    "[fg=blue bold]",  "[fg=magenta bold]", "[fg=cyan bold]",
    "[fg=white bold]",
};

void radio_color_code(char buffer[static 32], Mech *m, int i, int obs,
                      int team) {
  int t = mech_radio_mode(m, i) / FREQ_REST;
  int ii;

  buffer[0] = '\0';
  if (!obs) {
    if (!t)
      return;
    snprintf(buffer, 32, "%s", radio_color_styles[t - 1]);
  } else {
    if (team > 15)
      team = team % 15;
    for (ii = 0; ii < 15; ii++) {
      if (team == OBSERVER_TEAM_COLORS[ii].team)
        snprintf(buffer, 32, "%s", OBSERVER_TEAM_COLORS[ii].color_code);
    }
  }
}

void mech_set_channelmode(DbRef player, void *data, char *buffer) {
  int chn = -1, nm = 0, i;
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  char buf[SBUF_SIZE] = {0};

  skipws(buffer);
  DOCHECK_CONTEXT(mech_context(mech), !*buffer, "Invalid input!");
  chn = toupper(*buffer) - 'A';
  DOCHECK_CONTEXT(mech_context(mech),
                  chn < 0 || chn >= mech_radio_channel_count(mech),
                  "Invalid channel-letter!");
  buffer++;
  skipws(buffer);
  DOCHECK_CONTEXT(mech_context(mech), !*buffer, "Invalid input!");
  DOCHECK_CONTEXT(mech_context(mech), *buffer != '=', "Missing =!");
  buffer++;
  skipws(buffer);
  if (!buffer || !*buffer) {
    mech_radio_mode_set(mech, chn, 0);
    notify_printf(evaluation, player, "Channel %c <send> mode set to analog.",
                  'A' + chn);
    return;
  }
  while (buffer && *buffer) {
    switch (*buffer) {
    case 'D':
    case 'd':
      DOCHECK_CONTEXT(mech_context(mech),
                      mech_radio_capabilities(mech) & RADIO_NODIGITAL,
                      "Your radio can't handle digital frequencies!");
      nm |= FREQ_DIGITAL;
      break;
    case 'I':
    case 'i':
      DOCHECK_CONTEXT(mech_context(mech),
                      !(mech_radio_capabilities(mech) & RADIO_INFO),
                      "This unit is unable to use info functionality.");
      nm |= FREQ_INFO;
      break;
    case 'U':
    case 'u':
      nm |= FREQ_MUTE;
      break;
    case 'E':
    case 'e':
      DOCHECK_CONTEXT(mech_context(mech),
                      !(mech_radio_capabilities(mech) & RADIO_RELAY),
                      "This unit is unable to relay.");
      nm |= FREQ_RELAY;
      break;
    case 'S':
    case 's':
      DOCHECK_CONTEXT(mech_context(mech),
                      !(mech_radio_capabilities(mech) & RADIO_SCAN),
                      "This unit is unable to scan.");
      nm |= FREQ_SCAN;
      break;
    default:
      for (i = 0; radio_colorstr[i]; i++)
        if (*buffer == radio_colorstr[i]) {
          nm = nm % FREQ_REST + FREQ_REST * (i + 1);
          break;
        }
      if (!radio_colorstr[i])
        buffer = nullptr;
      break;
    }
    if (buffer)
      buffer++;
  }
  DOCHECK_CONTEXT(mech_context(mech), !(nm & FREQ_DIGITAL) && (nm & FREQ_RELAY),
                  "Error: Need digital transfer for relay to work.");
  DOCHECK_CONTEXT(mech_context(mech), !(nm & FREQ_DIGITAL) && (nm & FREQ_INFO),
                  "Error: Need digital transfer for transfer info to work.");
  mech_radio_mode_set(mech, chn, nm);
  i = 0;

  if (nm & FREQ_INFO)
    buf[i++] = 'I';
  if (nm & FREQ_MUTE)
    buf[i++] = 'U';
  if (nm & FREQ_RELAY)
    buf[i++] = 'E';
  if (nm & FREQ_SCAN)
    buf[i++] = 'S';
  if (!i)
    buf[i++] = '-';
  if (nm / FREQ_REST) {
    snprintf(buf + i, sizeof(buf) - i, "/color:%c",
             radio_colorstr[nm / FREQ_REST - 1]);
    i = strlen(buf);
  }
  buf[i] = 0;
  notify_printf(evaluation, player,
                "Channel %c <send> mode set to %s (flags:%s).", 'A' + chn,
                nm & FREQ_DIGITAL ? "digital" : "analog", buf);
}

void mech_list_freqs(DbRef player, void *data, char *buffer) {
  int i;
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));

  /* UH, this is code that _pretends_ it works :-) */
  notify(evaluation, player, "# -- Mode -- Frequency -- Comtitle");
  for (i = 0; i < mech_radio_channel_count(mech); i++) {
    int mode = mech_radio_mode(mech, i);
    notify_printf(evaluation, player, "%c    %c%c%c%c    %-9d    %s", 'A' + i,
                  mode & FREQ_DIGITAL ? 'D' : 'A',
                  mode & FREQ_RELAY ? 'R' : '-', mode & FREQ_MUTE ? 'M' : '-',
                  mode & FREQ_SCAN    ? 'S'
                  : mode >= FREQ_REST ? radio_colorstr[mode / FREQ_REST - 1]
                  : mode & FREQ_INFO  ? 'I'
                                      : '-',
                  mech_radio_frequency(mech, i), mech_radio_title(mech, i));
  }
}

void mech_sendchannel(DbRef player, void *data, char *buffer) {
  /* Basically, this is sorta routine 'sendchannel <letter>=message' code */
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  int fail = 0;
  int argc;
  int chn = 0;
  char *args[3];
  int i;

  cch(MECH_USUALS);
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_is_destroyed(mech) || !mech_radio_range(mech),
                  "Your communication gear is inoperative.");
  DOCHECK_CONTEXT(mech_context(mech), mech_event_count(mech, EVENT_UNSTUN_CREW),
                  "You are too stunned to use the radio!");
  if ((argc = proper_parseattributes(buffer, args, 3)) != 3)
    fail = 1;
  if (!fail && strlen(args[0]) > 1)
    fail = 1;
  if (!fail && args[0][0] >= 'a' && args[0][0] <= 'z')
    chn = args[0][0] - 'a';
  if (!fail && args[0][0] >= 'A' && args[0][0] <= 'Z')
    chn = args[0][0] - 'Z';
  if (!fail && (chn >= mech_radio_channel_count(mech) || chn < 0))
    fail = 1;
  if (!fail)
    for (i = 0; args[2][i]; i++) {
      if ((BOUNDED(32, args[2][i], 255)) != args[2][i]) {
        notify(evaluation, player,
               "Invalid: No control characters in radio messages, please.");
        for (i = 0; i < 3; i++) {
          if (args[i])
            free(args[i]);
        }
        return;
      }
    }

  if (fail) {
    notify(evaluation, player,
           "Invalid format! Usage: sendchannel <letter>=<string>");
    for (i = 0; i < 3; i++) {
      if (args[i])
        free(args[i]);
    }
    return;
  }

  if (mech_radio_frequency(mech, chn) == 0 &&
      is_in_character(btech_context_database(mech_context(mech)),
                      mech_map_dbref(mech))) {
    send_channel(
        evaluation, "ZeroFrequencies",
        "Player #%d (%s) in mech #%d (channel %c) "
        "on map #%d 0-freqs \"%s\"",
        (int)player,
        game_object_name(btech_context_database(mech_context(mech)), player),
        (int)mech_dbref(mech), chn + 'A', (int)mech_map_dbref(mech), args[2]);
  }

  sendchannelstuff(mech, chn, args[2]);
  for (i = 0; i < 3; i++) {
    if (args[i])
      free(args[i]);
  }
  explode_mines(mech, mech_radio_frequency(mech, chn));
}
