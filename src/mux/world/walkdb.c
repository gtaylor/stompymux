/*
 * walkdb.c -- Support for commands that walk the entire db
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/commands/command_invocation.h"
#include "mux/commands/functions.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "mux/world/match.h"
#include "mux/world/search.h"
#include "mux/world/walkdb.h"
#include "mux/world/world_context.h"

/**
 * Bind occurances of the universal var in ACTION to ARG, then run ACTION.
 * Cmds run in low-prio Q after a 1 sec delay for the first one.
 */
static void bind_and_queue(EvaluationContext *evaluation, DbRef player,
                           DbRef cause, char *action, char *argstr,
                           char *cargs[], int ncargs, int number) {
  char *command,
      *command2; /*

                                          * allocated by replace_string
                                          */

  command = replace_string(BOUND_VAR, argstr, action),
  command2 = replace_string(LISTPLACE_VAR, tprintf("%d", number), command);
  wait_que(evaluation->runtime->commands, player, cause, 0, NOTHING, 0,
           command2, cargs, ncargs, evaluation->registers);
  free_lbuf(command);
  free_lbuf(command2);
}

/**
 * Iterates through a delimited string. Used in @dolist.
 */
void do_dolist(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  DbRef cause = invocation->cause;
  int key = invocation->key;
  char *list = invocation->first;
  char *command = invocation->second;
  char **cargs = invocation->command_arguments;
  int ncargs = invocation->command_argument_count;
  const ServerConfiguration *configuration = evaluation->world->configuration;
  char *curr, *objstring, delimiter = ' ';
  int number = 0;

  if (!list || *list == '\0') {
    notify(evaluation, player,
           "That's terrific, but what should I do with the list?");
    return;
  }
  curr = list;

  if (key == DOLIST_DELIMIT) {
    char *tempstr;

    if (strlen((tempstr = parse_to(configuration, &curr, ' ', EV_STRIP))) > 1) {
      notify(evaluation, player, "The delimiter must be a single character!");
      return;
    }
    delimiter = *tempstr;
  }
  while (curr && *curr) {
    while (*curr == delimiter)
      curr++;
    if (*curr) {
      number++;
      objstring = parse_to(configuration, &curr, delimiter, EV_STRIP);
      bind_and_queue(evaluation, player, cause, command, objstring, cargs,
                     ncargs, number);
    }
  }
}

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
    if ((typeof_obj(evaluation->world->database, i) != TYPE_EXIT) &&
        is_controls(evaluation, player, i) &&
        (!*name ||
         string_match(game_object_pure_name(evaluation->world->database, i),
                      name))) {
      buff =
          unparse_object(evaluation->world->database, evaluation, player, i, 0);
      notify(evaluation, player, buff);
      free_lbuf(buff);
    }
  }
  notify(evaluation, player, "***End of List***");
}

/**
 * Get counts of items in the db.
 */
int database_statistics_get(EvaluationContext *evaluation, DbRef player,
                            DbRef who, DatabaseStatistics *info) {
  DbRef i;

  info->s_total = 0;
  info->s_rooms = 0;
  info->s_exits = 0;
  info->s_things = 0;
  info->s_players = 0;
  info->s_garbage = 0;

  /*
   * Do we have permission?
   */

  if (is_good_obj(evaluation->world->database, who) &&
      !is_controls(evaluation, player, who) &&
      !is_wizard(evaluation->world->database, player)) {
    notify(evaluation, player, "Permission denied.");
    return 0;
  }
  DO_WHOLE_DB(evaluation->world->database, i) {
    if ((who == NOTHING) ||
        (who == game_object_owner(evaluation->world->database, i))) {
      info->s_total++;
      if (is_going(evaluation->world->database, i) &&
          (typeof_obj(evaluation->world->database, i) != TYPE_ROOM)) {
        info->s_garbage++;
        continue;
      }
      switch (typeof_obj(evaluation->world->database, i)) {
      case TYPE_ROOM:
        info->s_rooms++;
        break;
      case TYPE_EXIT:
        info->s_exits++;
        break;
      case TYPE_THING:
        info->s_things++;
        break;
      case TYPE_PLAYER:
        info->s_players++;
        break;
      default:
        info->s_garbage++;
      }
    }
  }
  return 1;
}

/*
 * Get counts of items in the db.
 */
void do_stats(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *name = invocation->first;
  WorldContext *world = invocation->context->world;
  DbRef owner;
  DatabaseStatistics statinfo;

  switch (key) {
  case STAT_ALL:
    owner = NOTHING;
    break;
  case STAT_ME:
    owner = game_object_owner(evaluation->world->database, player);
    break;
  case STAT_PLAYER:
    if (!(name && *name)) {
      notify_printf(&invocation->context->evaluation, player,
                    "The universe contains %d objects.", world->database->top);
      return;
    }
    owner = lookup_player(world, player, name, 1);
    if (owner == NOTHING) {
      notify(evaluation, player, "Not found.");
      return;
    }
    break;
  default:
    notify(evaluation, player, "Illegal combination of switches.");
    return;
  }

  if (!database_statistics_get(evaluation, player, owner, &statinfo))
    return;
  notify_printf(
      &invocation->context->evaluation, player,
      "%d objects = %d rooms, %d exits, %d things, %d players. (%d garbage)",
      statinfo.s_total, statinfo.s_rooms, statinfo.s_exits, statinfo.s_things,
      statinfo.s_players, statinfo.s_garbage);
}

/**
 * Transfers ownership of all a player's objects to another player.
 */
int chown_all(GameDatabase *database, DbRef from_player, DbRef to_player) {
  int i, count;

  if (typeof_obj(database, from_player) != TYPE_PLAYER)
    from_player = game_object_owner(database, from_player);
  if (typeof_obj(database, to_player) != TYPE_PLAYER)
    to_player = game_object_owner(database, to_player);
  count = 0;
  DO_WHOLE_DB(database, i) {
    if ((game_object_owner(database, i) == from_player) &&
        (game_object_owner(database, i) != i)) {
      switch (typeof_obj(database, i)) {
      case TYPE_PLAYER:
        game_object_set_owner(database, i, i);
        break;
      case TYPE_THING:
      case TYPE_ROOM:
      case TYPE_EXIT:
      default:
        game_object_set_owner(database, i, to_player);
      }
      game_object_set_flags(database, i,
                            (game_object_flags(database, i) & ~INHERIT) | HALT);
      count++;
    }
  }
  return count;
}

/**
 * Transfers ownership of all a player's objects to another player.
 * Used in @chownall
 */
void do_chownall(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  char *from = invocation->first;
  char *to = invocation->second;
  int count;
  DbRef victim, recipient;

  MatchContext *match = &invocation->context->match;
  init_match(match, player, from, TYPE_PLAYER);
  match_neighbor(match);
  match_absolute(match);
  match_player(match);
  if ((victim = noisy_match_result(match)) == NOTHING)
    return;

  if ((to != nullptr) && *to) {
    init_match(match, player, to, TYPE_PLAYER);
    match_neighbor(match);
    match_absolute(match);
    match_player(match);
    if ((recipient = noisy_match_result(match)) == NOTHING)
      return;
  } else {
    recipient = player;
  }

  count = chown_all(invocation->context->world->database, victim, recipient);
  if (!is_quiet(invocation->context->world->database, player)) {
    notify_printf(&invocation->context->evaluation, player,
                  "%d objects @chowned.", count);
  }
}

#define ANY_OWNER -2

/**
 * Walk the db reporting various things (or setting/clearing
 * mark bits)
 */
int search_criteria_setup(EvaluationContext *context, DbRef player,
                          char *searchfor, SearchCriteria *parm) {
  char *pname, *searchtype, *t;
  int err;

  /*
   * Crack arg into <pname> <type>=<targ>,<low>,<high>
   */

  /* pname/searchtype are mutated in place elsewhere in this function
     (lowercased, split, null-terminated); they can't be const, so these
     literal defaults need an explicit cast. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  pname = parse_to(context->world->configuration, &searchfor, '=', EV_STRIP_TS);
  if (!pname || !*pname) {
    pname = (char *)"me";
  } else
    for (t = pname; *t; t++) {
      if (isupper(*t))
        *t = (char)tolower(*t);
    }

  if (searchfor && *searchfor) {
    searchtype = (char *)rindex(pname, ' ');
    if (searchtype) {
      *searchtype++ = '\0';
    } else {
      searchtype = pname;
      pname = (char *)"";
    }
  } else {
    searchtype = (char *)"";
  }
#pragma clang diagnostic pop

  /*
   * If the player name is quoted, strip the quotes
   */

  if (*pname == '\"') {
    err = (int)strlen(pname) - 1;
    if (pname[err] == '"') {
      pname[err] = '\0';
      pname++;
    }
  }
  /*
   * Strip any range arguments
   */

  parse_range(context->world->database, context->world->configuration,
              &searchfor, &parm->low_bound, &parm->high_bound);

  /*
   * set limits on who we search
   */

  parm->s_owner = game_object_owner(context->world->database, player);
  parm->s_wizard = is_wizard(context->world->database, player);
  parm->s_rst_owner = NOTHING;
  if (!*pname) {
    parm->s_rst_owner = parm->s_wizard ? ANY_OWNER : player;
  } else if (pname[0] == '#') {
    parm->s_rst_owner = clamped_atol(&pname[1]);
    if (!is_good_obj(context->world->database, parm->s_rst_owner) ||
        typeof_obj(context->world->database, parm->s_rst_owner) != TYPE_PLAYER)
      parm->s_rst_owner = NOTHING;

  } else if (strcmp(pname, "me") == 0) {
    parm->s_rst_owner = player;
  } else {
    parm->s_rst_owner = lookup_player(context->world, player, pname, 1);
  }

  if (parm->s_rst_owner == NOTHING) {
    notify_printf(context, player, "%s: No such player", pname);
    return 0;
  }
  /*
   * set limits on what we search for
   */

  err = 0;
  parm->s_rst_name = nullptr;
  parm->s_rst_eval = nullptr;
  parm->s_rst_type = NOTYPE;
  parm->s_parent = NOTHING;
  parm->s_zone = NOTHING;
  parm->s_fset.word1 = 0;
  parm->s_fset.word2 = 0;
  parm->s_fset.word3 = 0;
  parm->s_pset.word1 = 0;
  parm->s_pset.word2 = 0;

  switch (searchtype[0]) {
  case '\0': /*
              * the no class requested class  :)
              */
    break;
  case 'e':
    if (string_prefix("exits", searchtype)) {
      parm->s_rst_name = searchfor;
      parm->s_rst_type = TYPE_EXIT;
    } else if (string_prefix("evaluate", searchtype)) {
      parm->s_rst_eval = searchfor;
    } else if (string_prefix("eplayer", searchtype)) {
      parm->s_rst_type = TYPE_PLAYER;
      parm->s_rst_eval = searchfor;
    } else if (string_prefix("eroom", searchtype)) {
      parm->s_rst_type = TYPE_ROOM;
      parm->s_rst_eval = searchfor;
    } else if (string_prefix("eobject", searchtype) ||
               string_prefix("ething", searchtype)) {
      parm->s_rst_type = TYPE_THING;
      parm->s_rst_eval = searchfor;
    } else if (string_prefix("eexit", searchtype)) {
      parm->s_rst_type = TYPE_EXIT;
      parm->s_rst_eval = searchfor;
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
      parm->s_rst_type = TYPE_THING;
    } else {
      err = 1;
    }
    break;
  case 'p':
    if (string_prefix("players", searchtype)) {
      parm->s_rst_name = searchfor;
      parm->s_rst_type = TYPE_PLAYER;
      if (!*pname)
        parm->s_rst_owner = ANY_OWNER;
    } else if (string_prefix("parent", searchtype)) {
      parm->s_parent =
          match_controlled(&context->command->match, player, searchfor);
      if (!is_good_obj(context->world->database, parm->s_parent))
        return 0;
      if (!*pname)
        parm->s_rst_owner = ANY_OWNER;
    } else if (string_prefix("power", searchtype)) {
      if (!decode_power(context, context->world->indexes, player, searchfor,
                        &parm->s_pset))
        return 0;
    } else {
      err = 1;
    }
    break;
  case 'r':
    if (string_prefix("rooms", searchtype)) {
      parm->s_rst_name = searchfor;
      parm->s_rst_type = TYPE_ROOM;
    } else {
      err = 1;
    }
    break;
  case 't':
    if (string_prefix("type", searchtype)) {
      if (searchfor[0] == '\0')
        break;
      if (string_prefix("rooms", searchfor))
        parm->s_rst_type = TYPE_ROOM;
      else if (string_prefix("exits", searchfor))
        parm->s_rst_type = TYPE_EXIT;
      else if (string_prefix("objects", searchfor) ||
               string_prefix("things", searchfor))
        parm->s_rst_type = TYPE_THING;
      else if (string_prefix("garbage", searchfor))
        parm->s_rst_type = TYPE_GARBAGE;
      else if (string_prefix("players", searchfor)) {
        parm->s_rst_type = TYPE_PLAYER;
        if (!*pname)
          parm->s_rst_owner = ANY_OWNER;
      } else {
        notify_printf(context, player, "%s: unknown type", searchfor);
        return 0;
      }
    } else if (string_prefix("things", searchtype)) {
      parm->s_rst_name = searchfor;
      parm->s_rst_type = TYPE_THING;
    } else {
      err = 1;
    }
    break;
  case 'z':
    if (string_prefix("zone", searchtype)) {
      parm->s_zone =
          match_controlled(&context->command->match, player, searchfor);
      if (!is_good_obj(context->world->database, parm->s_zone))
        return 0;
      if (!*pname)
        parm->s_rst_owner = ANY_OWNER;
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
  /*
   * Make sure player is authorized to do the search
   */

  if (!parm->s_wizard && (parm->s_rst_type != TYPE_PLAYER) &&
      (parm->s_rst_owner != player) && (parm->s_rst_owner != ANY_OWNER)) {
    notify(context, player, "You need a search warrant to do that!");
    return 0;
  }
  return 1;
}

void search_criteria_perform(EvaluationContext *context, DbRef player,
                             DbRef cause, SearchCriteria *parm,
                             ObjectList *results) {
  Flag thing1flags, thing2flags, thing3flags;
  Power thing1powers, thing2powers;
  DbRef thing;
  char *buff, *buff2, *result, *bp, *str;
  int save_invk_ctr;

  buff = alloc_sbuf("search_criteria_perform.num");
  save_invk_ctr = context->function_invocations;

  for (thing = parm->low_bound; thing <= parm->high_bound; thing++) {
    context->function_invocations = save_invk_ctr;

    /*
     * Check for matching type
     */

    if ((parm->s_rst_type != NOTYPE) &&
        (parm->s_rst_type != typeof_obj(context->world->database, thing)))
      continue;

    /*
     * Check for matching owner
     */

    if ((parm->s_rst_owner != ANY_OWNER) &&
        (parm->s_rst_owner !=
         game_object_owner(context->world->database, thing)))
      continue;

    /*
     * Check for matching parent
     */

    if ((parm->s_parent != NOTHING) &&
        (parm->s_parent != game_object_parent(context->world->database, thing)))
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

    thing3flags = game_object_flags3(context->world->database, thing);
    thing2flags = game_object_flags2(context->world->database, thing);
    thing1flags = game_object_flags(context->world->database, thing);
    if ((thing1flags & parm->s_fset.word1) != parm->s_fset.word1)
      continue;
    if ((thing2flags & parm->s_fset.word2) != parm->s_fset.word2)
      continue;
    if ((thing3flags & parm->s_fset.word3) != parm->s_fset.word3)
      continue;

    /*
     * Check for matching power
     */

    thing1powers = game_object_powers(context->world->database, thing);
    thing2powers = game_object_powers2(context->world->database, thing);
    if ((thing1powers & parm->s_pset.word1) != parm->s_pset.word1)
      continue;
    if ((thing2powers & parm->s_pset.word2) != parm->s_pset.word2)
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
     * Check for successful evaluation
     */

    if (parm->s_rst_eval != nullptr) {
      if (typeof_obj(context->world->database, thing) == TYPE_GARBAGE)
        continue;
      snprintf(buff, SBUF_SIZE, "#%ld", thing);
      buff2 = replace_string(BOUND_VAR, buff, parm->s_rst_eval);
      result = bp = alloc_lbuf("search_criteria_perform");
      str = buff2;
      exec(context, result, &bp, 0, player, cause,
           EV_FCHECK | EV_EVAL | EV_NOTRACE, &str, (char **)nullptr, 0);
      *bp = '\0';
      free_lbuf(buff2);
      if (!*result || !xlate(result)) {
        free_lbuf(result);
        continue;
      }
      free_lbuf(result);
    }
    /*
     * It passed everything.  Amazing.
     */

    object_list_add(results, thing);
  }
  free_sbuf(buff);
  context->function_invocations = save_invk_ctr;
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
  if (searchparm.s_rst_type == TYPE_ROOM || searchparm.s_rst_type == NOTYPE) {
    flag = 1;
    for (thing = object_list_first(&results); thing != NOTHING;
         thing = object_list_next(&results)) {
      if (typeof_obj(evaluation->world->database, thing) != TYPE_ROOM)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(evaluation, player, "\nROOMS:");
      }
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing, 0);
      notify(evaluation, player, buff);
      free_lbuf(buff);
      rcount++;
    }
  }
  /*
   * exit search
   */
  if (searchparm.s_rst_type == TYPE_EXIT || searchparm.s_rst_type == NOTYPE) {
    flag = 1;
    for (thing = object_list_first(&results); thing != NOTHING;
         thing = object_list_next(&results)) {
      if (typeof_obj(evaluation->world->database, thing) != TYPE_EXIT)
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
                            thing, 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);

      safe_str(" [from ", outbuf, &bp);
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            from, 0);
      safe_str(((from == NOTHING) ? "NOWHERE" : buff), outbuf, &bp);
      free_lbuf(buff);

      safe_str(" to ", outbuf, &bp);
      buff = unparse_object(evaluation->world->database, evaluation, player, to,
                            0);
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
  if (searchparm.s_rst_type == TYPE_THING || searchparm.s_rst_type == NOTYPE) {
    flag = 1;
    for (thing = object_list_first(&results); thing != NOTHING;
         thing = object_list_next(&results)) {
      if (typeof_obj(evaluation->world->database, thing) != TYPE_THING)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(evaluation, player, "\nOBJECTS:");
      }
      bp = outbuf;
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing, 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);

      safe_str(" [owner: ", outbuf, &bp);
      buff = unparse_object(
          evaluation->world->database, evaluation, player,
          game_object_owner(evaluation->world->database, thing), 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);

      safe_chr(']', outbuf, &bp);
      *bp = '\0';
      notify(evaluation, player, outbuf);
      tcount++;
    }
  }
  /*
   * garbage search
   */
  if (searchparm.s_rst_type == TYPE_GARBAGE ||
      searchparm.s_rst_type == NOTYPE) {
    flag = 1;
    for (thing = object_list_first(&results); thing != NOTHING;
         thing = object_list_next(&results)) {
      if (typeof_obj(evaluation->world->database, thing) != TYPE_GARBAGE)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(evaluation, player, "\nGARBAGE:");
      }
      bp = outbuf;
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing, 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);

      safe_str(" [owner: ", outbuf, &bp);
      buff = unparse_object(
          evaluation->world->database, evaluation, player,
          game_object_owner(evaluation->world->database, thing), 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);

      safe_chr(']', outbuf, &bp);
      *bp = '\0';
      notify(evaluation, player, outbuf);
      gcount++;
    }
  }
  /*
   * player search
   */
  if (searchparm.s_rst_type == TYPE_PLAYER || searchparm.s_rst_type == NOTYPE) {
    flag = 1;
    for (thing = object_list_first(&results); thing != NOTHING;
         thing = object_list_next(&results)) {
      if (typeof_obj(evaluation->world->database, thing) != TYPE_PLAYER)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(evaluation, player, "\nPLAYERS:");
      }
      bp = outbuf;
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing, 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);
      if (searchparm.s_wizard) {
        safe_str(" [location: ", outbuf, &bp);
        buff = unparse_object(
            evaluation->world->database, evaluation, player,
            game_object_location(evaluation->world->database, thing), 0);
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
