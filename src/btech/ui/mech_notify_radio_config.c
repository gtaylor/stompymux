#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btech_text_builder.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_electronics_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_radio_api.h"
#include "mech_radio_render_internal.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_status_types.h"
#include "mine_api.h"
#include "mux/communication/comsys.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"

#include "mux/support/formatting.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct RadioCommandCursor {
  char *text;
  size_t length;
  size_t position;
} RadioCommandCursor;

typedef struct RadioCommandArguments {
  char *items[3];
} RadioCommandArguments;

static RadioCommandCursor radio_command_cursor(char *text) {
  return (RadioCommandCursor){
      .text = text,
      .length = text != nullptr ? strlen(text) : 0,
  };
}

static bool ascii_is_space(char value) {
  return value == ' ' || value == '\t' || value == '\n' || value == '\r' ||
         value == '\f' || value == '\v';
}

static char radio_cursor_current(const RadioCommandCursor *cursor) {
  return cursor->position < cursor->length
             ? *checked_string_suffix(cursor->text, cursor->position)
             : '\0';
}

static void radio_cursor_advance(RadioCommandCursor *cursor) {
  if (cursor->position < cursor->length)
    cursor->position++;
}

static void radio_cursor_skip_spaces(RadioCommandCursor *cursor) {
  while (ascii_is_space(radio_cursor_current(cursor)))
    radio_cursor_advance(cursor);
}

static char *radio_cursor_remaining(const RadioCommandCursor *cursor) {
  return cursor->text != nullptr
             ? checked_mutable_string_suffix(cursor->text, cursor->position)
             : nullptr;
}

static char *radio_command_argument(const RadioCommandArguments *arguments,
                                    int index) {
  if (index < 0)
    abort();
  return *(char *const *)checked_storage_at_const(
      arguments->items, 3, sizeof(*arguments->items), (size_t)index);
}

static void radio_command_arguments_destroy(RadioCommandArguments *arguments) {
  for (int index = 0; index < 3; index++)
    free(radio_command_argument(arguments, index));
}

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
  RadioCommandCursor input = radio_command_cursor(buffer);

  /* UH, this is code that _pretends_ it works :-) */
  radio_cursor_skip_spaces(&input);
  if (radio_cursor_current(&input) == '\0') {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid input!");
    return;
  }
  chn = ascii_to_upper(radio_cursor_current(&input)) - 'A';
  if (chn < 0 || chn >= mech_radio_channel_count(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid channel-letter!");
    return;
  }
  radio_cursor_advance(&input);
  radio_cursor_skip_spaces(&input);
  if (radio_cursor_current(&input) == '\0') {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid input!");
    return;
  }
  if (radio_cursor_current(&input) != '=') {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Missing =!");
    return;
  }
  radio_cursor_advance(&input);
  radio_cursor_skip_spaces(&input);
  if (radio_cursor_current(&input) == '\0') {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid input!");
    return;
  }
  char *frequency_text = radio_cursor_remaining(&input);
  freq = atoi(frequency_text);
  if (!freq && strcmp(frequency_text, "0")) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid frequency!");
    return;
  }
  if (freq < 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Are you trying to kid me?");
    return;
  }
  if (freq > 999999) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid frequency - range is from 0 to 999999.");
    return;
  }
  notify_printf(evaluation, player, "Channel %c set to %d.", 'A' + chn, freq);
  mech_radio_frequency_set(mech, chn, freq);

  /* Code added from Exile to check for possible cheat freq acquring.
   * When a player sets a freq, it loops through all the mechs on the
   * map that do not belong to the same team and checks their freqs
   * against the one set. If it matches it emits message
   */
  if (freq > 0 && map) {
    for (i = 0; i < battle_map_unit_count(map); i++) {
      const DbRef candidate = battle_map_unit_dbref(map, i);
      if (!(t = btech_context_find_object(mech_context(mech), candidate)))
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
  RadioCommandCursor input = radio_command_cursor(buffer);

  radio_cursor_skip_spaces(&input);
  if (radio_cursor_current(&input) == '\0') {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid input!");
    return;
  }
  chn = ascii_to_upper(radio_cursor_current(&input)) - 'A';
  if (chn < 0 || chn >= mech_radio_channel_count(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid channel-letter!");
    return;
  }
  radio_cursor_advance(&input);
  radio_cursor_skip_spaces(&input);
  if (radio_cursor_current(&input) == '\0') {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid input!");
    return;
  }
  if (radio_cursor_current(&input) != '=') {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Missing =!");
    return;
  }
  radio_cursor_advance(&input);
  radio_cursor_skip_spaces(&input);
  if (radio_cursor_current(&input) == '\0') {
    mech_radio_title_set(mech, chn, "");
    notify_printf(evaluation, player, "Channel %c title cleared.", 'A' + chn);
    return;
  }
  char *title = radio_cursor_remaining(&input);
  mech_radio_title_set(mech, chn, title);
  notify_printf(evaluation, player, "Channel %c title set to set to %s.",
                'A' + chn, title);
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

static const char *radio_color_style(int index) {
  if (index < 0)
    abort();
  return *(const char *const *)checked_storage_at_const(
      radio_color_styles,
      sizeof(radio_color_styles) / sizeof(*radio_color_styles),
      sizeof(*radio_color_styles), (size_t)index);
}

static const typeof(*OBSERVER_TEAM_COLORS) *observer_team_color(int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(OBSERVER_TEAM_COLORS,
                                  sizeof(OBSERVER_TEAM_COLORS) /
                                      sizeof(*OBSERVER_TEAM_COLORS),
                                  sizeof(*OBSERVER_TEAM_COLORS), (size_t)index);
}

static char radio_color_character(int index) {
  if (index < 0)
    abort();
  return *checked_string_suffix(radio_colorstr, (size_t)index);
}

void radio_color_code(char buffer[static 32], Mech *m, int i, int obs,
                      int team) {
  int t = mech_radio_mode(m, i) / FREQ_REST;
  int ii;

  *(char *)checked_storage_at(buffer, 32, sizeof(char), 0) = '\0';
  if (!obs) {
    if (!t)
      return;
    snprintf(buffer, 32, "%s", radio_color_style(t - 1));
  } else {
    if (team > 15)
      team = team % 15;
    for (ii = 0; ii < 15; ii++) {
      const typeof(*OBSERVER_TEAM_COLORS) *color = observer_team_color(ii);
      if (team == color->team)
        snprintf(buffer, 32, "%s", color->color_code);
    }
  }
}

void mech_set_channelmode(DbRef player, void *data, char *buffer) {
  int chn = -1, nm = 0, i;
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  char buf[SBUF_SIZE] = {0};
  RadioCommandCursor input = radio_command_cursor(buffer);

  radio_cursor_skip_spaces(&input);
  if (radio_cursor_current(&input) == '\0') {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid input!");
    return;
  }
  chn = ascii_to_upper(radio_cursor_current(&input)) - 'A';
  if (chn < 0 || chn >= mech_radio_channel_count(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid channel-letter!");
    return;
  }
  radio_cursor_advance(&input);
  radio_cursor_skip_spaces(&input);
  if (radio_cursor_current(&input) == '\0') {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid input!");
    return;
  }
  if (radio_cursor_current(&input) != '=') {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Missing =!");
    return;
  }
  radio_cursor_advance(&input);
  radio_cursor_skip_spaces(&input);
  if (radio_cursor_current(&input) == '\0') {
    mech_radio_mode_set(mech, chn, 0);
    notify_printf(evaluation, player, "Channel %c <send> mode set to analog.",
                  'A' + chn);
    return;
  }
  while (radio_cursor_current(&input) != '\0') {
    const char mode_character = radio_cursor_current(&input);
    switch (mode_character) {
    case 'D':
    case 'd':
      if (mech_radio_capabilities(mech) & RADIO_NODIGITAL) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Your radio can't handle digital frequencies!");
        return;
      }
      nm |= FREQ_DIGITAL;
      break;
    case 'I':
    case 'i':
      if (!(mech_radio_capabilities(mech) & RADIO_INFO)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "This unit is unable to use info functionality.");
        return;
      }
      nm |= FREQ_INFO;
      break;
    case 'U':
    case 'u':
      nm |= FREQ_MUTE;
      break;
    case 'E':
    case 'e':
      if (!(mech_radio_capabilities(mech) & RADIO_RELAY)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "This unit is unable to relay.");
        return;
      }
      nm |= FREQ_RELAY;
      break;
    case 'S':
    case 's':
      if (!(mech_radio_capabilities(mech) & RADIO_SCAN)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "This unit is unable to scan.");
        return;
      }
      nm |= FREQ_SCAN;
      break;
    default:
      for (i = 0; radio_color_character(i); i++)
        if (mode_character == radio_color_character(i)) {
          nm = nm % FREQ_REST + FREQ_REST * (i + 1);
          break;
        }
      if (!radio_color_character(i))
        input.position = input.length;
      break;
    }
    radio_cursor_advance(&input);
  }
  if (!(nm & FREQ_DIGITAL) && (nm & FREQ_RELAY)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Error: Need digital transfer for relay to work.");
    return;
  }
  if (!(nm & FREQ_DIGITAL) && (nm & FREQ_INFO)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Error: Need digital transfer for transfer info to work.");
    return;
  }
  mech_radio_mode_set(mech, chn, nm);
  BtechTextBuilder flags;
  btech_text_builder_initialize(&flags, buf, sizeof(buf));

  if (nm & FREQ_INFO)
    btech_text_builder_append_character(&flags, 'I');
  if (nm & FREQ_MUTE)
    btech_text_builder_append_character(&flags, 'U');
  if (nm & FREQ_RELAY)
    btech_text_builder_append_character(&flags, 'E');
  if (nm & FREQ_SCAN)
    btech_text_builder_append_character(&flags, 'S');
  if (flags.length == 0)
    btech_text_builder_append_character(&flags, '-');
  if (nm / FREQ_REST) {
    btech_text_builder_append_format(&flags, "/color:%c",
                                     radio_color_character(nm / FREQ_REST - 1));
  }
  notify_printf(evaluation, player,
                "Channel %c <send> mode set to %s (flags:%s).", 'A' + chn,
                nm & FREQ_DIGITAL ? "digital" : "analog", buf);
}

void mech_list_freqs(DbRef player, void *data, char *buffer) {
  int i;
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));

  /* UH, this is code that _pretends_ it works :-) */
  mecha_notify(evaluation, player, "# -- Mode -- Frequency -- Comtitle");
  for (i = 0; i < mech_radio_channel_count(mech); i++) {
    int mode = mech_radio_mode(mech, i);
    notify_printf(evaluation, player, "%c    %c%c%c%c    %-9d    %s", 'A' + i,
                  mode & FREQ_DIGITAL ? 'D' : 'A',
                  mode & FREQ_RELAY ? 'R' : '-', mode & FREQ_MUTE ? 'M' : '-',
                  mode & FREQ_SCAN ? 'S'
                  : mode >= FREQ_REST
                      ? radio_color_character(mode / FREQ_REST - 1)
                  : mode & FREQ_INFO ? 'I'
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
  RadioCommandArguments arguments = {0};

  if (!common_checks(player, mech, MECH_USUALS))
    return;
  if (mech_is_destroyed(mech) || !mech_radio_range(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your communication gear is inoperative.");
    return;
  }
  if (mech_event_count(mech, EVENT_UNSTUN_CREW)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are too stunned to use the radio!");
    return;
  }
  if ((argc = proper_parseattributes(buffer, arguments.items, 3)) != 3)
    fail = 1;
  char *channel_text = radio_command_argument(&arguments, 0);
  char *message = radio_command_argument(&arguments, 2);
  if (!fail && strlen(channel_text) > 1)
    fail = 1;
  const char channel_character = !fail ? *channel_text : '\0';
  if (!fail && channel_character >= 'a' && channel_character <= 'z')
    chn = channel_character - 'a';
  if (!fail && channel_character >= 'A' && channel_character <= 'Z')
    chn = channel_character - 'Z';
  if (!fail && (chn >= mech_radio_channel_count(mech) || chn < 0))
    fail = 1;
  if (!fail) {
    const size_t message_length = strlen(message);
    for (size_t index = 0; index < message_length; index++) {
      const char character = *checked_string_suffix(message, index);
      if ((BOUNDED(32, character, 255)) != character) {
        mecha_notify(
            evaluation, player,
            "Invalid: No control characters in radio messages, please.");
        radio_command_arguments_destroy(&arguments);
        return;
      }
    }
  }

  if (fail) {
    mecha_notify(evaluation, player,
                 "Invalid format! Usage: sendchannel <letter>=<string>");
    radio_command_arguments_destroy(&arguments);
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
        (int)mech_dbref(mech), chn + 'A', (int)mech_map_dbref(mech), message);
  }

  sendchannelstuff(mech, chn, message);
  radio_command_arguments_destroy(&arguments);
  mine_command_detonate(mech, mech_radio_frequency(mech, chn));
}
