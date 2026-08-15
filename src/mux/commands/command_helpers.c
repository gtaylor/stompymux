/* command_helpers.c - Shared helpers for native command implementations. */

#include "mux/commands/command_helpers.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/commands/command_context.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "mux/world/match.h"

char *trim_space_sep(char *string, char separator) {
  if (*string == '\0' || separator != ' ')
    return string;

  const size_t LENGTH = strlen(string);
  size_t start = 0;
  size_t end = LENGTH;

  while (start < LENGTH && *(const char *)checked_storage_at_const(
                               string, LENGTH + 1, sizeof(char), start) == ' ')
    start++;
  while (end > start && *(const char *)checked_storage_at_const(
                            string, LENGTH + 1, sizeof(char), end - 1) == ' ')
    end--;
  *(char *)checked_storage_at(string, LENGTH + 1, sizeof(char), end) = '\0';
  return checked_mutable_string_suffix(string, start);
}

char *next_token(char *string, char separator) {
  const size_t LENGTH = strlen(string);
  size_t offset = 0;

  while (offset < LENGTH &&
         *(const char *)checked_storage_at_const(
             string, LENGTH + 1, sizeof(char), offset) != separator)
    offset++;
  if (offset == LENGTH)
    return nullptr;
  offset++;
  if (separator == ' ')
    while (offset < LENGTH &&
           *(const char *)checked_storage_at_const(
               string, LENGTH + 1, sizeof(char), offset) == separator)
      offset++;
  return checked_mutable_string_suffix(string, offset);
}

DbRef match_thing(MatchContext *match, DbRef player, char *name) {
  init_match(match, player, name, OBJECT_TYPE_NOTYPE);
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

static const UptimeUnit UPTIME_UNITS[] = {
    {60 * 60 * 24 * 30 * 12, "year"},
    {60 * 60 * 24 * 30, "month"},
    {60 * 60 * 24, "day"},
    {60 * 60, "hour"},
    {60, "minute"},
    {1, "second"},
};

static const UptimeUnit *uptime_unit_at(size_t index) {
  return checked_storage_at_const(
      UPTIME_UNITS, sizeof(UPTIME_UNITS) / sizeof(UPTIME_UNITS[0]),
      sizeof(*UPTIME_UNITS), index);
}

static int *uptime_value_slot(int *values, size_t count, size_t index) {
  return checked_storage_at(values, count, sizeof(*values), index);
}

char *get_uptime_to_string(int uptime) {
  char *result = alloc_sbuf("get_uptime_to_string");
  if (uptime <= 0) {
    (void)string_copy_bounded(result, SBUF_SIZE, "#-1 INVALID VALUE");
    return result;
  }

  int remaining = uptime;
  int populated = 0;
  constexpr size_t UNIT_COUNT = sizeof(UPTIME_UNITS) / sizeof(UPTIME_UNITS[0]);
  int values[UNIT_COUNT] = {0};
  for (size_t i = 0; i < UNIT_COUNT; i++) {
    const UptimeUnit *unit = uptime_unit_at(i);
    int *value = uptime_value_slot(values, UNIT_COUNT, i);

    *value = remaining / unit->multiplier;
    remaining %= unit->multiplier;
    if (*value > 0)
      populated++;
  }

  result[0] = '\0';
  for (size_t i = 0; i < UNIT_COUNT; i++) {
    const UptimeUnit *unit = uptime_unit_at(i);
    const int VALUE = *uptime_value_slot(values, UNIT_COUNT, i);

    if (VALUE == 0)
      continue;
    populated--;
    size_t used = strlen(result);

    (void)snprintf(
        checked_storage_region(result, SBUF_SIZE, used, SBUF_SIZE - used),
        SBUF_SIZE - used, "%d %s%s", VALUE, unit->name, VALUE == 1 ? "" : "s");
    if (populated > 1)
      (void)string_append_bounded(result, SBUF_SIZE, ", ");
    else if (populated == 1)
      (void)string_append_bounded(result, SBUF_SIZE, " and ");
  }
  return result;
}

int xlate(char *argument) {
  int value;

  if (*argument == '#') {
    argument = checked_mutable_string_suffix(argument, 1);
    if (*argument == '-')
      return 0;
    return parse_int_checked(argument, &value) ? value : 0;
  }

  argument = trim_space_sep(argument, ' ');
  if (!*argument)
    return 0;
  return parse_int_checked(argument, &value) ? value : 1;
}
