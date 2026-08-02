#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "libtelnet.h"
#include "mux/network/descriptor.h"
#include "mux/network/telnet_environment.h"

static bool expect_value(const Descriptor *descriptor,
                         TelnetEnvironmentKind kind, const void *name,
                         size_t name_size, const void *expected,
                         size_t expected_size) {
  const void *value;
  size_t value_size;

  return descriptor_telnet_environment_has(descriptor, kind, name, name_size) &&
         descriptor_telnet_environment_get(descriptor, kind, name, name_size,
                                           &value, &value_size) &&
         value_size == expected_size &&
         memcmp(value, expected, expected_size) == 0;
}

int main(void) {
  static const char initial[] = {
      TELNET_ENVIRON_IS,
      TELNET_ENVIRON_VAR,
      'U',
      'S',
      'E',
      'R',
      TELNET_ENVIRON_VALUE,
      'a',
      'l',
      'i',
      'c',
      'e',
      TELNET_ENVIRON_USERVAR,
      'E',
      'M',
      'P',
      'T',
      'Y',
      TELNET_ENVIRON_VALUE,
      TELNET_ENVIRON_USERVAR,
      'B',
      'I',
      'N',
      TELNET_ENVIRON_ESC,
      TELNET_ENVIRON_VAR,
      TELNET_ENVIRON_VALUE,
      'x',
      TELNET_ENVIRON_ESC,
      TELNET_ENVIRON_VALUE,
      'y',
  };
  static const unsigned char binary_name[] = {'B', 'I', 'N', 0};
  static const unsigned char binary_value[] = {'x', 1, 'y'};
  static const char replace[] = {
      TELNET_ENVIRON_INFO,
      TELNET_ENVIRON_VAR,
      'U',
      'S',
      'E',
      'R',
      TELNET_ENVIRON_VALUE,
      'b',
      'o',
      'b',
  };
  static const char remove[] = {
      TELNET_ENVIRON_INFO, TELNET_ENVIRON_VAR, 'U', 'S', 'E', 'R',
  };
  static const char malformed[] = {
      TELNET_ENVIRON_INFO,
      TELNET_ENVIRON_VAR,
      'B',
      TELNET_ENVIRON_ESC,
  };
  static const char empty[] = {TELNET_ENVIRON_IS};
  static const char capability_values[] = {
      TELNET_ENVIRON_IS,
      TELNET_ENVIRON_USERVAR,
      'E',
      'X',
      'A',
      'C',
      'T',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'Z',
      'E',
      'R',
      'O',
      TELNET_ENVIRON_VALUE,
      '0',
      TELNET_ENVIRON_USERVAR,
      'E',
      'X',
      'T',
      'R',
      'A',
      TELNET_ENVIRON_VALUE,
      '1',
      ' ',
      TELNET_ENVIRON_USERVAR,
      'E',
      'M',
      'P',
      'T',
      'Y',
      TELNET_ENVIRON_VALUE,
      TELNET_ENVIRON_VAR,
      'V',
      'A',
      'R',
      TELNET_ENVIRON_VALUE,
      '1',
  };
  static const char disable_exact[] = {
      TELNET_ENVIRON_INFO,
      TELNET_ENVIRON_USERVAR,
      'E',
      'X',
      'A',
      'C',
      'T',
      TELNET_ENVIRON_VALUE,
      '0',
  };
  Descriptor descriptor = {0};
  char *oversized;
  size_t oversized_size = 1 + 1 + 1 + 1 + 4097;
  bool result = true;

  descriptor.telnet_environment = telnet_environment_create();
  if (descriptor.telnet_environment == nullptr)
    return 1;
  result &= telnet_environment_receive(descriptor.telnet_environment, initial,
                                       sizeof(initial));
  result &=
      expect_value(&descriptor, TELNET_ENVIRONMENT_VAR, "USER", 4, "alice", 5);
  result &=
      expect_value(&descriptor, TELNET_ENVIRONMENT_USERVAR, "EMPTY", 5, "", 0);
  result &=
      expect_value(&descriptor, TELNET_ENVIRONMENT_USERVAR, binary_name,
                   sizeof(binary_name), binary_value, sizeof(binary_value));
  result &= descriptor_telnet_environment_count(&descriptor) == 3;

  result &= telnet_environment_receive(descriptor.telnet_environment, replace,
                                       sizeof(replace));
  result &=
      expect_value(&descriptor, TELNET_ENVIRONMENT_VAR, "USER", 4, "bob", 3);
  result &= !telnet_environment_receive(descriptor.telnet_environment,
                                        malformed, sizeof(malformed));
  result &=
      expect_value(&descriptor, TELNET_ENVIRONMENT_VAR, "USER", 4, "bob", 3);
  oversized = malloc(oversized_size);
  if (oversized == nullptr) {
    telnet_environment_destroy(descriptor.telnet_environment);
    return 1;
  }
  oversized[0] = TELNET_ENVIRON_INFO;
  oversized[1] = TELNET_ENVIRON_VAR;
  oversized[2] = 'X';
  oversized[3] = TELNET_ENVIRON_VALUE;
  memset(oversized + 4, 'x', oversized_size - 4);
  result &= !telnet_environment_receive(descriptor.telnet_environment,
                                        oversized, oversized_size);
  free(oversized);
  result &=
      expect_value(&descriptor, TELNET_ENVIRONMENT_VAR, "USER", 4, "bob", 3);
  result &= telnet_environment_receive(descriptor.telnet_environment, remove,
                                       sizeof(remove));
  result &= !descriptor_telnet_environment_has(
      &descriptor, TELNET_ENVIRONMENT_VAR, "USER", 4);
  result &= telnet_environment_receive(descriptor.telnet_environment, empty,
                                       sizeof(empty));
  result &= descriptor_telnet_environment_count(&descriptor) == 0;
  result &=
      telnet_environment_receive(descriptor.telnet_environment,
                                 capability_values, sizeof(capability_values));
  result &= descriptor_telnet_environment_value_is_one(
      &descriptor, TELNET_ENVIRONMENT_USERVAR, "EXACT");
  result &= !descriptor_telnet_environment_value_is_one(
      &descriptor, TELNET_ENVIRONMENT_USERVAR, "ZERO");
  result &= !descriptor_telnet_environment_value_is_one(
      &descriptor, TELNET_ENVIRONMENT_USERVAR, "EXTRA");
  result &= !descriptor_telnet_environment_value_is_one(
      &descriptor, TELNET_ENVIRONMENT_USERVAR, "EMPTY");
  result &= !descriptor_telnet_environment_value_is_one(
      &descriptor, TELNET_ENVIRONMENT_USERVAR, "MISSING");
  result &= !descriptor_telnet_environment_value_is_one(
      &descriptor, TELNET_ENVIRONMENT_USERVAR, "VAR");
  result &= descriptor_telnet_environment_value_is_one(
      &descriptor, TELNET_ENVIRONMENT_VAR, "VAR");
  result &= telnet_environment_receive(descriptor.telnet_environment,
                                       disable_exact, sizeof(disable_exact));
  result &= !descriptor_telnet_environment_value_is_one(
      &descriptor, TELNET_ENVIRONMENT_USERVAR, "EXACT");

  telnet_environment_destroy(descriptor.telnet_environment);
  if (!result)
    fprintf(stderr, "telnet environment test failed\n");
  return result ? 0 : 1;
}
