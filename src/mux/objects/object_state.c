/* object_state.c - Typed, namespaced persistent object state. */

#include <math.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"

constexpr size_t OBJECT_STATE_NAMESPACE_LIMIT = 127;
constexpr size_t OBJECT_STATE_KEY_LIMIT = 255;
constexpr size_t OBJECT_STATE_DEFAULT_VALUE_LIMIT = 65536;
constexpr size_t OBJECT_STATE_DEFAULT_ENTRY_LIMIT = 1024;
constexpr size_t OBJECT_STATE_DEFAULT_OBJECT_LIMIT = 1024 * 1024;

typedef struct ObjectStateEntry ObjectStateEntry;
struct ObjectStateEntry {
  char *name_space;
  char *key;
  ObjectStateValue value;
};

struct ObjectStateCollection {
  ObjectStateEntry *entries;
  size_t count;
  size_t capacity;
  size_t bytes;
};

static ObjectStateEntry *
object_state_entry_slot(ObjectStateCollection *collection, size_t index) {
  return checked_storage_at(collection->entries, collection->capacity,
                            sizeof(*collection->entries), index);
}

static const ObjectStateEntry *
object_state_entry_const(const ObjectStateCollection *collection,
                         size_t index) {
  return checked_storage_at_const(collection->entries, collection->capacity,
                                  sizeof(*collection->entries), index);
}

typedef struct ObjectStateTransactionObject ObjectStateTransactionObject;
struct ObjectStateTransactionObject {
  DbRef object;
  ObjectStateCollection *collection;
};

static ObjectStateTransactionObject *
object_state_transaction_object(ObjectStateTransaction *transaction,
                                size_t index) {
  return checked_storage_at(transaction->objects, transaction->object_capacity,
                            sizeof(ObjectStateTransactionObject), index);
}

static void object_state_error(char *error, size_t error_size,
                               const char *format, ...)
    __attribute__((format(printf, 3, 4)));

static void object_state_error(char *error, size_t error_size,
                               const char *format, ...) {
  va_list arguments;

  if (!error || error_size == 0)
    return;
  va_start(arguments, format);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  vsnprintf(error, error_size, format, arguments);
  va_end(arguments);
}

bool object_state_name_is_valid(const char *name) {
  if (name == nullptr)
    return false;
  const size_t length = strlen(name);
  if (length == 0)
    return false;
  for (size_t index = 0; index < length; index++) {
    const unsigned char character =
        *(const unsigned char *)checked_storage_at_const(name, length + 1,
                                                         sizeof(char), index);
    if (index == 0 && !((character >= 'A' && character <= 'Z') ||
                        (character >= 'a' && character <= 'z')))
      return false;
    if (!((character >= 'A' && character <= 'Z') ||
          (character >= 'a' && character <= 'z') ||
          (character >= '0' && character <= '9')) &&
        character != '_' && character != '-' && character != '.' &&
        character != '/')
      return false;
  }
  return true;
}

static int object_state_entry_compare(const ObjectStateEntry *entry,
                                      const char *name_space, const char *key) {
  int comparison = strcmp(entry->name_space, name_space);

  return comparison != 0 ? comparison : strcmp(entry->key, key);
}

static size_t object_state_find(const ObjectStateCollection *collection,
                                const char *name_space, const char *key,
                                bool *found) {
  size_t low = 0;
  size_t high = collection ? collection->count : 0;

  while (low < high) {
    size_t middle = low + (high - low) / 2;
    int comparison = object_state_entry_compare(
        object_state_entry_const(collection, middle), name_space, key);
    if (comparison == 0) {
      *found = true;
      return middle;
    }
    if (comparison < 0)
      low = middle + 1;
    else
      high = middle;
  }
  *found = false;
  return low;
}

static size_t object_state_value_bytes(const ObjectStateValue *value) {
  switch (value->type) {
  case OBJECT_STATE_STRING:
    return value->as.string.length;
  case OBJECT_STATE_BOOLEAN:
    return sizeof(bool);
  case OBJECT_STATE_INTEGER:
    return sizeof(int64_t);
  case OBJECT_STATE_NUMBER:
    return sizeof(double);
  }
  return 0;
}

static size_t object_state_entry_bytes(const char *name_space, const char *key,
                                       const ObjectStateValue *value) {
  return strlen(name_space) + 1 + strlen(key) + 1 +
         object_state_value_bytes(value);
}

static void object_state_value_destroy(ObjectStateValue *value) {
  if (value->type == OBJECT_STATE_STRING) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
    free((char *)value->as.string.data);
#pragma clang diagnostic pop
  }
  memset(value, 0, sizeof(*value));
}

static bool object_state_value_copy(ObjectStateValue *destination,
                                    const ObjectStateValue *source) {
  *destination = *source;
  if (source->type != OBJECT_STATE_STRING)
    return true;
  char *data = malloc(source->as.string.length + 1);
  if (!data)
    return false;
  if (source->as.string.length > 0)
    memcpy(data, source->as.string.data, source->as.string.length);
  *(char *)checked_storage_at(data, source->as.string.length + 1, sizeof(char),
                              source->as.string.length) = '\0';
  destination->as.string.data = data;
  return true;
}

static void object_state_collection_destroy(ObjectStateCollection *collection) {
  if (!collection)
    return;
  for (size_t index = 0; index < collection->count; index++) {
    ObjectStateEntry *entry = object_state_entry_slot(collection, index);
    free(entry->name_space);
    free(entry->key);
    object_state_value_destroy(&entry->value);
  }
  free(collection->entries);
  free(collection);
}

static ObjectStateCollection *
object_state_collection_clone(const ObjectStateCollection *source) {
  ObjectStateCollection *copy = calloc(1, sizeof(*copy));

  if (!copy)
    return nullptr;
  if (!source || source->count == 0)
    return copy;
  copy->entries = calloc(source->count, sizeof(*copy->entries));
  if (!copy->entries) {
    free(copy);
    return nullptr;
  }
  copy->capacity = source->count;
  for (size_t index = 0; index < source->count; index++) {
    ObjectStateEntry *destination = object_state_entry_slot(copy, index);
    const ObjectStateEntry *entry = object_state_entry_const(source, index);

    destination->name_space = strdup(entry->name_space);
    destination->key = strdup(entry->key);
    if (!destination->name_space || !destination->key ||
        !object_state_value_copy(&destination->value, &entry->value)) {
      copy->count = index + 1;
      object_state_collection_destroy(copy);
      return nullptr;
    }
    copy->count++;
  }
  copy->bytes = source->bytes;
  return copy;
}

static size_t object_state_value_limit(const GameDatabase *database) {
  if (database->configuration &&
      database->configuration->lua.state_value_limit > 0)
    return (size_t)database->configuration->lua.state_value_limit;
  return OBJECT_STATE_DEFAULT_VALUE_LIMIT;
}

static size_t object_state_entry_limit(const GameDatabase *database) {
  if (database->configuration &&
      database->configuration->lua.state_entry_limit > 0)
    return (size_t)database->configuration->lua.state_entry_limit;
  return OBJECT_STATE_DEFAULT_ENTRY_LIMIT;
}

static size_t object_state_object_limit(const GameDatabase *database) {
  if (database->configuration &&
      database->configuration->lua.state_object_limit > 0)
    return (size_t)database->configuration->lua.state_object_limit;
  return OBJECT_STATE_DEFAULT_OBJECT_LIMIT;
}

static bool object_state_value_is_valid(const ObjectStateValue *value) {
  if (!value)
    return false;
  switch (value->type) {
  case OBJECT_STATE_STRING:
    return value->as.string.data != nullptr || value->as.string.length == 0;
  case OBJECT_STATE_BOOLEAN:
  case OBJECT_STATE_INTEGER:
    return true;
  case OBJECT_STATE_NUMBER:
    return isfinite(value->as.number);
  }
  return false;
}

static bool object_state_collection_set(GameDatabase *database,
                                        ObjectStateCollection *collection,
                                        const char *name_space, const char *key,
                                        const ObjectStateValue *value,
                                        char *error, size_t error_size) {
  bool found;
  size_t index;
  size_t old_bytes = 0;
  size_t new_bytes;
  ObjectStateValue value_copy;

  if (!collection || (collection->count > 0 && !collection->entries)) {
    object_state_error(error, error_size, "invalid object state collection");
    return false;
  }
  if (!name_space || strlen(name_space) > OBJECT_STATE_NAMESPACE_LIMIT ||
      !object_state_name_is_valid(name_space)) {
    object_state_error(error, error_size, "invalid state namespace");
    return false;
  }
  if (!key || strlen(key) > OBJECT_STATE_KEY_LIMIT ||
      !object_state_name_is_valid(key)) {
    object_state_error(error, error_size, "invalid state key");
    return false;
  }
  if (!object_state_value_is_valid(value)) {
    object_state_error(error, error_size, "invalid state value");
    return false;
  }
  if (object_state_value_bytes(value) > object_state_value_limit(database)) {
    object_state_error(error, error_size, "state value exceeds %zu bytes",
                       object_state_value_limit(database));
    return false;
  }

  index = object_state_find(collection, name_space, key, &found);
  if (found && (collection->entries == nullptr || index >= collection->count)) {
    object_state_error(error, error_size, "invalid object state collection");
    return false;
  }
  if (found) {
    const ObjectStateEntry *entry = object_state_entry_const(collection, index);
    old_bytes =
        object_state_entry_bytes(entry->name_space, entry->key, &entry->value);
  }
  if (!found && collection->count >= object_state_entry_limit(database)) {
    object_state_error(error, error_size, "object state exceeds %zu entries",
                       object_state_entry_limit(database));
    return false;
  }
  new_bytes = object_state_entry_bytes(name_space, key, value);
  if (collection->bytes - old_bytes + new_bytes >
      object_state_object_limit(database)) {
    object_state_error(error, error_size, "object state exceeds %zu bytes",
                       object_state_object_limit(database));
    return false;
  }
  memset(&value_copy, 0, sizeof(value_copy));
  if (!object_state_value_copy(&value_copy, value)) {
    object_state_error(error, error_size, "out of memory");
    return false;
  }
  if (found) {
    ObjectStateEntry *entry = object_state_entry_slot(collection, index);
    object_state_value_destroy(&entry->value);
    entry->value = value_copy;
    collection->bytes = collection->bytes - old_bytes + new_bytes;
    return true;
  }
  if (collection->count == collection->capacity) {
    size_t capacity = collection->capacity == 0 ? 8 : collection->capacity * 2;
    ObjectStateEntry *entries =
        realloc(collection->entries, capacity * sizeof(*entries));
    if (!entries) {
      object_state_value_destroy(&value_copy);
      object_state_error(error, error_size, "out of memory");
      return false;
    }
    collection->entries = entries;
    collection->capacity = capacity;
  }
  char *namespace_copy = strdup(name_space);
  char *key_copy = strdup(key);
  if (!namespace_copy || !key_copy) {
    free(namespace_copy);
    free(key_copy);
    object_state_value_destroy(&value_copy);
    object_state_error(error, error_size, "out of memory");
    return false;
  }
  if (index < collection->count)
    memmove(object_state_entry_slot(collection, index + 1),
            object_state_entry_slot(collection, index),
            (collection->count - index) * sizeof(*collection->entries));
  *object_state_entry_slot(collection, index) = (ObjectStateEntry){
      .name_space = namespace_copy, .key = key_copy, .value = value_copy};
  collection->count++;
  collection->bytes += new_bytes;
  return true;
}

static bool object_state_collection_delete(ObjectStateCollection *collection,
                                           const char *name_space,
                                           const char *key) {
  bool found;
  size_t index;

  if (!collection || !name_space || !key)
    return false;
  index = object_state_find(collection, name_space, key, &found);
  if (!found)
    return false;
  ObjectStateEntry *entry = object_state_entry_slot(collection, index);
  collection->bytes -=
      object_state_entry_bytes(entry->name_space, entry->key, &entry->value);
  free(entry->name_space);
  free(entry->key);
  object_state_value_destroy(&entry->value);
  collection->count--;
  if (index < collection->count)
    memmove(object_state_entry_slot(collection, index),
            object_state_entry_slot(collection, index + 1),
            (collection->count - index) * sizeof(*collection->entries));
  return true;
}

static ObjectStateCollection *
object_state_collection_require(GameDatabase *database, DbRef object) {
  GameObject *game_object = game_database_object(database, object);

  if (!game_object->state)
    game_object->state = calloc(1, sizeof(*game_object->state));
  return game_object->state;
}

const ObjectStateValue *object_state_get(GameDatabase *database, DbRef object,
                                         const char *name_space,
                                         const char *key) {
  bool found;
  size_t index;
  ObjectStateCollection *collection;

  if (!is_good_obj(database, object) || !name_space || !key)
    return nullptr;
  collection = game_database_object(database, object)->state;
  if (!collection)
    return nullptr;
  index = object_state_find(collection, name_space, key, &found);
  return found ? &object_state_entry_slot(collection, index)->value : nullptr;
}

bool object_state_set(GameDatabase *database, DbRef object,
                      const char *name_space, const char *key,
                      const ObjectStateValue *value, char *error,
                      size_t error_size) {
  ObjectStateCollection *collection;

  if (!is_good_obj(database, object)) {
    object_state_error(error, error_size, "invalid object");
    return false;
  }
  collection = object_state_collection_require(database, object);
  if (!collection) {
    object_state_error(error, error_size, "out of memory");
    return false;
  }
  return object_state_collection_set(database, collection, name_space, key,
                                     value, error, error_size);
}

bool object_state_delete(GameDatabase *database, DbRef object,
                         const char *name_space, const char *key) {
  if (!is_good_obj(database, object))
    return false;
  return object_state_collection_delete(
      game_database_object(database, object)->state, name_space, key);
}

size_t object_state_count(GameDatabase *database, DbRef object) {
  ObjectStateCollection *collection;

  if (!is_good_obj(database, object))
    return 0;
  collection = game_database_object(database, object)->state;
  return collection ? collection->count : 0;
}

bool object_state_entry(GameDatabase *database, DbRef object, size_t index,
                        ObjectStateEntryView *entry) {
  ObjectStateCollection *collection;

  if (!entry || !is_good_obj(database, object))
    return false;
  collection = game_database_object(database, object)->state;
  if (!collection || index >= collection->count)
    return false;
  const ObjectStateEntry *stored = object_state_entry_const(collection, index);
  *entry = (ObjectStateEntryView){
      .name_space = stored->name_space,
      .key = stored->key,
      .value = &stored->value,
  };
  return true;
}

void object_state_clear(GameDatabase *database, DbRef object) {
  if (object < 0 || object >= database->size)
    return;
  object_state_collection_destroy(
      game_database_object(database, object)->state);
  game_database_object(database, object)->state = nullptr;
}

bool object_state_copy(GameDatabase *database, DbRef destination,
                       DbRef source) {
  ObjectStateCollection *copy;

  if (!is_good_obj(database, destination) || !is_good_obj(database, source))
    return false;
  copy = object_state_collection_clone(
      game_database_object(database, source)->state);
  if (!copy)
    return false;
  object_state_clear(database, destination);
  game_database_object(database, destination)->state = copy;
  return true;
}

void object_state_transaction_initialize(ObjectStateTransaction *transaction) {
  memset(transaction, 0, sizeof(*transaction));
}

bool object_state_transaction_begin(ObjectStateTransaction *transaction,
                                    GameDatabase *database) {
  if (transaction->depth > 0) {
    if (transaction->database != database)
      return false;
    transaction->depth++;
    return true;
  }
  transaction->database = database;
  transaction->depth = 1;
  transaction->rollback_only = false;
  return true;
}

static ObjectStateTransactionObject *
object_state_transaction_find(ObjectStateTransaction *transaction,
                              DbRef object) {
  for (size_t index = 0; index < transaction->object_count; index++) {
    ObjectStateTransactionObject *candidate =
        object_state_transaction_object(transaction, index);
    if (candidate->object == object)
      return candidate;
  }
  return nullptr;
}

static ObjectStateTransactionObject *
object_state_transaction_require(ObjectStateTransaction *transaction,
                                 DbRef object, char *error, size_t error_size) {
  ObjectStateTransactionObject *target =
      object_state_transaction_find(transaction, object);
  ObjectStateTransactionObject *objects;

  if (target)
    return target;
  if (!transaction->depth) {
    object_state_error(error, error_size,
                       "state writes require an active Lua callback");
    return nullptr;
  }
  if (!is_good_obj(transaction->database, object)) {
    object_state_error(error, error_size, "invalid object");
    return nullptr;
  }
  if (transaction->object_count == transaction->object_capacity) {
    size_t capacity = transaction->object_capacity == 0
                          ? 4
                          : transaction->object_capacity * 2;
    objects = realloc(transaction->objects, capacity * sizeof(*objects));
    if (!objects) {
      object_state_error(error, error_size, "out of memory");
      return nullptr;
    }
    transaction->objects = objects;
    transaction->object_capacity = capacity;
  }
  target =
      object_state_transaction_object(transaction, transaction->object_count);
  target->object = object;
  target->collection = object_state_collection_clone(
      game_database_object(transaction->database, object)->state);
  if (!target->collection) {
    object_state_error(error, error_size, "out of memory");
    return nullptr;
  }
  transaction->object_count++;
  return target;
}

const ObjectStateValue *
object_state_transaction_get(ObjectStateTransaction *transaction, DbRef object,
                             const char *name_space, const char *key) {
  ObjectStateTransactionObject *target =
      object_state_transaction_find(transaction, object);
  bool found;
  size_t index;

  if (!target)
    return object_state_get(transaction->database, object, name_space, key);
  index = object_state_find(target->collection, name_space, key, &found);
  return found ? &object_state_entry_slot(target->collection, index)->value
               : nullptr;
}

bool object_state_transaction_set(ObjectStateTransaction *transaction,
                                  DbRef object, const char *name_space,
                                  const char *key,
                                  const ObjectStateValue *value, char *error,
                                  size_t error_size) {
  ObjectStateTransactionObject *target =
      object_state_transaction_require(transaction, object, error, error_size);

  return target &&
         object_state_collection_set(transaction->database, target->collection,
                                     name_space, key, value, error, error_size);
}

bool object_state_transaction_delete(ObjectStateTransaction *transaction,
                                     DbRef object, const char *name_space,
                                     const char *key) {
  ObjectStateTransactionObject *target =
      object_state_transaction_require(transaction, object, nullptr, 0);

  return target &&
         object_state_collection_delete(target->collection, name_space, key);
}

size_t object_state_transaction_count(ObjectStateTransaction *transaction,
                                      DbRef object, const char *name_space) {
  ObjectStateTransactionObject *target =
      object_state_transaction_find(transaction, object);
  ObjectStateCollection *collection =
      target ? target->collection
             : game_database_object(transaction->database, object)->state;
  size_t count = 0;

  if (!collection)
    return 0;
  for (size_t index = 0; index < collection->count; index++) {
    if (!strcmp(object_state_entry_const(collection, index)->name_space,
                name_space))
      count++;
  }
  return count;
}

bool object_state_transaction_entry(ObjectStateTransaction *transaction,
                                    DbRef object, const char *name_space,
                                    size_t index, ObjectStateEntryView *entry) {
  ObjectStateTransactionObject *target =
      object_state_transaction_find(transaction, object);
  ObjectStateCollection *collection =
      target ? target->collection
             : game_database_object(transaction->database, object)->state;
  size_t current = 0;

  if (!collection || !entry)
    return false;
  for (size_t position = 0; position < collection->count; position++) {
    ObjectStateEntry *candidate = object_state_entry_slot(collection, position);
    if (strcmp(candidate->name_space, name_space))
      continue;
    if (current++ != index)
      continue;
    *entry = (ObjectStateEntryView){
        .name_space = candidate->name_space,
        .key = candidate->key,
        .value = &candidate->value,
    };
    return true;
  }
  return false;
}

static void
object_state_transaction_reset(ObjectStateTransaction *transaction) {
  for (size_t index = 0; index < transaction->object_count; index++)
    object_state_collection_destroy(
        object_state_transaction_object(transaction, index)->collection);
  transaction->object_count = 0;
  transaction->depth = 0;
  transaction->database = nullptr;
  transaction->rollback_only = false;
}

void object_state_transaction_finish(ObjectStateTransaction *transaction,
                                     bool commit) {
  if (transaction->depth == 0)
    return;
  if (!commit)
    transaction->rollback_only = true;
  transaction->depth--;
  if (transaction->depth > 0)
    return;
  if (!transaction->rollback_only) {
    for (size_t index = 0; index < transaction->object_count; index++) {
      ObjectStateTransactionObject *stored =
          object_state_transaction_object(transaction, index);
      GameObject *object =
          game_database_object(transaction->database, stored->object);
      object_state_collection_destroy(object->state);
      object->state = stored->collection;
      stored->collection = nullptr;
    }
  }
  object_state_transaction_reset(transaction);
}

void object_state_transaction_destroy(ObjectStateTransaction *transaction) {
  object_state_transaction_reset(transaction);
  free(transaction->objects);
  memset(transaction, 0, sizeof(*transaction));
}
