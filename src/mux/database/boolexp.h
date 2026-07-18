/* boolexp.h - Lock expression parsing and evaluation interface. */

#pragma once

#include "mux/database/db.h"

typedef struct EvaluationContext EvaluationContext;

int boolean_expression_evaluate(EvaluationContext *context, DbRef player,
                                DbRef thing, DbRef from,
                                BooleanExpression *expression);
BooleanExpression *boolean_expression_parse(GameDatabase *database,
                                            EvaluationContext *evaluation,
                                            DbRef player, const char *text,
                                            int internal);
int eval_boolexp_atr(EvaluationContext *context, DbRef player, DbRef thing,
                     DbRef from, char *key);

void boolean_expression_unparse(GameDatabase *database,
                                EvaluationContext *evaluation, char *buffer,
                                DbRef player, BooleanExpression *expression);
void boolean_expression_unparse_quiet(GameDatabase *database,
                                      EvaluationContext *evaluation,
                                      char *buffer, DbRef player,
                                      BooleanExpression *expression);
void boolean_expression_unparse_function(GameDatabase *database,
                                         EvaluationContext *evaluation,
                                         char *buffer, DbRef player,
                                         BooleanExpression *expression);
