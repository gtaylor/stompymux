/*
 * walkdb.c -- Support for commands that walk the entire db
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/commands/functions.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"
#include "mux/world/search.h"
#include "mux/world/walkdb.h"

#ifdef MCHECK
#endif

/**
 * Bind occurances of the universal var in ACTION to ARG, then run ACTION.
 * Cmds run in low-prio Q after a 1 sec delay for the first one.
 */
static void bind_and_queue(DbRef player, DbRef cause, char *action,
                           char *argstr, char *cargs[], int ncargs,
                           int number) {
  char *command,
      *command2; /*

                                          * allocated by replace_string
                                          */

  command = replace_string(BOUND_VAR, argstr, action),
  command2 = replace_string(LISTPLACE_VAR, tprintf("%d", number), command);
  wait_que(player, cause, 0, NOTHING, 0, command2, cargs, ncargs,
           mudstate.global_regs);
  free_lbuf(command);
  free_lbuf(command2);
}

/**
 * Iterates through a delimited string. Used in @dolist.
 */
void do_dolist(DbRef player, DbRef cause, int key, char *list, char *command,
               char *cargs[], int ncargs) {
  char *curr, *objstring, delimiter = ' ';
  int number = 0;

  if (!list || *list == '\0') {
    notify(player, "That's terrific, but what should I do with the list?");
    return;
  }
  curr = list;

  if (key == DOLIST_DELIMIT) {
    char *tempstr;

    if (strlen((tempstr = parse_to(&curr, ' ', EV_STRIP))) > 1) {
      notify(player, "The delimiter must be a single character!");
      return;
    }
    delimiter = *tempstr;
  }
  while (curr && *curr) {
    while (*curr == delimiter)
      curr++;
    if (*curr) {
      number++;
      objstring = parse_to(&curr, delimiter, EV_STRIP);
      bind_and_queue(player, cause, command, objstring, cargs, ncargs, number);
    }
  }
}

/**
 * Regular @find command
 */
void do_find(DbRef player, DbRef cause, int key, char *name) {
  DbRef i, low_bound, high_bound;
  char *buff;

  parse_range(&name, &low_bound, &high_bound);
  for (i = low_bound; i <= high_bound; i++) {
    if ((typeof_obj(i) != TYPE_EXIT) && is_controls(player, i) &&
        (!*name || string_match(PureName(i), name))) {
      buff = unparse_object(player, i, 0);
      notify(player, buff);
      free_lbuf(buff);
    }
  }
  notify(player, "***End of List***");
}

/**
 * Get counts of items in the db.
 */
int database_statistics_get(DbRef player, DbRef who, DatabaseStatistics *info) {
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

  if (is_good_obj(who) && !is_controls(player, who) && !is_wizard(player)) {
    notify(player, "Permission denied.");
    return 0;
  }
  DO_WHOLE_DB(i) {
    if ((who == NOTHING) || (who == obj_owner(i))) {
      info->s_total++;
      if (is_going(i) && (typeof_obj(i) != TYPE_ROOM)) {
        info->s_garbage++;
        continue;
      }
      switch (typeof_obj(i)) {
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
void do_stats(DbRef player, DbRef cause, int key, char *name) {
  DbRef owner;
  DatabaseStatistics statinfo;

  switch (key) {
  case STAT_ALL:
    owner = NOTHING;
    break;
  case STAT_ME:
    owner = obj_owner(player);
    break;
  case STAT_PLAYER:
    if (!(name && *name)) {
      notify_printf(player, "The universe contains %d objects.",
                    mudstate.db_top);
      return;
    }
    owner = lookup_player(player, name, 1);
    if (owner == NOTHING) {
      notify(player, "Not found.");
      return;
    }
    break;
  default:
    notify(player, "Illegal combination of switches.");
    return;
  }

  if (!database_statistics_get(player, owner, &statinfo))
    return;
  notify_printf(
      player,
      "%d objects = %d rooms, %d exits, %d things, %d players. (%d garbage)",
      statinfo.s_total, statinfo.s_rooms, statinfo.s_exits, statinfo.s_things,
      statinfo.s_players, statinfo.s_garbage);

#ifdef MCHECK
  if (is_wizard(player)) {
    struct mstats mval;

    mval = mstats();
    notify_printf(player, "Total size of the heap: %d", mval.bytes_total);
    notify_printf(player,
                  "Chunks allocated: %d -- Total size of allocated chunks: %d",
                  mval.chunks_used, mval.bytes_used);
    notify_printf(player, "Chunks free: %d -- Total size of free chunks: %d",
                  mval.chunks_free, mval.bytes_free);
  }
#endif /*                                                                      \
        * MCHECK                                                               \
        */
}

/**
 * Transfers ownership of all a player's objects to another player.
 */
int chown_all(DbRef from_player, DbRef to_player) {
  int i, count;

  if (typeof_obj(from_player) != TYPE_PLAYER)
    from_player = obj_owner(from_player);
  if (typeof_obj(to_player) != TYPE_PLAYER)
    to_player = obj_owner(to_player);
  count = 0;
  DO_WHOLE_DB(i) {
    if ((obj_owner(i) == from_player) && (obj_owner(i) != i)) {
      switch (typeof_obj(i)) {
      case TYPE_PLAYER:
        s_owner(i, i);
        break;
      case TYPE_THING:
      case TYPE_ROOM:
      case TYPE_EXIT:
      default:
        s_owner(i, to_player);
      }
      s_flags(i, (obj_flags(i) & ~INHERIT) | HALT);
      count++;
    }
  }
  return count;
}

/**
 * Transfers ownership of all a player's objects to another player.
 * Used in @chownall
 */
void do_chownall(DbRef player, DbRef cause, int key, char *from, char *to) {
  int count;
  DbRef victim, recipient;

  init_match(player, from, TYPE_PLAYER);
  match_neighbor();
  match_absolute();
  match_player();
  if ((victim = noisy_match_result()) == NOTHING)
    return;

  if ((to != nullptr) && *to) {
    init_match(player, to, TYPE_PLAYER);
    match_neighbor();
    match_absolute();
    match_player();
    if ((recipient = noisy_match_result()) == NOTHING)
      return;
  } else {
    recipient = player;
  }

  count = chown_all(victim, recipient);
  if (!is_quiet(player)) {
    notify_printf(player, "%d objects @chowned.", count);
  }
}

#define ANY_OWNER -2

/**
 * Walk the db reporting various things (or setting/clearing
 * mark bits)
 */
int search_criteria_setup(DbRef player, char *searchfor, SearchCriteria *parm) {
  char *pname, *searchtype, *t;
  int err;

  /*
   * Crack arg into <pname> <type>=<targ>,<low>,<high>
   */

  /* pname/searchtype are mutated in place elsewhere in this function
     (lowercased, split, null-terminated); they can't be const, so these
     literal defaults need an explicit cast. */
#ifdef __clang__
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
#endif
  pname = parse_to(&searchfor, '=', EV_STRIP_TS);
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
#ifdef __clang__
#pragma clang diagnostic pop
#endif

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

  parse_range(&searchfor, &parm->low_bound, &parm->high_bound);

  /*
   * set limits on who we search
   */

  parm->s_owner = obj_owner(player);
  parm->s_wizard = is_wizard(player);
  parm->s_rst_owner = NOTHING;
  if (!*pname) {
    parm->s_rst_owner = parm->s_wizard ? ANY_OWNER : player;
  } else if (pname[0] == '#') {
    parm->s_rst_owner = atoi(&pname[1]);
    if (!is_good_obj(parm->s_rst_owner) ||
        typeof_obj(parm->s_rst_owner) != TYPE_PLAYER)
      parm->s_rst_owner = NOTHING;

  } else if (strcmp(pname, "me") == 0) {
    parm->s_rst_owner = player;
  } else {
    parm->s_rst_owner = lookup_player(player, pname, 1);
  }

  if (parm->s_rst_owner == NOTHING) {
    notify_printf(player, "%s: No such player", pname);
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

      if (!convert_flags(player, searchfor, &parm->s_fset, &parm->s_rst_type))
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
      parm->s_parent = match_controlled(player, searchfor);
      if (!is_good_obj(parm->s_parent))
        return 0;
      if (!*pname)
        parm->s_rst_owner = ANY_OWNER;
    } else if (string_prefix("power", searchtype)) {
      if (!decode_power(player, searchfor, &parm->s_pset))
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
        notify_printf(player, "%s: unknown type", searchfor);
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
      parm->s_zone = match_controlled(player, searchfor);
      if (!is_good_obj(parm->s_zone))
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
    notify_printf(player, "%s: unknown class", searchtype);
    return 0;
  }
  /*
   * Make sure player is authorized to do the search
   */

  if (!parm->s_wizard && (parm->s_rst_type != TYPE_PLAYER) &&
      (parm->s_rst_owner != player) && (parm->s_rst_owner != ANY_OWNER)) {
    notify(player, "You need a search warrant to do that!");
    return 0;
  }
  return 1;
}

void search_criteria_perform(DbRef player, DbRef cause, SearchCriteria *parm) {
  Flag thing1flags, thing2flags, thing3flags;
  Power thing1powers, thing2powers;
  DbRef thing;
  char *buff, *buff2, *result, *bp, *str;
  int save_invk_ctr;

  buff = alloc_sbuf("search_criteria_perform.num");
  save_invk_ctr = mudstate.func_invk_ctr;

  for (thing = parm->low_bound; thing <= parm->high_bound; thing++) {
    mudstate.func_invk_ctr = save_invk_ctr;

    /*
     * Check for matching type
     */

    if ((parm->s_rst_type != NOTYPE) && (parm->s_rst_type != typeof_obj(thing)))
      continue;

    /*
     * Check for matching owner
     */

    if ((parm->s_rst_owner != ANY_OWNER) &&
        (parm->s_rst_owner != obj_owner(thing)))
      continue;

    /*
     * Check for matching parent
     */

    if ((parm->s_parent != NOTHING) && (parm->s_parent != obj_parent(thing)))
      continue;

    /*
     * Check for matching zone
     */

    if ((parm->s_zone != NOTHING) && (parm->s_zone != obj_zone(thing)))
      continue;

    /*
     * Check for matching flags
     */

    thing3flags = obj_flags3(thing);
    thing2flags = obj_flags2(thing);
    thing1flags = obj_flags(thing);
    if ((thing1flags & parm->s_fset.word1) != parm->s_fset.word1)
      continue;
    if ((thing2flags & parm->s_fset.word2) != parm->s_fset.word2)
      continue;
    if ((thing3flags & parm->s_fset.word3) != parm->s_fset.word3)
      continue;

    /*
     * Check for matching power
     */

    thing1powers = obj_powers(thing);
    thing2powers = obj_powers2(thing);
    if ((thing1powers & parm->s_pset.word1) != parm->s_pset.word1)
      continue;
    if ((thing2powers & parm->s_pset.word2) != parm->s_pset.word2)
      continue;

    /*
     * Check for matching name
     */

    if (parm->s_rst_name != nullptr) {
      if (!string_prefix((char *)PureName(thing), parm->s_rst_name))
        continue;
    }
    /*
     * Check for successful evaluation
     */

    if (parm->s_rst_eval != nullptr) {
      if (typeof_obj(thing) == TYPE_GARBAGE)
        continue;
      snprintf(buff, SBUF_SIZE, "#%ld", thing);
      buff2 = replace_string(BOUND_VAR, buff, parm->s_rst_eval);
      result = bp = alloc_lbuf("search_criteria_perform");
      str = buff2;
      exec(result, &bp, 0, player, cause, EV_FCHECK | EV_EVAL | EV_NOTRACE,
           &str, (char **)nullptr, 0);
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

    olist_add(thing);
  }
  free_sbuf(buff);
  mudstate.func_invk_ctr = save_invk_ctr;
}

void do_search(DbRef player, DbRef cause, int key, char *arg) {
  int flag, destitute;
  int rcount, ecount, tcount, pcount, gcount;
  char *buff, *outbuf, *bp;
  DbRef thing, from, to;
  SearchCriteria searchparm;

  if (!search_criteria_setup(player, arg, &searchparm))
    return;
  olist_push();
  search_criteria_perform(player, cause, &searchparm);
  destitute = 1;

  outbuf = alloc_lbuf("do_search.outbuf");

  rcount = ecount = tcount = pcount = gcount = 0;

  /*
   * room search
   */
  if (searchparm.s_rst_type == TYPE_ROOM || searchparm.s_rst_type == NOTYPE) {
    flag = 1;
    for (thing = olist_first(); thing != NOTHING; thing = olist_next()) {
      if (typeof_obj(thing) != TYPE_ROOM)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(player, "\nROOMS:");
      }
      buff = unparse_object(player, thing, 0);
      notify(player, buff);
      free_lbuf(buff);
      rcount++;
    }
  }
  /*
   * exit search
   */
  if (searchparm.s_rst_type == TYPE_EXIT || searchparm.s_rst_type == NOTYPE) {
    flag = 1;
    for (thing = olist_first(); thing != NOTHING; thing = olist_next()) {
      if (typeof_obj(thing) != TYPE_EXIT)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(player, "\nEXITS:");
      }
      from = obj_exits(thing);
      to = obj_location(thing);

      bp = outbuf;
      buff = unparse_object(player, thing, 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);

      safe_str(" [from ", outbuf, &bp);
      buff = unparse_object(player, from, 0);
      safe_str(((from == NOTHING) ? "NOWHERE" : buff), outbuf, &bp);
      free_lbuf(buff);

      safe_str(" to ", outbuf, &bp);
      buff = unparse_object(player, to, 0);
      safe_str(((to == NOTHING) ? "NOWHERE" : buff), outbuf, &bp);
      free_lbuf(buff);

      safe_chr(']', outbuf, &bp);
      *bp = '\0';
      notify(player, outbuf);
      ecount++;
    }
  }
  /*
   * object search
   */
  if (searchparm.s_rst_type == TYPE_THING || searchparm.s_rst_type == NOTYPE) {
    flag = 1;
    for (thing = olist_first(); thing != NOTHING; thing = olist_next()) {
      if (typeof_obj(thing) != TYPE_THING)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(player, "\nOBJECTS:");
      }
      bp = outbuf;
      buff = unparse_object(player, thing, 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);

      safe_str(" [owner: ", outbuf, &bp);
      buff = unparse_object(player, obj_owner(thing), 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);

      safe_chr(']', outbuf, &bp);
      *bp = '\0';
      notify(player, outbuf);
      tcount++;
    }
  }
  /*
   * garbage search
   */
  if (searchparm.s_rst_type == TYPE_GARBAGE ||
      searchparm.s_rst_type == NOTYPE) {
    flag = 1;
    for (thing = olist_first(); thing != NOTHING; thing = olist_next()) {
      if (typeof_obj(thing) != TYPE_GARBAGE)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(player, "\nGARBAGE:");
      }
      bp = outbuf;
      buff = unparse_object(player, thing, 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);

      safe_str(" [owner: ", outbuf, &bp);
      buff = unparse_object(player, obj_owner(thing), 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);

      safe_chr(']', outbuf, &bp);
      *bp = '\0';
      notify(player, outbuf);
      gcount++;
    }
  }
  /*
   * player search
   */
  if (searchparm.s_rst_type == TYPE_PLAYER || searchparm.s_rst_type == NOTYPE) {
    flag = 1;
    for (thing = olist_first(); thing != NOTHING; thing = olist_next()) {
      if (typeof_obj(thing) != TYPE_PLAYER)
        continue;
      if (flag) {
        flag = 0;
        destitute = 0;
        notify(player, "\nPLAYERS:");
      }
      bp = outbuf;
      buff = unparse_object(player, thing, 0);
      safe_str(buff, outbuf, &bp);
      free_lbuf(buff);
      if (searchparm.s_wizard) {
        safe_str(" [location: ", outbuf, &bp);
        buff = unparse_object(player, obj_location(thing), 0);
        safe_str(buff, outbuf, &bp);
        free_lbuf(buff);
        safe_chr(']', outbuf, &bp);
      }
      *bp = '\0';
      notify(player, outbuf);
      pcount++;
    }
  }
  /*
   * if nothing found matching search criteria
   */

  if (destitute) {
    notify(player, "Nothing found.");
  } else {
    snprintf(outbuf, LBUF_SIZE,
             "\nFound:  Rooms...%d  Exits...%d  Objects...%d  Players...%d  "
             "Garbage...%d",
             rcount, ecount, tcount, pcount, gcount);
    notify(player, outbuf);
  }
  free_lbuf(outbuf);
  olist_pop();
}

/**
 * Object list management routines:
 * olist_push, olist_pop, olist_add, olist_first, olist_next
 */

/**
 * Create a new object list at the top of the object list stack
 */
void olist_push(void) {
  OLSTK *ol;

  ol = (OLSTK *)XMALLOC(sizeof(OLSTK), "olist_push");
  ol->next = mudstate.olist;
  mudstate.olist = ol;

  ol->head = nullptr;
  ol->tail = nullptr;
  ol->cblock = nullptr;
  ol->count = 0;
  ol->citm = 0;
}

/**
 * Pop one entire list off the object list stack
 */
void olist_pop(void) {
  OLSTK *ol;
  OBLOCK *op, *onext;

  ol = mudstate.olist->next;

  for (op = mudstate.olist->head; op != nullptr; op = onext) {
    onext = op->next;
    free_lbuf(op);
  }
  XFREE(mudstate.olist, "olist_pop");
  mudstate.olist = ol;
}

/**
 * Add an entry to the object list
 */
void olist_add(DbRef item) {
  OBLOCK *op;

  if (!mudstate.olist->head) {
    op = (OBLOCK *)alloc_lbuf("olist_add.first");
    mudstate.olist->head = mudstate.olist->tail = op;
    mudstate.olist->count = 0;
    op->next = nullptr;
  } else if ((size_t)mudstate.olist->count >= OBLOCK_SIZE) {
    op = (OBLOCK *)alloc_lbuf("olist_add.next");
    mudstate.olist->tail->next = op;
    mudstate.olist->tail = op;
    mudstate.olist->count = 0;
    op->next = nullptr;
  } else {
    op = mudstate.olist->tail;
  }
  op->data[mudstate.olist->count++] = item;
}

/**
 * Return the first entry in the object list
 */
DbRef olist_first(void) {
  if (!mudstate.olist->head)
    return NOTHING;
  if ((mudstate.olist->head == mudstate.olist->tail) &&
      (mudstate.olist->count == 0))
    return NOTHING;
  mudstate.olist->cblock = mudstate.olist->head;
  mudstate.olist->citm = 0;
  return mudstate.olist->cblock->data[mudstate.olist->citm++];
}

DbRef olist_next(void) {
  DbRef thing;

  if (!mudstate.olist->cblock)
    return NOTHING;
  if ((mudstate.olist->cblock == mudstate.olist->tail) &&
      (mudstate.olist->citm >= mudstate.olist->count))
    return NOTHING;
  thing = mudstate.olist->cblock->data[mudstate.olist->citm++];
  if ((size_t)mudstate.olist->citm >= OBLOCK_SIZE) {
    mudstate.olist->cblock = mudstate.olist->cblock->next;
    mudstate.olist->citm = 0;
  }
  return thing;
}
