/*
 * walkdb.c -- Support for commands that walk the entire db
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/commands/command_helpers.h"
#include "mux/commands/command_invocation.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "mux/world/match.h"
#include "mux/world/search.h"
#include "mux/world/walkdb.h"
#include "mux/world/world_context.h"

/**
 * Regular @find command
 */
void do_find(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  WorldContext *world = invocation->context->world;
  DbRef i, low_bound, high_bound;
  char *buff;

  parse_range(world->database, world->configuration, &name, &low_bound,
              &high_bound);
  for (i = low_bound; i <= high_bound; i++) {
    if ((typeof_obj(evaluation->world->database, i) != OBJECT_TYPE_EXIT) &&
        is_controls(evaluation->world->database, player, i) &&
        (!*name ||
         string_match(game_object_pure_name(evaluation->world->database, i),
                      name))) {
      buff = unparse_object(evaluation->world->database, evaluation, player, i);
      notify(evaluation, player, buff);
      free_lbuf(buff);
    }
  }
  notify(evaluation, player, "***End of List***");
}

/**
 * Get counts of items in the db.
 */
void database_statistics_get(GameDatabase *database, DatabaseStatistics *info) {
  DbRef i;

  info->s_total = 0;
  info->s_rooms = 0;
  info->s_exits = 0;
  info->s_things = 0;
  info->s_players = 0;
  info->s_garbage = 0;

  DO_WHOLE_DB(database, i) {
    info->s_total++;
    if (is_going(database, i) && typeof_obj(database, i) != OBJECT_TYPE_ROOM) {
      info->s_garbage++;
      continue;
    }
    switch (typeof_obj(database, i)) {
    case OBJECT_TYPE_ROOM:
      info->s_rooms++;
      break;
    case OBJECT_TYPE_EXIT:
      info->s_exits++;
      break;
    case OBJECT_TYPE_THING:
      info->s_things++;
      break;
    case OBJECT_TYPE_PLAYER:
      info->s_players++;
      break;
    default:
      info->s_garbage++;
    }
  }
}

/*
 * Get counts of items in the db.
 */
void do_stats(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  DatabaseStatistics statinfo;

  database_statistics_get(evaluation->world->database, &statinfo);
  notify_printf(
      &invocation->context->evaluation, player,
      "%d objects = %d rooms, %d exits, %d things, %d players. (%d garbage)",
      statinfo.s_total, statinfo.s_rooms, statinfo.s_exits, statinfo.s_things,
      statinfo.s_players, statinfo.s_garbage);
}

/**
 * Walk the db reporting various things (or setting/clearing
 * mark bits)
 */
int search_criteria_setup(EvaluationContext *context, DbRef player,
                          char *searchfor, SearchCriteria *parm) {
  char *searchtype, *t;
  int err;

  /* Split <type>=<target>,<low>,<high>. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  searchtype = parse_to(context->world->configuration, &searchfor, '=',
                        COMMAND_PARSE_STRIP_TRAILING);
  if (!searchtype)
    searchtype = (char *)"";
  if (!searchfor)
    searchfor = (char *)"";
  for (t = searchtype; *t; t++) {
    if (isupper(*t))
      *t = (char)tolower(*t);
  }
#pragma clang diagnostic pop
  /*
   * Strip any range arguments
   */

  parse_range(context->world->database, context->world->configuration,
              &searchfor, &parm->low_bound, &parm->high_bound);

  parm->s_wizard = is_wizard(context->world->database, player);
  /*
   * set limits on what we search for
   */

  err = 0;
  parm->s_rst_name = nullptr;
  parm->s_rst_type = OBJECT_TYPE_NOTYPE;
  parm->s_zone = NOTHING;
  parm->s_fset = (ObjectFlagSet){0};
  parm->s_power = POWER_NONE;

  switch (searchtype[0]) {
  case '\0': /*
              * the no class requested class  :)
              */
    break;
  case 'e':
    if (string_prefix("exits", searchtype)) {
      parm->s_rst_name = searchfor;
      parm->s_rst_type = OBJECT_TYPE_EXIT;
    } else {
      err = 1;
    }
    break;
  case 'f':
    if (string_prefix("flags", searchtype)) {

      /*
       * convert_flags ignores previous values of flag_mask
       * * * * * and s_rst_type while setting them
       */

      if (!convert_flags(context, player, searchfor, &parm->s_fset,
                         &parm->s_rst_type))
        return 0;
    } else {
      err = 1;
    }
    break;
  case 'n':
    if (string_prefix("name", searchtype)) {
      parm->s_rst_name = searchfor;
    } else {
      err = 1;
    }
    break;
  case 'o':
    if (string_prefix("objects", searchtype)) {
      parm->s_rst_name = searchfor;
      parm->s_rst_type = OBJECT_TYPE_THING;
    } else {
      err = 1;
    }
    break;
  case 'p':
    if (string_prefix("players", searchtype)) {
      parm->s_rst_name = searchfor;
      parm->s_rst_type = OBJECT_TYPE_PLAYER;
    } else if (string_prefix("power", searchtype)) {
      if (!decode_power(context, context->world->indexes, player, searchfor,
                        &parm->s_power))
        return 0;
    } else {
      err = 1;
    }
    break;
  case 'r':
    if (string_prefix("rooms", searchtype)) {
      parm->s_rst_name = searchfor;
      parm->s_rst_type = OBJECT_TYPE_ROOM;
    } else {
      err = 1;
    }
    break;
  case 't':
    if (string_prefix("type", searchtype)) {
      if (searchfor[0] == '\0')
        break;
      if (string_prefix("rooms", searchfor))
        parm->s_rst_type = OBJECT_TYPE_ROOM;
      else if (string_prefix("exits", searchfor))
        parm->s_rst_type = OBJECT_TYPE_EXIT;
      else if (string_prefix("objects", searchfor) ||
               string_prefix("things", searchfor))
        parm->s_rst_type = OBJECT_TYPE_THING;
      else if (string_prefix("garbage", searchfor))
        parm->s_rst_type = OBJECT_TYPE_GARBAGE;
      else if (string_prefix("players", searchfor))
        parm->s_rst_type = OBJECT_TYPE_PLAYER;
      else {
        notify_printf(context, player, "%s: unknown type", searchfor);
        return 0;
      }
    } else if (string_prefix("things", searchtype)) {
      parm->s_rst_name = searchfor;
      parm->s_rst_type = OBJECT_TYPE_THING;
    } else {
      err = 1;
    }
    break;
  case 'z':
    if (string_prefix("zone", searchtype)) {
      parm->s_zone = match_thing(&context->command->match, player, searchfor);
      if (!is_good_obj(context->world->database, parm->s_zone))
        return 0;
    } else {
      err = 1;
    }
    break;
  default:
    err = 1;
  }

  if (err) {
    notify_printf(context, player, "%s: unknown class", searchtype);
    return 0;
  }
  return 1;
}

void search_criteria_perform(EvaluationContext *context, DbRef player,
                             DbRef cause, SearchCriteria *parm,
                             ObjectList *results) {
  DbRef thing;
  (void)cause;

  for (thing = parm->low_bound; thing <= parm->high_bound; thing++) {
    /*
     * Check for matching type
     */

    if ((parm->s_rst_type != OBJECT_TYPE_NOTYPE) &&
        (parm->s_rst_type != typeof_obj(context->world->database, thing)))
      continue;

    /*
     * Check for matching zone
     */

    if ((parm->s_zone != NOTHING) &&
        (parm->s_zone != game_object_zone(context->world->database, thing)))
      continue;

    /*
     * Check for matching flags
     */

    for (ObjectFlag flag = OBJECT_FLAG_ANSI; flag < OBJECT_FLAG_COUNT; flag++)
      if (object_flag_set_has(&parm->s_fset, flag) &&
          !game_object_has_flag(context->world->database, thing, flag))
        goto next_object;

    /*
     * Check for matching power
     */

    if (parm->s_power != POWER_NONE &&
        !game_object_has_power(context->world->database, thing, parm->s_power))
      continue;

    /*
     * Check for matching name
     */

    if (parm->s_rst_name != nullptr) {
      if (!string_prefix(
              (char *)game_object_pure_name(context->world->database, thing),
              parm->s_rst_name))
        continue;
    }
    /*
     * It passed everything.  Amazing.
     */

    object_list_add(results, thing);
  next_object:;
  }
}

void do_search(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  DbRef cause = invocation->cause;
  char *arg = invocation->first;
  int flag, destitute;
  int rcount, ecount, tcount, pcount, gcount;
  char *buff, *outbuf, *bp;
  DbRef thing, from, to;
  SearchCriteria searchparm;
  ObjectList results;

  if (!search_criteria_setup(evaluation, player, arg, &searchparm))
    return;
  object_list_initialize(&results);
  search_criteria_perform(evaluation, player, cause, &searchparm, &results);
  destitute = 1;

  outbuf = alloc_lbuf("do_search.outbuf");

  rcount = ecount = tcount = pcount = gcount = 0;

  /*
   * room search
   */
  if (searchparm.s_rst_type == OBJECT_TYPE_ROOM ||
      searchparm.s_rst_type == OBJECT_TYPE_NOTYPE) {
    flag = 1;
    for (thing = object_list_first(&results); thing != NOTHING;
         thing = object_list_next(&results)) {
      if (typeof_obj(evaluation->world->database, thing) != OBJECT_TYPE_ROOM)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(evaluation, player, "\nROOMS:");
      }
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing);
      notify(evaluation, player, buff);
      free_lbuf(buff);
      rcount++;
    }
  }
  /*
   * exit search
   */
  if (searchparm.s_rst_type == OBJECT_TYPE_EXIT ||
      searchparm.s_rst_type == OBJECT_TYPE_NOTYPE) {
    flag = 1;
    for (thing = object_list_first(&results); thing != NOTHING;
         thing = object_list_next(&results)) {
      if (typeof_obj(evaluation->world->database, thing) != OBJECT_TYPE_EXIT)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(evaluation, player, "\nEXITS:");
      }
      from = game_object_exits(evaluation->world->database, thing);
      to = game_object_location(evaluation->world->database, thing);

      bp = outbuf;
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);

      safe_str(" [from ", outbuf, &bp);
      buff =
          unparse_object(evaluation->world->database, evaluation, player, from);
      safe_str(((from == NOTHING) ? "NOWHERE" : buff), outbuf, &bp);
      free_lbuf(buff);

      safe_str(" to ", outbuf, &bp);
      buff =
          unparse_object(evaluation->world->database, evaluation, player, to);
      safe_str(((to == NOTHING) ? "NOWHERE" : buff), outbuf, &bp);
      free_lbuf(buff);

      safe_chr(']', outbuf, &bp);
      *bp = '\0';
      notify(evaluation, player, outbuf);
      ecount++;
    }
  }
  /*
   * object search
   */
  if (searchparm.s_rst_type == OBJECT_TYPE_THING ||
      searchparm.s_rst_type == OBJECT_TYPE_NOTYPE) {
    flag = 1;
    for (thing = object_list_first(&results); thing != NOTHING;
         thing = object_list_next(&results)) {
      if (typeof_obj(evaluation->world->database, thing) != OBJECT_TYPE_THING)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(evaluation, player, "\nOBJECTS:");
      }
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing);
      notify(evaluation, player, buff);
      free_lbuf(buff);
      tcount++;
    }
  }
  /*
   * garbage search
   */
  if (searchparm.s_rst_type == OBJECT_TYPE_GARBAGE ||
      searchparm.s_rst_type == OBJECT_TYPE_NOTYPE) {
    flag = 1;
    for (thing = object_list_first(&results); thing != NOTHING;
         thing = object_list_next(&results)) {
      if (typeof_obj(evaluation->world->database, thing) != OBJECT_TYPE_GARBAGE)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(evaluation, player, "\nGARBAGE:");
      }
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing);
      notify(evaluation, player, buff);
      free_lbuf(buff);
      gcount++;
    }
  }
  /*
   * player search
   */
  if (searchparm.s_rst_type == OBJECT_TYPE_PLAYER ||
      searchparm.s_rst_type == OBJECT_TYPE_NOTYPE) {
    flag = 1;
    for (thing = object_list_first(&results); thing != NOTHING;
         thing = object_list_next(&results)) {
      if (typeof_obj(evaluation->world->database, thing) != OBJECT_TYPE_PLAYER)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(evaluation, player, "\nPLAYERS:");
      }
      bp = outbuf;
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);
      if (searchparm.s_wizard) {
        safe_str(" [location: ", outbuf, &bp);
        buff = unparse_object(
            evaluation->world->database, evaluation, player,
            game_object_location(evaluation->world->database, thing));
        safe_str(buff, outbuf, &bp);
        free_lbuf(buff);
        safe_chr(']', outbuf, &bp);
      }
      *bp = '\0';
      notify(evaluation, player, outbuf);
      pcount++;
    }
  }
  /*
   * if nothing found matching search criteria
   */

  if (destitute) {
    notify(evaluation, player, "Nothing found.");
  } else {
    snprintf(outbuf, LBUF_SIZE,
             "\nFound:  Rooms...%d  Exits...%d  Objects...%d  Players...%d  "
             "Garbage...%d",
             rcount, ecount, tcount, pcount, gcount);
    notify(evaluation, player, outbuf);
  }
  free_lbuf(outbuf);
  object_list_destroy(&results);
}

void object_list_initialize(ObjectList *list) { *list = (ObjectList){0}; }

void object_list_destroy(ObjectList *list) {
  ObjectListBlock *block = list->head;
  while (block != nullptr) {
    ObjectListBlock *next = block->next;
    free_lbuf(block);
    block = next;
  }
  object_list_initialize(list);
}

void object_list_add(ObjectList *list, DbRef item) {
  constexpr int block_capacity =
      (LBUF_SIZE - sizeof(ObjectListBlock *)) / sizeof(DbRef);
  ObjectListBlock *block;

  if (list->head == nullptr) {
    block = (ObjectListBlock *)alloc_lbuf("object_list_add.first");
    list->head = list->tail = block;
    list->count = 0;
    block->next = nullptr;
  } else if (list->count >= block_capacity) {
    block = (ObjectListBlock *)alloc_lbuf("object_list_add.next");
    list->tail->next = block;
    list->tail = block;
    list->count = 0;
    block->next = nullptr;
  } else {
    block = list->tail;
  }
  block->data[list->count++] = item;
}

DbRef object_list_first(ObjectList *list) {
  if (list->head == nullptr)
    return NOTHING;
  if (list->head == list->tail && list->count == 0)
    return NOTHING;
  list->cursor_block = list->head;
  list->cursor_index = 0;
  return list->cursor_block->data[list->cursor_index++];
}

DbRef object_list_next(ObjectList *list) {
  constexpr int block_capacity =
      (LBUF_SIZE - sizeof(ObjectListBlock *)) / sizeof(DbRef);
  DbRef thing;

  if (list->cursor_block == nullptr)
    return NOTHING;
  if (list->cursor_block == list->tail && list->cursor_index >= list->count)
    return NOTHING;
  thing = list->cursor_block->data[list->cursor_index++];
  if (list->cursor_index >= block_capacity) {
    list->cursor_block = list->cursor_block->next;
    list->cursor_index = 0;
  }
  return thing;
}
