/* object_state.h - Typed, namespaced persistent object state. */

#pragma once

#include "mux/server/platform.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct GameDatabase GameDatabase;
typedef struct ObjectStateCollection ObjectStateCollection;

typedef enum ObjectStateType {
  OBJECT_STATE_STRING = 1,
  OBJECT_STATE_BOOLEAN,
  OBJECT_STATE_INTEGER,
  OBJECT_STATE_NUMBER,
} ObjectStateType;

typedef struct ObjectStateString ObjectStateString;
struct ObjectStateString {
  const char *data;
  size_t length;
};

typedef struct ObjectStateValue ObjectStateValue;
struct ObjectStateValue {
  ObjectStateType type;
  union {
    ObjectStateString string;
    bool boolean;
    int64_t integer;
    double number;
  } as;
};

typedef struct ObjectStateEntryView ObjectStateEntryView;
struct ObjectStateEntryView {
  const char *name_space;
  const char *key;
  const ObjectStateValue *value;
};

typedef struct ObjectStateTransaction ObjectStateTransaction;
struct ObjectStateTransaction {
  GameDatabase *database;
  void *objects;
  size_t object_count;
  size_t object_capacity;
  unsigned int depth;
  bool rollback_only;
};

bool object_state_name_is_valid(const char *name);
const ObjectStateValue *object_state_get(GameDatabase *database, DbRef object,
                                         const char *name_space,
                                         const char *key);
bool object_state_set(GameDatabase *database, DbRef object,
                      const char *name_space, const char *key,
                      const ObjectStateValue *value, char *error,
                      size_t error_size);
bool object_state_delete(GameDatabase *database, DbRef object,
                         const char *name_space, const char *key);
size_t object_state_count(GameDatabase *database, DbRef object);
bool object_state_entry(GameDatabase *database, DbRef object, size_t index,
                        ObjectStateEntryView *entry);
void object_state_clear(GameDatabase *database, DbRef object);
bool object_state_copy(GameDatabase *database, DbRef destination, DbRef source);

void object_state_transaction_initialize(ObjectStateTransaction *transaction);
bool object_state_transaction_begin(ObjectStateTransaction *transaction,
                                    GameDatabase *database);
const ObjectStateValue *
object_state_transaction_get(ObjectStateTransaction *transaction, DbRef object,
                             const char *name_space, const char *key);
bool object_state_transaction_set(ObjectStateTransaction *transaction,
                                  DbRef object, const char *name_space,
                                  const char *key,
                                  const ObjectStateValue *value, char *error,
                                  size_t error_size);
bool object_state_transaction_delete(ObjectStateTransaction *transaction,
                                     DbRef object, const char *name_space,
                                     const char *key);
size_t object_state_transaction_count(ObjectStateTransaction *transaction,
                                      DbRef object, const char *name_space);
bool object_state_transaction_entry(ObjectStateTransaction *transaction,
                                    DbRef object, const char *name_space,
                                    size_t index, ObjectStateEntryView *entry);
void object_state_transaction_finish(ObjectStateTransaction *transaction,
                                     bool commit);
void object_state_transaction_destroy(ObjectStateTransaction *transaction);
