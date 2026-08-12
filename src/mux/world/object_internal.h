/* Private object lifecycle helpers used by database consistency checks. */

#pragma once

#include <stdbool.h>

#include "mux/objects/db.h"

typedef struct EvaluationContext EvaluationContext;

typedef struct ObjectPointerError {
  EvaluationContext *evaluation;
  DbRef prior;
  DbRef object;
  DbRef location;
  DbRef reference;
  const char *reference_type;
  const char *error_type;
} ObjectPointerError;

void object_log_pointer_error(const ObjectPointerError *error);
void object_log_header_error(EvaluationContext *evaluation, DbRef object,
                             DbRef location, DbRef value, bool value_is_object,
                             const char *valtype, const char *errtype);
void object_log_simple_error(EvaluationContext *evaluation, DbRef object,
                             DbRef location, const char *errtype);
void object_purge_going(EvaluationContext *evaluation, bool full_check);
void object_make_freelist(GameDatabase *database);
