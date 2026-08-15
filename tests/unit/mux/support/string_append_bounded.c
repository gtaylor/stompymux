#include "mux/server/platform.h"

#include <stdio.h>
#include <string.h>

static bool empty_destination_appends(void) {
  char destination[5] = {0};

  return string_append_bounded(destination, sizeof(destination), "test") &&
         strcmp(destination, "test") == 0;
}

static bool exact_fit_terminates(void) {
  char destination[6] = "hi";

  return string_append_bounded(destination, sizeof(destination), "123") &&
         strcmp(destination, "hi123") == 0;
}

static bool truncation_is_reported(void) {
  char destination[5] = "hi";

  return !string_append_bounded(destination, sizeof(destination), "123") &&
         strcmp(destination, "hi12") == 0;
}

static bool zero_size_writes_nothing(void) {
  char destination = 'x';

  return !string_append_bounded(&destination, 0, "test") && destination == 'x';
}

static bool one_byte_destination_stays_terminated(void) {
  char destination = '\0';

  return !string_append_bounded(&destination, 1, "test") &&
         destination == '\0';
}

static bool full_destination_writes_nothing(void) {
  char destination[4] = "abc";

  return !string_append_bounded(destination, sizeof(destination), "x") &&
         strcmp(destination, "abc") == 0;
}

static bool unterminated_destination_writes_nothing(void) {
  char destination[4] = {'a', 'b', 'c', 'd'};
  const char expected[4] = {'a', 'b', 'c', 'd'};

  return !string_append_bounded(destination, sizeof(destination), "x") &&
         memcmp(destination, expected, sizeof(destination)) == 0;
}

static bool empty_source_succeeds(void) {
  char destination[4] = "hi";

  return string_append_bounded(destination, sizeof(destination), "") &&
         strcmp(destination, "hi") == 0;
}

static bool append_does_not_write_past_terminator(void) {
  unsigned char destination[8];
  const unsigned char EXPECTED[8] = {'h',  'i',  '!',  '\0',
                                     0xAA, 0xAA, 0xAA, 0xAA};
  memset(destination, 0xAA, sizeof(destination));
  memcpy(destination, "hi", 3);

  return string_append_bounded((char *)destination, sizeof(destination), "!") &&
         memcmp(destination, EXPECTED, sizeof(destination)) == 0;
}

int main(void) {
  int failures = 0;

  if (!empty_destination_appends()) {
    (void)fprintf(stderr, "empty destination append failed\n");
    failures++;
  }
  if (!exact_fit_terminates()) {
    (void)fprintf(stderr, "exact-fit append failed or was not terminated\n");
    failures++;
  }
  if (!truncation_is_reported()) {
    (void)fprintf(stderr, "truncated append was not reported\n");
    failures++;
  }
  if (!zero_size_writes_nothing()) {
    (void)fprintf(stderr, "zero-sized append wrote output\n");
    failures++;
  }
  if (!one_byte_destination_stays_terminated()) {
    (void)fprintf(stderr, "one-byte append changed its terminator\n");
    failures++;
  }
  if (!full_destination_writes_nothing()) {
    (void)fprintf(stderr, "full destination append wrote output\n");
    failures++;
  }
  if (!unterminated_destination_writes_nothing()) {
    (void)fprintf(stderr, "unterminated destination append wrote output\n");
    failures++;
  }
  if (!empty_source_succeeds()) {
    (void)fprintf(stderr, "empty source append failed\n");
    failures++;
  }
  if (!append_does_not_write_past_terminator()) {
    (void)fprintf(stderr, "append overwrote bytes beyond its terminator\n");
    failures++;
  }
  return failures != 0;
}
