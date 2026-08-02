/* telnet_environment.c - RFC 1572 environment storage and lookup. */

#include "mux/server/platform.h"

#include "mux/network/telnet_environment.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "libtelnet.h"
#include "mux/network/descriptor.h"

constexpr size_t TELNET_ENVIRONMENT_MAX_ENTRIES = 64;
constexpr size_t TELNET_ENVIRONMENT_MAX_NAME_SIZE = 256;
constexpr size_t TELNET_ENVIRONMENT_MAX_VALUE_SIZE = 4096;
constexpr size_t TELNET_ENVIRONMENT_MAX_TOTAL_SIZE = 65536;

typedef struct TelnetEnvironmentEntry {
  TelnetEnvironmentKind kind;
  unsigned char *name;
  size_t name_size;
  unsigned char *value;
  size_t value_size;
} TelnetEnvironmentEntry;

typedef struct TelnetEnvironmentUpdate {
  TelnetEnvironmentKind kind;
  unsigned char *name;
  size_t name_size;
  unsigned char *value;
  size_t value_size;
  bool has_value;
} TelnetEnvironmentUpdate;

struct TelnetEnvironment {
  TelnetEnvironmentEntry entries[TELNET_ENVIRONMENT_MAX_ENTRIES];
  size_t count;
  size_t total_size;
};

static bool telnet_environment_kind_valid(TelnetEnvironmentKind kind) {
  return kind == TELNET_ENVIRONMENT_VAR || kind == TELNET_ENVIRONMENT_USERVAR;
}

static void telnet_environment_entry_destroy(TelnetEnvironmentEntry *entry) {
  free(entry->name);
  free(entry->value);
  *entry = (TelnetEnvironmentEntry){0};
}

TelnetEnvironment *telnet_environment_create(void) {
  return calloc(1, sizeof(TelnetEnvironment));
}

void telnet_environment_clear(TelnetEnvironment *environment) {
  if (environment == nullptr)
    return;
  for (size_t index = 0; index < environment->count; index++)
    telnet_environment_entry_destroy(&environment->entries[index]);
  environment->count = 0;
  environment->total_size = 0;
}

void telnet_environment_destroy(TelnetEnvironment *environment) {
  telnet_environment_clear(environment);
  free(environment);
}

static size_t telnet_environment_find(const TelnetEnvironment *environment,
                                      TelnetEnvironmentKind kind,
                                      const void *name, size_t name_size) {
  if (environment == nullptr || !telnet_environment_kind_valid(kind) ||
      (name == nullptr && name_size != 0))
    return SIZE_MAX;
  for (size_t index = 0; index < environment->count; index++) {
    const TelnetEnvironmentEntry *entry = &environment->entries[index];

    if (entry->kind == kind && entry->name_size == name_size &&
        memcmp(entry->name, name, name_size) == 0)
      return index;
  }
  return SIZE_MAX;
}

bool descriptor_telnet_environment_has(const Descriptor *descriptor,
                                       TelnetEnvironmentKind kind,
                                       const void *name, size_t name_size) {
  return descriptor != nullptr &&
         telnet_environment_find(descriptor->telnet_environment, kind, name,
                                 name_size) != SIZE_MAX;
}

bool descriptor_telnet_environment_get(const Descriptor *descriptor,
                                       TelnetEnvironmentKind kind,
                                       const void *name, size_t name_size,
                                       const void **value, size_t *value_size) {
  size_t index;

  if (value != nullptr)
    *value = nullptr;
  if (value_size != nullptr)
    *value_size = 0;
  if (descriptor == nullptr)
    return false;
  index = telnet_environment_find(descriptor->telnet_environment, kind, name,
                                  name_size);
  if (index == SIZE_MAX)
    return false;
  if (value != nullptr)
    *value = descriptor->telnet_environment->entries[index].value;
  if (value_size != nullptr)
    *value_size = descriptor->telnet_environment->entries[index].value_size;
  return true;
}

bool descriptor_telnet_environment_value_is_one(const Descriptor *descriptor,
                                                TelnetEnvironmentKind kind,
                                                const char *name) {
  const void *value;
  size_t value_size;

  return name != nullptr &&
         descriptor_telnet_environment_get(descriptor, kind, name, strlen(name),
                                           &value, &value_size) &&
         value_size == 1 && *(const unsigned char *)value == '1';
}

size_t descriptor_telnet_environment_count(const Descriptor *descriptor) {
  if (descriptor == nullptr || descriptor->telnet_environment == nullptr)
    return 0;
  return descriptor->telnet_environment->count;
}

bool descriptor_telnet_environment_entry(const Descriptor *descriptor,
                                         size_t index,
                                         TelnetEnvironmentEntryView *entry) {
  const TelnetEnvironmentEntry *stored;

  if (descriptor == nullptr || descriptor->telnet_environment == nullptr ||
      entry == nullptr || index >= descriptor->telnet_environment->count)
    return false;
  stored = &descriptor->telnet_environment->entries[index];
  *entry = (TelnetEnvironmentEntryView){
      .kind = stored->kind,
      .name = stored->name,
      .name_size = stored->name_size,
      .value = stored->value,
      .value_size = stored->value_size,
  };
  return true;
}

static void telnet_environment_updates_destroy(TelnetEnvironmentUpdate *updates,
                                               size_t count) {
  for (size_t index = 0; index < count; index++) {
    free(updates[index].name);
    free(updates[index].value);
  }
}

static bool telnet_environment_parse_bytes(const unsigned char *buffer,
                                           size_t size, size_t *position,
                                           bool parsing_name,
                                           unsigned char **bytes,
                                           size_t *bytes_size) {
  size_t maximum = parsing_name ? TELNET_ENVIRONMENT_MAX_NAME_SIZE
                                : TELNET_ENVIRONMENT_MAX_VALUE_SIZE;
  unsigned char *shrunk;

  while (*position < size) {
    unsigned char byte = buffer[*position];

    if (byte == TELNET_ENVIRON_VAR || byte == TELNET_ENVIRON_USERVAR ||
        (parsing_name && byte == TELNET_ENVIRON_VALUE))
      break;
    if (!parsing_name && byte == TELNET_ENVIRON_VALUE)
      return false;
    (*position)++;
    if (byte == TELNET_ENVIRON_ESC) {
      if (*position == size)
        return false;
      byte = buffer[(*position)++];
    }
    if (*bytes_size == maximum)
      return false;
    if (*bytes == nullptr) {
      *bytes = malloc(maximum);
      if (*bytes == nullptr)
        return false;
    }
    (*bytes)[(*bytes_size)++] = byte;
  }
  if (*bytes_size != 0 && *bytes_size != maximum) {
    shrunk = realloc(*bytes, *bytes_size);
    if (shrunk != nullptr)
      *bytes = shrunk;
  }
  return true;
}

static bool telnet_environment_parse_updates(const char *raw_buffer,
                                             size_t size,
                                             TelnetEnvironmentUpdate *updates,
                                             size_t *update_count) {
  const unsigned char *buffer = (const unsigned char *)raw_buffer;
  size_t position = 1;

  *update_count = 0;
  if (size == 0 ||
      (buffer[0] != TELNET_ENVIRON_IS && buffer[0] != TELNET_ENVIRON_INFO))
    return false;
  while (position < size) {
    TelnetEnvironmentUpdate *update;
    unsigned char marker = buffer[position++];

    if ((marker != TELNET_ENVIRON_VAR && marker != TELNET_ENVIRON_USERVAR) ||
        *update_count == TELNET_ENVIRONMENT_MAX_ENTRIES)
      goto fail;
    update = &updates[(*update_count)++];
    *update = (TelnetEnvironmentUpdate){
        .kind = marker == TELNET_ENVIRON_VAR ? TELNET_ENVIRONMENT_VAR
                                             : TELNET_ENVIRONMENT_USERVAR};
    if (!telnet_environment_parse_bytes(buffer, size, &position, true,
                                        &update->name, &update->name_size) ||
        update->name_size == 0)
      goto fail;
    if (position < size && buffer[position] == TELNET_ENVIRON_VALUE) {
      position++;
      update->has_value = true;
      if (!telnet_environment_parse_bytes(buffer, size, &position, false,
                                          &update->value, &update->value_size))
        goto fail;
    }
  }
  return true;

fail:
  telnet_environment_updates_destroy(updates, *update_count);
  *update_count = 0;
  return false;
}

static bool telnet_environment_remove(TelnetEnvironment *environment,
                                      TelnetEnvironmentKind kind,
                                      const void *name, size_t name_size) {
  size_t index = telnet_environment_find(environment, kind, name, name_size);

  if (index == SIZE_MAX)
    return true;
  environment->total_size -= environment->entries[index].name_size +
                             environment->entries[index].value_size;
  telnet_environment_entry_destroy(&environment->entries[index]);
  if (index + 1 < environment->count)
    memmove(&environment->entries[index], &environment->entries[index + 1],
            (environment->count - index - 1) * sizeof(*environment->entries));
  environment->count--;
  environment->entries[environment->count] = (TelnetEnvironmentEntry){0};
  return true;
}

static bool telnet_environment_set(TelnetEnvironment *environment,
                                   TelnetEnvironmentUpdate *update) {
  size_t index = telnet_environment_find(environment, update->kind,
                                         update->name, update->name_size);
  size_t previous_size = 0;
  size_t new_total;

  if (index != SIZE_MAX)
    previous_size = environment->entries[index].name_size +
                    environment->entries[index].value_size;
  else if (environment->count == TELNET_ENVIRONMENT_MAX_ENTRIES)
    return false;
  if (update->name_size > SIZE_MAX - update->value_size)
    return false;
  new_total = environment->total_size - previous_size + update->name_size +
              update->value_size;
  if (new_total > TELNET_ENVIRONMENT_MAX_TOTAL_SIZE)
    return false;
  if (update->value == nullptr) {
    update->value = calloc(1, 1);
    if (update->value == nullptr)
      return false;
  }
  if (index == SIZE_MAX)
    index = environment->count++;
  else
    telnet_environment_entry_destroy(&environment->entries[index]);
  environment->entries[index] = (TelnetEnvironmentEntry){
      .kind = update->kind,
      .name = update->name,
      .name_size = update->name_size,
      .value = update->value,
      .value_size = update->value_size,
  };
  environment->total_size = new_total;
  update->name = nullptr;
  update->value = nullptr;
  return true;
}

static TelnetEnvironment *
telnet_environment_clone(const TelnetEnvironment *environment) {
  TelnetEnvironment *copy = telnet_environment_create();

  if (copy == nullptr)
    return nullptr;
  for (size_t index = 0; index < environment->count; index++) {
    const TelnetEnvironmentEntry *source = &environment->entries[index];
    TelnetEnvironmentUpdate update = {
        .kind = source->kind,
        .name = malloc(source->name_size),
        .name_size = source->name_size,
        .value = malloc(source->value_size == 0 ? 1 : source->value_size),
        .value_size = source->value_size,
        .has_value = true,
    };

    if ((source->name_size != 0 && update.name == nullptr) ||
        update.value == nullptr) {
      free(update.name);
      free(update.value);
      telnet_environment_destroy(copy);
      return nullptr;
    }
    memcpy(update.name, source->name, source->name_size);
    if (source->value_size != 0)
      memcpy(update.value, source->value, source->value_size);
    else
      update.value[0] = '\0';
    if (!telnet_environment_set(copy, &update)) {
      free(update.name);
      free(update.value);
      telnet_environment_destroy(copy);
      return nullptr;
    }
  }
  return copy;
}

bool telnet_environment_receive(TelnetEnvironment *environment,
                                const char *buffer, size_t size) {
  TelnetEnvironmentUpdate updates[TELNET_ENVIRONMENT_MAX_ENTRIES] = {0};
  TelnetEnvironment *updated;
  size_t update_count;
  bool result = true;

  if (environment == nullptr ||
      !telnet_environment_parse_updates(buffer, size, updates, &update_count))
    return false;
  updated = (unsigned char)buffer[0] == TELNET_ENVIRON_IS
                ? telnet_environment_create()
                : telnet_environment_clone(environment);
  if (updated == nullptr) {
    telnet_environment_updates_destroy(updates, update_count);
    return false;
  }
  for (size_t index = 0; index < update_count; index++) {
    TelnetEnvironmentUpdate *update = &updates[index];

    if (update->has_value)
      result = telnet_environment_set(updated, update);
    else
      result = telnet_environment_remove(updated, update->kind, update->name,
                                         update->name_size);
    if (!result)
      break;
  }
  telnet_environment_updates_destroy(updates, update_count);
  if (!result) {
    telnet_environment_destroy(updated);
    return false;
  }
  telnet_environment_clear(environment);
  *environment = *updated;
  free(updated);
  return true;
}
