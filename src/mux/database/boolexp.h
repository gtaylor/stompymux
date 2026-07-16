/* boolexp.h - Lock expression parsing and evaluation interface. */

#pragma once

#include "mux/database/db.h"

int boolean_expression_evaluate(DbRef player, DbRef thing, DbRef from,
                                BooleanExpression *expression);
BooleanExpression *boolean_expression_parse(DbRef player, const char *text,
                                            int internal);
int eval_boolexp_atr(DbRef player, DbRef thing, DbRef from, char *key);

char *boolean_expression_unparse(DbRef player, BooleanExpression *expression);
char *boolean_expression_unparse_quiet(DbRef player,
                                       BooleanExpression *expression);
char *boolean_expression_unparse_function(DbRef player,
                                          BooleanExpression *expression);
