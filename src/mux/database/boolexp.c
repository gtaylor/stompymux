/* boolexp.c - Lock expression parsing, evaluation, and serialization. */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command_context.h"
#include "mux/database/attrs.h"
#include "mux/database/boolexp.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"

struct BooleanExpression *alloc_bool(const char *s) {
  return (struct BooleanExpression *)malloc(sizeof(struct BooleanExpression));
}

void free_bool(struct BooleanExpression *b) {
  if (b)
    free(b);
}

/**
 * Indicate if attribute ATTR on player passes key when checked by
 * the object lockobj
 */
static int check_attr(EvaluationContext *context, DbRef player, DbRef lockobj,
                      Attribute *attr, char *key) {
  char *buff;
  DbRef aowner;
  long aflags;
  int checkit;

  buff = attribute_parent_get(context->world->database, player, attr->number,
                              &aowner, &aflags);
  checkit = 0;

  /* We can see enterlocks... else we'd break zones */
  if (attr->number == A_LENTER || attr->number == A_NAME ||
      see_attr(context, lockobj, player, attr, aowner, aflags)) {
    checkit = 1;
  }
  if (checkit && (!wild_match(key, buff))) {
    checkit = 0;
  }
  free_lbuf(buff);
  return checkit;
} /* end check_attr() */

int boolean_expression_evaluate(EvaluationContext *context, DbRef player,
                                DbRef thing, DbRef from, BooleanExpression *b) {
  DbRef aowner, obj, source;
  long aflags;
  int c, checkit;
  char *key, *buff, *buff2, *bp, *str;
  Attribute *a;

  if (b == TRUE_BOOLEXP)
    return 1;

  switch (b->type) {
  case BOOLEXP_AND:
    return (
        boolean_expression_evaluate(context, player, thing, from, b->sub1) &&
        boolean_expression_evaluate(context, player, thing, from, b->sub2));
  case BOOLEXP_OR:
    return (
        boolean_expression_evaluate(context, player, thing, from, b->sub1) ||
        boolean_expression_evaluate(context, player, thing, from, b->sub2));
  case BOOLEXP_NOT:
    return !boolean_expression_evaluate(context, player, thing, from, b->sub1);
  case BOOLEXP_INDIR:
    /*
     * BOOLEXP_INDIR (i.e. @) is a unary operation which is replaced at
     * evaluation time by the lock of the object whose number is the
     * argument of the operation.
     */

    context->lock_nesting++;
    if (context->lock_nesting >= context->world->configuration->lock_nest_lim) {
      //            log_error(context->log, LOG_BUGS, "BUG", "LOCK", "
      STARTLOG(context->log, LOG_BUGS, "BUG", "LOCK") {
        log_name_and_loc(context->log, player);
        log_text(": Lock exceeded recursion limit.");
        ENDLOG(context->log);
      }
      notify(context, player, "Sorry, broken lock!");
      context->lock_nesting--;
      return (0);
    }
    if ((b->sub1->type != BOOLEXP_CONST) || (b->sub1->thing < 0)) {
      STARTLOG(context->log, LOG_BUGS, "BUG", "LOCK") {
        log_name_and_loc(context->log, player);
        buff = alloc_mbuf("boolean_expression_evaluate.LOG.indir");
        snprintf(buff, MBUF_SIZE, ": Lock had bad indirection (%c, type %d)",
                 INDIR_TOKEN, b->sub1->type);
        log_text(buff);
        free_mbuf(buff);
        ENDLOG(context->log);
      }
      notify(context, player, "Sorry, broken lock!");
      context->lock_nesting--;
      return (0);
    }
    key = attribute_get(context->world->database, b->sub1->thing, A_LOCK,
                        &aowner, &aflags);
    c = eval_boolexp_atr(context, player, b->sub1->thing, from, key);
    free_lbuf(key);
    context->lock_nesting--;
    return (c);
  case BOOLEXP_CONST:
    return (b->thing == player ||
            member(context->world->database, b->thing,
                   game_object_contents(context->world->database, player)));
  case BOOLEXP_ATR:
    a = attribute_by_number(context->world->database, (int)b->thing);
    if (!a)
      return 0; /*
                 * no such attribute
                 */

    /*
     * First check the object itself, then its contents
     */

    if (check_attr(context, player, from, a, (char *)b->sub1))
      return 1;
    DOLIST(context->world->database, obj,
           game_object_contents(context->world->database, player)) {
      if (check_attr(context, obj, from, a, (char *)b->sub1))
        return 1;
    }
    return 0;
  case BOOLEXP_EVAL:
    a = attribute_by_number(context->world->database, (int)b->thing);
    if (!a)
      return 0; /*
                 * no such attribute
                 */
    source = from;
    buff = attribute_parent_get(context->world->database, from, a->number,
                                &aowner, &aflags);
    if (!buff || !*buff) {
      free_lbuf(buff);
      buff = attribute_parent_get(context->world->database, thing, a->number,
                                  &aowner, &aflags);
      source = thing;
    }
    checkit = 0;

    if ((a->number == A_NAME) || (a->number == A_LENTER) ||
        read_attr(context, source, source, a, aowner, aflags)) {
      checkit = 1;
    }
    if (checkit) {
      buff2 = bp = alloc_lbuf("boolean_expression_evaluate");
      str = buff;
      exec(context, buff2, &bp, 0, source, player,
           EV_FIGNORE | EV_EVAL | EV_TOP, &str, (char **)nullptr, 0);
      *bp = '\0';
      checkit = !string_compare(context->world->configuration, buff2,
                                (char *)b->sub1);
      free_lbuf(buff2);
    }
    free_lbuf(buff);
    return checkit;
  case BOOLEXP_IS:

    /*
     * If an object check, do that
     */

    if (b->sub1->type == BOOLEXP_CONST)
      return (b->sub1->thing == player);

    /*
     * Nope, do an attribute check
     */

    a = attribute_by_number(context->world->database, (int)b->sub1->thing);
    if (!a)
      return 0;
    return (check_attr(context, player, from, a, (char *)(b->sub1)->sub1));
  case BOOLEXP_CARRY:

    /*
     * If an object check, do that
     */

    if (b->sub1->type == BOOLEXP_CONST)
      return (member(context->world->database, b->sub1->thing,
                     game_object_contents(context->world->database, player)));

    /*
     * Nope, do an attribute check
     */

    a = attribute_by_number(context->world->database, (int)b->sub1->thing);
    if (!a)
      return 0;
    DOLIST(context->world->database, obj,
           game_object_contents(context->world->database, player)) {
      if (check_attr(context, obj, from, a, (char *)(b->sub1)->sub1))
        return 1;
    }
    return 0;
  case BOOLEXP_OWNER:
    return (game_object_owner(context->world->database, b->sub1->thing) ==
            game_object_owner(context->world->database, player));
  default:
    abort(); /*
              * bad type
              */
    return 0;
  }
} /* end boolean_expression_evaluate() */

int eval_boolexp_atr(EvaluationContext *context, DbRef player, DbRef thing,
                     DbRef from, char *key) {
  BooleanExpression *b;
  int ret_value;

  b = boolean_expression_parse(context->world->database, context, player, key,
                               1);
  if (b == nullptr) {
    ret_value = 1;
  } else {
    ret_value = boolean_expression_evaluate(context, player, thing, from, b);
    boolean_expression_free(b);
  }
  return (ret_value);
} /* end eval_boolexp_atr() */

/*
 * If the parser returns TRUE_BOOLEXP, you lose
 */

/*
 * TRUE_BOOLEXP cannot be typed in by the user; use @unlock instead
 */

typedef struct BooleanParseContext BooleanParseContext;
struct BooleanParseContext {
  GameDatabase *database;
  const char *cursor;
  char storage[LBUF_SIZE];
  DbRef player;
  bool is_internal;
  MatchContext *match;
};

static void skip_whitespace(BooleanParseContext *context) {
  while (*context->cursor && isspace(*context->cursor))
    context->cursor++;
}

static BooleanExpression *parse_boolexp_E(BooleanParseContext *context);

static BooleanExpression *test_atr(BooleanParseContext *context, char *s) {
  Attribute *attrib;
  BooleanExpression *b;
  char *buff, *s1;
  int anum, locktype;

  buff = alloc_lbuf("test_atr");
  StringCopy(buff, s);
  for (s = buff; *s && (*s != ':') && (*s != '/'); s++)
    ;
  if (!*s) {
    free_lbuf(buff);
    return ((BooleanExpression *)nullptr);
  }
  if (*s == '/')
    locktype = BOOLEXP_EVAL;
  else
    locktype = BOOLEXP_ATR;

  *s++ = '\0';
  /*
   * see if left side is valid attribute.  Access to attr is checked on
   *
   * *  * *  * *  * *  * * eval * Also allow numeric references to *
   * attributes. * It * can't * hurt  * us, and * lets us import stuff
   * * that stores * attr * locks by * number * instead of by * name.
   */
  if (!(attrib = attribute_by_name(context->database, buff))) {

    /*
     * Only #1 can lock on numbers
     */
    if (!is_god(context->database, context->player)) {
      free_lbuf(buff);
      return ((BooleanExpression *)nullptr);
    }
    for (s1 = buff; isdigit(*s1); s1++)
      ;
    if (*s1) {
      free_lbuf(buff);
      return ((BooleanExpression *)nullptr);
    }
    anum = atoi(buff);
  } else {
    anum = attrib->number;
  }

  /*
   * made it now make the parse tree node
   */
  b = alloc_bool("test_str");
  b->type = (boolexp_type)locktype;
  b->thing = (DbRef)anum;
  b->sub1 = (BooleanExpression *)(void *)strsave(s);
  free_lbuf(buff);
  return (b);
} /* end test_atr() */

/*
 * L -> (E); L -> object identifier
 */
static BooleanExpression *parse_boolexp_L(BooleanParseContext *context) {
  BooleanExpression *b;
  char *p, *buf;
  MSTATE mstate;

  buf = nullptr;
  skip_whitespace(context);

  switch (*context->cursor) {
  case '(':
    context->cursor++;
    b = parse_boolexp_E(context);
    skip_whitespace(context);
    if (b == TRUE_BOOLEXP || *context->cursor++ != ')') {
      boolean_expression_free(b);
      return TRUE_BOOLEXP;
    }
    break;
  default:

    /*
     * Must have hit an object ref. Load the name into our
     * buffer
     */

    buf = alloc_lbuf("parse_boolexp_L");
    p = buf;
    while (*context->cursor && (*context->cursor != AND_TOKEN) &&
           (*context->cursor != OR_TOKEN) && (*context->cursor != ')')) {
      *p++ = *context->cursor++;
    }

    /*
     * strip trailing whitespace
     */

    *p = '\0';
    while (p > buf && isspace((unsigned char)p[-1]))
      *--p = '\0';

    /*
     * check for an attribute
     */

    if ((b = test_atr(context, buf)) != nullptr) {
      free_lbuf(buf);
      return (b);
    }
    b = alloc_bool("parse_boolexp_L");
    b->type = BOOLEXP_CONST;

    /*
     * do the match
     */

    /*
     * If we are parsing a boolexp that was a stored lock then
     * we know that object refs are all dbrefs, so we
     * skip the expensive match code.
     */

    if (context->is_internal) {
      if (buf[0] != '#') {
        free_lbuf(buf);
        free_bool(b);
        return TRUE_BOOLEXP;
      }
      b->thing = atoi(&buf[1]);
      if (!is_good_obj(context->database, b->thing)) {
        free_lbuf(buf);
        free_bool(b);
        return TRUE_BOOLEXP;
      }
    } else {
      MatchContext *match = context->match;
      save_match_state(match, &mstate);
      init_match(match, context->player, buf, TYPE_THING);
      match_everything(match, MAT_EXIT_PARENTS);
      b->thing = match_result(match);
      restore_match_state(match, &mstate);
    }

    if (b->thing == NOTHING) {
      notify_printf(context->match->evaluation, context->player,
                    "I don't see %s here.", buf);
      free_lbuf(buf);
      free_bool(b);
      return TRUE_BOOLEXP;
    }
    if (b->thing == AMBIGUOUS) {
      notify_printf(context->match->evaluation, context->player,
                    "I don't know which %s you mean!", buf);
      free_lbuf(buf);
      free_bool(b);
      return TRUE_BOOLEXP;
    }
    free_lbuf(buf);
  }
  return b;
} /* end parse_boolexp_L() */

/*
 * F -> !F; F -> @L; F -> =L; F -> +L; F -> $L
 */

/*
 * The argument L must be type BOOLEXP_CONST
 */

static BooleanExpression *parse_boolexp_F(BooleanParseContext *context) {
  BooleanExpression *b2;

  skip_whitespace(context);
  switch (*context->cursor) {
  case NOT_TOKEN:
    context->cursor++;
    b2 = alloc_bool("parse_boolexp_F.not");
    b2->type = BOOLEXP_NOT;
    if ((b2->sub1 = parse_boolexp_F(context)) == TRUE_BOOLEXP) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else
      return (b2);
    /*
     * NOTREACHED
     */
    break;
  case INDIR_TOKEN:
    context->cursor++;
    b2 = alloc_bool("parse_boolexp_F.indir");
    b2->type = BOOLEXP_INDIR;
    b2->sub1 = parse_boolexp_L(context);
    if ((b2->sub1) == TRUE_BOOLEXP || (b2->sub1->type) != BOOLEXP_CONST) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else
      return (b2);
    /*
     * NOTREACHED
     */
    break;
  case IS_TOKEN:
    context->cursor++;
    b2 = alloc_bool("parse_boolexp_F.is");
    b2->type = BOOLEXP_IS;
    b2->sub1 = parse_boolexp_L(context);
    if ((b2->sub1) == TRUE_BOOLEXP || (((b2->sub1->type) != BOOLEXP_CONST) &&
                                       ((b2->sub1->type) != BOOLEXP_ATR))) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else
      return (b2);
    /*
     * NOTREACHED
     */
    break;
  case CARRY_TOKEN:
    context->cursor++;
    b2 = alloc_bool("parse_boolexp_F.carry");
    b2->type = BOOLEXP_CARRY;
    b2->sub1 = parse_boolexp_L(context);
    if ((b2->sub1) == TRUE_BOOLEXP || (((b2->sub1->type) != BOOLEXP_CONST) &&
                                       ((b2->sub1->type) != BOOLEXP_ATR))) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else
      return (b2);
    /*
     * NOTREACHED
     */
    break;
  case OWNER_TOKEN:
    context->cursor++;
    b2 = alloc_bool("parse_boolexp_F.owner");
    b2->type = BOOLEXP_OWNER;
    b2->sub1 = parse_boolexp_L(context);
    if ((b2->sub1) == TRUE_BOOLEXP || (b2->sub1->type) != BOOLEXP_CONST) {
      boolean_expression_free(b2);
      return (TRUE_BOOLEXP);
    } else
      return (b2);
    /*
     * NOTREACHED
     */
    break;
  default:
    return (parse_boolexp_L(context));
  }
} /* end parse_boolexp_F() */

/*
 * T -> F; T -> F & T
 */

static BooleanExpression *parse_boolexp_T(BooleanParseContext *context) {
  BooleanExpression *b, *b2;

  if ((b = parse_boolexp_F(context)) != TRUE_BOOLEXP) {
    skip_whitespace(context);
    if (*context->cursor == AND_TOKEN) {
      context->cursor++;

      b2 = alloc_bool("parse_boolexp_T");
      b2->type = BOOLEXP_AND;
      b2->sub1 = b;
      if ((b2->sub2 = parse_boolexp_T(context)) == TRUE_BOOLEXP) {
        boolean_expression_free(b2);
        return TRUE_BOOLEXP;
      }
      b = b2;
    }
  }
  return b;
} /* end parse_boolexp_T() */

/*
 * E -> T; E -> T | E
 */
static BooleanExpression *parse_boolexp_E(BooleanParseContext *context) {
  BooleanExpression *b, *b2;

  if ((b = parse_boolexp_T(context)) != TRUE_BOOLEXP) {
    skip_whitespace(context);
    if (*context->cursor == OR_TOKEN) {
      context->cursor++;

      b2 = alloc_bool("parse_boolexp_E");
      b2->type = BOOLEXP_OR;
      b2->sub1 = b;
      if ((b2->sub2 = parse_boolexp_E(context)) == TRUE_BOOLEXP) {
        boolean_expression_free(b2);
        return TRUE_BOOLEXP;
      }
      b = b2;
    }
  }
  return b;
} /* end parse_boolexp_E() */

BooleanExpression *boolean_expression_parse(GameDatabase *database,
                                            EvaluationContext *evaluation,
                                            DbRef player, const char *buf,
                                            int internal) {
  if ((buf == nullptr) || (*buf == '\0'))
    return (TRUE_BOOLEXP);
  BooleanParseContext context = {
      .database = database,
      .player = player,
      .is_internal = internal != 0,
      .match = evaluation != nullptr ? &evaluation->command->match : nullptr};

  StringCopy(context.storage, buf);
  context.cursor = context.storage;
  return parse_boolexp_E(&context);
} /* end boolean_expression_parse() */

typedef enum BooleanExpressionUnparseFormat {
  BOOLEXP_UNPARSE_EXAMINE,
  BOOLEXP_UNPARSE_QUIET,
  BOOLEXP_UNPARSE_FUNCTION,
} BooleanExpressionUnparseFormat;

typedef struct BooleanUnparseContext BooleanUnparseContext;
struct BooleanUnparseContext {
  GameDatabase *database;
  EvaluationContext *evaluation;
  char *buffer;
  char *top;
  char object_buffer[SBUF_SIZE];
};

static const char *
boolean_expression_unparse_object_quiet(BooleanUnparseContext *context,
                                        DbRef object) {
  switch (object) {
  case NOTHING:
    return "-1";
  case HOME:
    return "-3";
  default:
    snprintf(context->object_buffer, sizeof(context->object_buffer), "(#%ld)",
             object);
    return context->object_buffer;
  }
}

static void boolean_expression_unparse_internal(
    BooleanUnparseContext *context, DbRef player, BooleanExpression *expression,
    char outer_type, BooleanExpressionUnparseFormat format) {
  Attribute *attribute;
  char *attribute_number, separator;
  char *buffer;

  if (expression == TRUE_BOOLEXP) {
    if (format == BOOLEXP_UNPARSE_EXAMINE)
      safe_str("*UNLOCKED*", context->buffer, &context->top);
    return;
  }

  switch (expression->type) {
  case BOOLEXP_AND:
    if (outer_type == BOOLEXP_NOT)
      safe_chr('(', context->buffer, &context->top);
    boolean_expression_unparse_internal(context, player, expression->sub1,
                                        expression->type, format);
    safe_chr(AND_TOKEN, context->buffer, &context->top);
    boolean_expression_unparse_internal(context, player, expression->sub2,
                                        expression->type, format);
    if (outer_type == BOOLEXP_NOT)
      safe_chr(')', context->buffer, &context->top);
    break;
  case BOOLEXP_OR:
    if (outer_type == BOOLEXP_NOT || outer_type == BOOLEXP_AND)
      safe_chr('(', context->buffer, &context->top);
    boolean_expression_unparse_internal(context, player, expression->sub1,
                                        expression->type, format);
    safe_chr(OR_TOKEN, context->buffer, &context->top);
    boolean_expression_unparse_internal(context, player, expression->sub2,
                                        expression->type, format);
    if (outer_type == BOOLEXP_NOT || outer_type == BOOLEXP_AND)
      safe_chr(')', context->buffer, &context->top);
    break;
  case BOOLEXP_NOT:
    safe_chr('!', context->buffer, &context->top);
    boolean_expression_unparse_internal(context, player, expression->sub1,
                                        expression->type, format);
    break;
  case BOOLEXP_INDIR:
    safe_chr(INDIR_TOKEN, context->buffer, &context->top);
    boolean_expression_unparse_internal(context, player, expression->sub1,
                                        expression->type, format);
    break;
  case BOOLEXP_IS:
    safe_chr(IS_TOKEN, context->buffer, &context->top);
    boolean_expression_unparse_internal(context, player, expression->sub1,
                                        expression->type, format);
    break;
  case BOOLEXP_CARRY:
    safe_chr(CARRY_TOKEN, context->buffer, &context->top);
    boolean_expression_unparse_internal(context, player, expression->sub1,
                                        expression->type, format);
    break;
  case BOOLEXP_OWNER:
    safe_chr(OWNER_TOKEN, context->buffer, &context->top);
    boolean_expression_unparse_internal(context, player, expression->sub1,
                                        expression->type, format);
    break;
  case BOOLEXP_CONST:
    if (format == BOOLEXP_UNPARSE_QUIET) {
      safe_str(
          boolean_expression_unparse_object_quiet(context, expression->thing),
          context->buffer, &context->top);
    } else if (format == BOOLEXP_UNPARSE_EXAMINE) {
      buffer = unparse_object(context->database, context->evaluation, player,
                              expression->thing, 0);
      safe_str(buffer, context->buffer, &context->top);
      free_lbuf(buffer);
    } else {
      if (typeof_obj(context->database, expression->thing) == TYPE_PLAYER) {
        safe_chr('*', context->buffer, &context->top);
        safe_str(game_object_name(context->database, expression->thing),
                 context->buffer, &context->top);
      } else {
        buffer = alloc_sbuf("boolean_expression_unparse_internal");
        snprintf(buffer, SBUF_SIZE, "#%ld", expression->thing);
        safe_str(buffer, context->buffer, &context->top);
        free_sbuf(buffer);
      }
    }
    break;
  case BOOLEXP_ATR:
  case BOOLEXP_EVAL:
    separator = expression->type == BOOLEXP_EVAL ? '/' : ':';
    attribute = attribute_by_number(context->database, (int)expression->thing);
    if (attribute && attribute->number) {
      safe_str(attribute->name, context->buffer, &context->top);
      safe_chr(separator, context->buffer, &context->top);
      safe_str((char *)expression->sub1, context->buffer, &context->top);
    } else if (expression->thing > 0) {
      attribute_number =
          alloc_sbuf("boolean_expression_unparse_internal.attribute_number");
      snprintf(attribute_number, SBUF_SIZE, "%ld", expression->thing);
      safe_str(attribute_number, context->buffer, &context->top);
      safe_chr(separator, context->buffer, &context->top);
      safe_str((char *)expression->sub1, context->buffer, &context->top);
      free_sbuf(attribute_number);
    } else {
      safe_str((char *)expression->sub2, context->buffer, &context->top);
      safe_chr(separator, context->buffer, &context->top);
      safe_str((char *)expression->sub1, context->buffer, &context->top);
    }
    break;
  default:
    fprintf(stderr, "Fell off the end of switch in "
                    "boolean_expression_unparse_internal()\n");
    abort();
  }
}

static void boolean_expression_unparse_format(
    GameDatabase *database, EvaluationContext *evaluation,
    char buffer[LBUF_SIZE], DbRef player, BooleanExpression *expression,
    BooleanExpressionUnparseFormat format) {
  BooleanUnparseContext context = {.database = database,
                                   .evaluation = evaluation,
                                   .buffer = buffer,
                                   .top = buffer};

  boolean_expression_unparse_internal(&context, player, expression,
                                      BOOLEXP_CONST, format);
  *context.top = '\0';
}

void boolean_expression_unparse_quiet(GameDatabase *database,
                                      EvaluationContext *evaluation,
                                      char buffer[LBUF_SIZE], DbRef player,
                                      BooleanExpression *expression) {
  boolean_expression_unparse_format(database, evaluation, buffer, player,
                                    expression, BOOLEXP_UNPARSE_QUIET);
}

void boolean_expression_unparse(GameDatabase *database,
                                EvaluationContext *evaluation,
                                char buffer[LBUF_SIZE], DbRef player,
                                BooleanExpression *expression) {
  boolean_expression_unparse_format(database, evaluation, buffer, player,
                                    expression, BOOLEXP_UNPARSE_EXAMINE);
}

void boolean_expression_unparse_function(GameDatabase *database,
                                         EvaluationContext *evaluation,
                                         char buffer[LBUF_SIZE], DbRef player,
                                         BooleanExpression *expression) {
  boolean_expression_unparse_format(database, evaluation, buffer, player,
                                    expression, BOOLEXP_UNPARSE_FUNCTION);
}
