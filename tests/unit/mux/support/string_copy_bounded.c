#include "mux/server/platform.h"

#include <stdio.h>
#include <string.h>

static bool exact_fit_terminates(void) {
  char destination[5] = {0};

  return string_copy_bounded(destination, sizeof(destination), "test") &&
         strcmp(destination, "test") == 0;
}

static bool truncation_is_reported(void) {
  char destination[4] = {0};

  return !string_copy_bounded(destination, sizeof(destination), "test") &&
         strcmp(destination, "tes") == 0;
}

static bool zero_size_writes_nothing(void) {
  char destination = 'x';

  return !string_copy_bounded(&destination, 0, "test") && destination == 'x';
}

static bool one_byte_destination_is_terminated(void) {
  char destination = 'x';

  return !string_copy_bounded(&destination, 1, "test") && destination == '\0';
}

static bool empty_source_succeeds(void) {
  char destination[2] = {'x', 'y'};

  return string_copy_bounded(destination, sizeof(destination), "") &&
         destination[0] == '\0';
}

static bool maximum_player_name_fits(void) {
  char destination[31] = {0};
  const char *source = "abcdefghijklmnopqrstuvwxyz1234";

  return strlen(source) == 30 &&
         string_copy_bounded(destination, sizeof(destination), source) &&
         strcmp(destination, source) == 0;
}

static bool short_copy_does_not_pad_tail(void) {
  unsigned char destination[8];
  const unsigned char EXPECTED[8] = {'h',  'i',  '\0', 0xAA,
                                     0xAA, 0xAA, 0xAA, 0xAA};
  memset(destination, 0xAA, sizeof(destination));

  return string_copy_bounded((char *)destination, sizeof(destination), "hi") &&
         memcmp(destination, EXPECTED, sizeof(destination)) == 0;
}

int main(void) {
  int failures = 0;

  if (!exact_fit_terminates()) {
    (void)fprintf(stderr, "exact-fit copy failed or was not terminated\n");
    failures++;
  }
  if (!truncation_is_reported()) {
    (void)fprintf(stderr, "truncated copy was not terminated and reported\n");
    failures++;
  }
  if (!zero_size_writes_nothing()) {
    (void)fprintf(stderr, "zero-sized copy wrote output or reported success\n");
    failures++;
  }
  if (!one_byte_destination_is_terminated()) {
    (void)fprintf(stderr, "one-byte copy did not produce an empty string\n");
    failures++;
  }
  if (!empty_source_succeeds()) {
    (void)fprintf(stderr, "empty source did not copy successfully\n");
    failures++;
  }
  if (!maximum_player_name_fits()) {
    (void)fprintf(stderr, "maximum-length player name did not fit\n");
    failures++;
  }
  if (!short_copy_does_not_pad_tail()) {
    (void)fprintf(stderr, "short copy overwrote bytes beyond its terminator\n");
    failures++;
  }
  return failures != 0;
}
