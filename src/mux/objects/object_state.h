/** @file
 * Typed, namespaced persistent object state.
 */
#pragma once

#include <stddef.h>
#include <stdint.h>

#include "mux/objects/db.h"
#include "mux/server/platform.h"

typedef struct GameDatabase GameDatabase;
typedef struct ObjectStateCollection ObjectStateCollection;

typedef enum ObjectStateType : int {
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
  void *savepoints;
  size_t savepoint_count;
  size_t savepoint_capacity;
  unsigned int depth;
};

/** Executes object state name is valid. @param[in] name Name to use. */

bool object_state_name_is_valid(const char *name);
/** Returns object state. @param[in] database Game database. @param[in] object
 * Game object. @param[in] name_space Name space. @param[in] key Lookup key or
 * command flags. */

const ObjectStateValue *object_state_get(GameDatabase *database, DbRef object,
                                         const char *name_space,
                                         const char *key);
/** Sets object state. @param[in,out] database Game database. @param[in] object
 * Game object. @param[in] name_space Name space. @param[in] key Lookup key or
 * command flags. @param[in] value Value to use. @param[out] error Storage
 * receiving an error description. @param[in] error_size Size of error in bytes.
 */

bool object_state_set(GameDatabase *database, DbRef object,
                      const char *name_space, const char *key,
                      const ObjectStateValue *value, char *error,
                      size_t error_size);
/** Executes object state delete. @param[in,out] database Game database.
 * @param[in] object Game object. @param[in] name_space Name space. @param[in]
 * key Lookup key or command flags. */

bool object_state_delete(GameDatabase *database, DbRef object,
                         const char *name_space, const char *key);
/** Counts object state. @param[in] database Game database. @param[in] object
 * Game object. */

size_t object_state_count(GameDatabase *database, DbRef object);
typedef struct ObjectStateEntryRequest {
  GameDatabase *database;
  DbRef object;
  size_t index;
} ObjectStateEntryRequest;

typedef struct ObjectStateEntryResult {
  bool found;
  ObjectStateEntryView entry;
} ObjectStateEntryResult;

/** Executes object state entry. @param[in] request Request. */

ObjectStateEntryResult
object_state_entry(const ObjectStateEntryRequest *request);
/** Clears object state. @param[in,out] database Game database. @param[in]
 * object Game object. */

void object_state_clear(GameDatabase *database, DbRef object);
/** Executes object state copy. @param[in,out] database Game database.
 * @param[in] destination Destination storage. @param[in] source Source value.
 */

bool object_state_copy(GameDatabase *database, DbRef destination, DbRef source);

/** Initializes object state transaction. @param[out] transaction Transaction.
 */

void object_state_transaction_initialize(ObjectStateTransaction *transaction);
/** Executes object state transaction begin. @param[in,out] transaction
 * Transaction. @param[in,out] database Game database. */

bool object_state_transaction_begin(ObjectStateTransaction *transaction,
                                    GameDatabase *database);
/** Returns object state transaction. @param[in] transaction Transaction.
 * @param[in] object Game object. @param[in] name_space Name space. @param[in]
 * key Lookup key or command flags. */

const ObjectStateValue *
object_state_transaction_get(ObjectStateTransaction *transaction, DbRef object,
                             const char *name_space, const char *key);
/** Sets object state transaction. @param[in,out] transaction Transaction.
 * @param[in] object Game object. @param[in] name_space Name space. @param[in]
 * key Lookup key or command flags. @param[in] value Value to use. @param[out]
 * error Storage receiving an error description. @param[in] error_size Size of
 * error in bytes. */

bool object_state_transaction_set(ObjectStateTransaction *transaction,
                                  DbRef object, const char *name_space,
                                  const char *key,
                                  const ObjectStateValue *value, char *error,
                                  size_t error_size);
/** Executes object state transaction delete. @param[in,out] transaction
 * Transaction. @param[in] object Game object. @param[in] name_space Name space.
 * @param[in] key Lookup key or command flags. */

bool object_state_transaction_delete(ObjectStateTransaction *transaction,
                                     DbRef object, const char *name_space,
                                     const char *key);
/** Counts object state transaction. @param[in] transaction Transaction.
 * @param[in] object Game object. @param[in] name_space Name space. */

size_t object_state_transaction_count(ObjectStateTransaction *transaction,
                                      DbRef object, const char *name_space);
/** Executes object state transaction entry. @param[in,out] transaction
 * Transaction. @param[in] object Game object. @param[in] name_space Name space.
 * @param[in] index Zero-based index. @param[in,out] entry Entry. */

bool object_state_transaction_entry(ObjectStateTransaction *transaction,
                                    DbRef object, const char *name_space,
                                    size_t index, ObjectStateEntryView *entry);
/** Executes object state transaction finish. @param[in,out] transaction
 * Transaction. @param[in] commit Commit. */

void object_state_transaction_finish(ObjectStateTransaction *transaction,
                                     bool commit);
/** Destroys object state transaction. @param[in,out] transaction Transaction.
 */

void object_state_transaction_destroy(ObjectStateTransaction *transaction);
