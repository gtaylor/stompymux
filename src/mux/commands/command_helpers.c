/* command_helpers.c - Shared helpers for native command implementations. */

#include "mux/commands/command_helpers.h"

#include <stdlib.h>
#include <string.h>

#include "mux/database/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/validation.h"
#include "mux/world/match.h"

char *trim_space_sep(char *string, char separator) {
  if (*string == '\0' || separator != ' ')
    return string;

  while (*string == ' ')
    string++;

  char *end = string + strlen(string);
  while (end > string && end[-1] == ' ')
    end--;
  *end = '\0';
  return string;
}

char *next_token(char *string, char separator) {
  while (*string && *string != separator)
    string++;
  if (!*string)
    return nullptr;
  string++;
  if (separator == ' ')
    while (*string == separator)
      string++;
  return string;
}

DbRef match_thing(MatchContext *match, DbRef player, char *name) {
  init_match(match, player, name, NOTYPE);
  match_everything(match, 0);
  return noisy_match_result(match);
}

bool argument_count_in_range(const char *name, int count, int minimum,
                             int maximum, char *result, char **result_cursor) {
  if (count >= minimum && count <= maximum)
    return true;

  if (maximum == minimum + 1) {
    safe_tprintf_str(result, result_cursor, "#-1 %s EXPECTS %d OR %d ARGUMENTS",
                     name, minimum, maximum);
  } else {
    safe_tprintf_str(result, result_cursor,
                     "#-1 %s EXPECTS BETWEEN %d AND %d ARGUMENTS", name,
                     minimum, maximum);
  }
  return false;
}

typedef struct UptimeUnit {
  int multiplier;
  const char *name;
} UptimeUnit;

static const UptimeUnit uptime_units[] = {
    {60 * 60 * 24 * 30 * 12, "year"},
    {60 * 60 * 24 * 30, "month"},
    {60 * 60 * 24, "day"},
    {60 * 60, "hour"},
    {60, "minute"},
    {1, "second"},
};

char *get_uptime_to_string(int uptime) {
  char *result = alloc_sbuf("get_uptime_to_string");
  if (uptime <= 0) {
    strlcpy(result, "#-1 INVALID VALUE", SBUF_SIZE);
    return result;
  }

  int remaining = uptime;
  int populated = 0;
  int values[sizeof(uptime_units) / sizeof(uptime_units[0])] = {0};
  for (size_t i = 0; i < sizeof(uptime_units) / sizeof(uptime_units[0]); i++) {
    values[i] = remaining / uptime_units[i].multiplier;
    remaining %= uptime_units[i].multiplier;
    if (values[i] > 0)
      populated++;
  }

  result[0] = '\0';
  for (size_t i = 0; i < sizeof(uptime_units) / sizeof(uptime_units[0]); i++) {
    if (values[i] == 0)
      continue;
    populated--;
    snprintf(result + strlen(result), SBUF_SIZE - strlen(result), "%d %s%s",
             values[i], uptime_units[i].name, values[i] == 1 ? "" : "s");
    if (populated > 1)
      strlcat(result, ", ", SBUF_SIZE);
    else if (populated == 1)
      strlcat(result, " and ", SBUF_SIZE);
  }
  return result;
}

int xlate(char *argument) {
  if (argument[0] == '#') {
    argument++;
    if (argument[0] == '-')
      return 0;
    return is_integer(argument) ? atoi(argument) : 0;
  }

  argument = trim_space_sep(argument, ' ');
  if (!*argument)
    return 0;
  return is_integer(argument) ? atoi(argument) : 1;
}
