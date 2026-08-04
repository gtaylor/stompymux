#include "mech_notify_internal.h"

void mech_set_channelfreq(DbRef player, void *data, char *buffer) {
  int chn = -1;
  int freq;
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  Mech *t;
  // map pointer is NULL if in a carrier. Careful.
  BattleMap *map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  int i, j;

  /* UH, this is code that _pretends_ it works :-) */
  skipws(buffer);
  DOCHECK_CONTEXT(mech->xcode.context, !*buffer, "Invalid input!");
  chn = toupper(*buffer) - 'A';
  DOCHECK_CONTEXT(mech->xcode.context, chn < 0 || chn >= MFreqs(mech),
                  "Invalid channel-letter!");
  buffer++;
  skipws(buffer);
  DOCHECK_CONTEXT(mech->xcode.context, !*buffer, "Invalid input!");
  DOCHECK_CONTEXT(mech->xcode.context, *buffer != '=', "Missing =!");
  buffer++;
  skipws(buffer);
  DOCHECK_CONTEXT(mech->xcode.context, !*buffer, "Invalid input!");
  freq = atoi(buffer);
  DOCHECK_CONTEXT(mech->xcode.context, !freq && strcmp(buffer, "0"),
                  "Invalid frequency!");
  DOCHECK_CONTEXT(mech->xcode.context, freq < 0, "Are you trying to kid me?");
  DOCHECK_CONTEXT(mech->xcode.context, freq > 999999,
                  "Invalid frequency - range is from 0 to 999999.");
  notify_printf(evaluation, player, "Channel %c set to %d.", 'A' + chn, freq);
  mech->freq[chn] = freq;

  /* Code added from Exile to check for possible cheat freq acquring.
   * When a player sets a freq, it loops through all the mechs on the
   * map that do not belong to the same team and checks their freqs
   * against the one set. If it matches it emits message
   */
  if (freq > 0 && map) {
    for (i = 0; i < map->first_free; i++) {
      if (!(t = btech_context_find_object(mech->xcode.context,
                                          map->mechsOnMap[i])))
        continue;
      if (t == mech)
        continue;
      if (MechTeam(t) == MechTeam(mech))
        continue;
      for (j = 0; j < MFreqs(t); j++) {
        if (t->freq[j] == freq && !(t->freqmodes[j] & FREQ_SCAN))
          btech_channel_send(
              mech->xcode.context, BTECH_CHANNEL_MECH_FREQS, "%s",
              tprintf("ALERT: Possible abuse by #%ld (Team %d)"
                      " setting freq %d matching #%ld (Team %d)!",
                      mech->mynum, MechTeam(mech), freq, t->mynum,
                      MechTeam(t)));
      }
    }
  }
}

void mech_set_channeltitle(DbRef player, void *data, char *buffer) {
  int chn = -1;
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);

  skipws(buffer);
  DOCHECK_CONTEXT(mech->xcode.context, !*buffer, "Invalid input!");
  chn = toupper(*buffer) - 'A';
  DOCHECK_CONTEXT(mech->xcode.context, chn < 0 || chn >= MFreqs(mech),
                  "Invalid channel-letter!");
  buffer++;
  skipws(buffer);
  DOCHECK_CONTEXT(mech->xcode.context, !*buffer, "Invalid input!");
  DOCHECK_CONTEXT(mech->xcode.context, *buffer != '=', "Missing =!");
  buffer++;
  skipws(buffer);
  if (!*buffer) {
    mech->chantitle[chn][0] = 0;
    notify_printf(evaluation, player, "Channel %c title cleared.", 'A' + chn);
    return;
  }
  strncpy(mech->chantitle[chn], buffer, CHTITLELEN);
  mech->chantitle[chn][CHTITLELEN] = 0;
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
  int t = m->freqmodes[i] / FREQ_REST;
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
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  char buf[SBUF_SIZE] = {0};

  skipws(buffer);
  DOCHECK_CONTEXT(mech->xcode.context, !*buffer, "Invalid input!");
  chn = toupper(*buffer) - 'A';
  DOCHECK_CONTEXT(mech->xcode.context, chn < 0 || chn >= MFreqs(mech),
                  "Invalid channel-letter!");
  buffer++;
  skipws(buffer);
  DOCHECK_CONTEXT(mech->xcode.context, !*buffer, "Invalid input!");
  DOCHECK_CONTEXT(mech->xcode.context, *buffer != '=', "Missing =!");
  buffer++;
  skipws(buffer);
  if (!buffer || !*buffer) {
    mech->freqmodes[chn] = 0;
    notify_printf(evaluation, player, "Channel %c <send> mode set to analog.",
                  'A' + chn);
    return;
  }
  while (buffer && *buffer) {
    switch (*buffer) {
    case 'D':
    case 'd':
      DOCHECK_CONTEXT(mech->xcode.context,
                      MechRadioInfo(mech) & RADIO_NODIGITAL,
                      "Your radio can't handle digital frequencies!");
      nm |= FREQ_DIGITAL;
      break;
    case 'I':
    case 'i':
      DOCHECK_CONTEXT(mech->xcode.context, !(MechRadioInfo(mech) & RADIO_INFO),
                      "This unit is unable to use info functionality.");
      nm |= FREQ_INFO;
      break;
    case 'U':
    case 'u':
      nm |= FREQ_MUTE;
      break;
    case 'E':
    case 'e':
      DOCHECK_CONTEXT(mech->xcode.context, !(MechRadioInfo(mech) & RADIO_RELAY),
                      "This unit is unable to relay.");
      nm |= FREQ_RELAY;
      break;
    case 'S':
    case 's':
      DOCHECK_CONTEXT(mech->xcode.context, !(MechRadioInfo(mech) & RADIO_SCAN),
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
        buffer = NULL;
      break;
    }
    if (buffer)
      buffer++;
  }
  DOCHECK_CONTEXT(mech->xcode.context,
                  !(nm & FREQ_DIGITAL) && (nm & FREQ_RELAY),
                  "Error: Need digital transfer for relay to work.");
  DOCHECK_CONTEXT(mech->xcode.context, !(nm & FREQ_DIGITAL) && (nm & FREQ_INFO),
                  "Error: Need digital transfer for transfer info to work.");
  mech->freqmodes[chn] = nm;
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
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);

  /* UH, this is code that _pretends_ it works :-) */
  notify(evaluation, player, "# -- Mode -- Frequency -- Comtitle");
  for (i = 0; i < MFreqs(mech); i++)
    notify_printf(evaluation, player, "%c    %c%c%c%c    %-9d    %s", 'A' + i,
                  mech->freqmodes[i] & FREQ_DIGITAL ? 'D' : 'A',
                  mech->freqmodes[i] & FREQ_RELAY ? 'R' : '-',
                  mech->freqmodes[i] & FREQ_MUTE ? 'M' : '-',
                  mech->freqmodes[i] & FREQ_SCAN ? 'S'
                  : mech->freqmodes[i] >= FREQ_REST
                      ? radio_colorstr[mech->freqmodes[i] / FREQ_REST - 1]
                  : mech->freqmodes[i] & FREQ_INFO ? 'I'
                                                   : '-',
                  mech->freq[i], mech->chantitle[i]);
}

void mech_sendchannel(DbRef player, void *data, char *buffer) {
  /* Basically, this is sorta routine 'sendchannel <letter>=message' code */
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  int fail = 0;
  int argc;
  int chn = 0;
  char *args[3];
  int i;

  cch(MECH_USUALS);
  DOCHECK_CONTEXT(mech->xcode.context, Destroyed(mech) || !MechRadioRange(mech),
                  "Your communication gear is inoperative.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_event_count(mech, EVENT_UNSTUN_CREW),
                  "You are too stunned to use the radio!");
  if ((argc = proper_parseattributes(buffer, args, 3)) != 3)
    fail = 1;
  if (!fail && strlen(args[0]) > 1)
    fail = 1;
  if (!fail && args[0][0] >= 'a' && args[0][0] <= 'z')
    chn = args[0][0] - 'a';
  if (!fail && args[0][0] >= 'A' && args[0][0] <= 'Z')
    chn = args[0][0] - 'Z';
  if (!fail && (chn >= MFreqs(mech) || chn < 0))
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

  if (mech->freq[chn] == 0 &&
      is_in_character(mech->xcode.context->database, mech->mapindex)) {
    send_channel(evaluation, "ZeroFrequencies",
                 "Player #%d (%s) in mech #%d (channel %c) "
                 "on map #%d 0-freqs \"%s\"",
                 player,
                 game_object_name(mech->xcode.context->database, player),
                 mech->mynum, chn + 'A', mech->mapindex, args[2]);
  }

  sendchannelstuff(mech, chn, args[2]);
  for (i = 0; i < 3; i++) {
    if (args[i])
      free(args[i]);
  }
  explode_mines(mech, mech->freq[chn]);
}
