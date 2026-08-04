/* telnet_environment.h - RFC 1572 environment storage and lookup. */

#pragma once

#include <stddef.h>

#include "mux/network/descriptor.h"

typedef struct Descriptor Descriptor;
typedef struct TelnetEnvironment TelnetEnvironment;

typedef enum TelnetEnvironmentKind {
  TELNET_ENVIRONMENT_VAR = 0,
  TELNET_ENVIRONMENT_USERVAR = 3,
} TelnetEnvironmentKind;

typedef struct TelnetEnvironmentEntryView {
  TelnetEnvironmentKind kind;
  const unsigned char *name;
  size_t name_size;
  const unsigned char *value;
  size_t value_size;
} TelnetEnvironmentEntryView;

TelnetEnvironment *telnet_environment_create(void);
void telnet_environment_destroy(TelnetEnvironment *environment);
void telnet_environment_clear(TelnetEnvironment *environment);

/* Apply one raw NEW-ENVIRON IS or INFO payload atomically. */
bool telnet_environment_receive(TelnetEnvironment *environment,
                                const char *buffer, size_t size);

bool descriptor_telnet_environment_has(const Descriptor *descriptor,
                                       TelnetEnvironmentKind kind,
                                       const void *name, size_t name_size);
/* Return false when absent; an empty value is present with value_size zero. */
bool descriptor_telnet_environment_get(const Descriptor *descriptor,
                                       TelnetEnvironmentKind kind,
                                       const void *name, size_t name_size,
                                       const void **value, size_t *value_size);
bool descriptor_telnet_environment_value_is_one(const Descriptor *descriptor,
                                                TelnetEnvironmentKind kind,
                                                const char *name);
size_t descriptor_telnet_environment_count(const Descriptor *descriptor);
bool descriptor_telnet_environment_entry(const Descriptor *descriptor,
                                         size_t index,
                                         TelnetEnvironmentEntryView *entry);
