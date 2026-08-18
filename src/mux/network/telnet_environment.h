/** @file
 * RFC 1572 environment storage and lookup.
 */
#pragma once

#include <stddef.h>

#include "mux/network/descriptor.h"

typedef struct Descriptor Descriptor;
typedef struct TelnetEnvironment TelnetEnvironment;

typedef enum TelnetEnvironmentKind : int {
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

/** Creates telnet environment. */

TelnetEnvironment *telnet_environment_create(void);
/** Destroys telnet environment. @param[in,out] environment Environment. */

void telnet_environment_destroy(TelnetEnvironment *environment);
/** Clears telnet environment. @param[in,out] environment Environment. */

void telnet_environment_clear(TelnetEnvironment *environment);

/* Apply one raw NEW-ENVIRON IS or INFO payload atomically. */
/** Executes telnet environment receive. @param[in,out] environment Environment.
 * @param[in] buffer Caller-owned output storage. @param[in] size Storage size
 * in bytes. */

bool telnet_environment_receive(TelnetEnvironment *environment,
                                const char *buffer, size_t size);

/** Executes descriptor telnet environment has. @param[in] descriptor Network
 * descriptor. @param[in] kind Kind. @param[in] name Name to use. @param[in]
 * name_size Size of name in bytes. */

bool descriptor_telnet_environment_has(const Descriptor *descriptor,
                                       TelnetEnvironmentKind kind,
                                       const void *name, size_t name_size);
/* Return false when absent; an empty value is present with value_size zero. */
/** Returns descriptor telnet environment. @param[in] descriptor Network
 * descriptor. @param[in] kind Kind. @param[in] name Name to use. @param[in]
 * name_size Size of name in bytes. @param[in,out] value Value to use.
 * @param[in] value_size Size of value in bytes. */

bool descriptor_telnet_environment_get(const Descriptor *descriptor,
                                       TelnetEnvironmentKind kind,
                                       const void *name, size_t name_size,
                                       const void **value, size_t *value_size);
/** Executes descriptor telnet environment value is one. @param[in] descriptor
 * Network descriptor. @param[in] kind Kind. @param[in] name Name to use. */

bool descriptor_telnet_environment_value_is_one(const Descriptor *descriptor,
                                                TelnetEnvironmentKind kind,
                                                const char *name);
/** Counts descriptor telnet environment. @param[in] descriptor Network
 * descriptor. */

size_t descriptor_telnet_environment_count(const Descriptor *descriptor);
/** Executes descriptor telnet environment entry. @param[in] descriptor Network
 * descriptor. @param[in] index Zero-based index. @param[in,out] entry Entry. */

bool descriptor_telnet_environment_entry(const Descriptor *descriptor,
                                         size_t index,
                                         TelnetEnvironmentEntryView *entry);
