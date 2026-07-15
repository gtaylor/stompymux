/*
 * builder_commands.c -- Commands that create and configure world objects
 */

#include "mux/commands/command.h"

#include "p.glue.h"

#include "mux/server/platform.h"

#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/world/match.h"
#include "mux/world/object.h"
#include "mux/world/object_set.h"

extern NameTable indiv_attraccess_nametab[];
extern void lower_xp(DbRef, int);
extern char *silly_atr_get(int id, int flag);
extern void silly_atr_set(int id, int flag, char *dat);

/*
 * ---------------------------------------------------------------------------
 * * parse_linkable_room: Get a location to link to.
 */

static DbRef parse_linkable_room(DbRef player, char *room_name) {
  DbRef room;

  init_match(player, room_name, NOTYPE);
  match_everything(MAT_NO_EXITS | MAT_NUMERIC | MAT_HOME);
  room = match_result();

  /*
   * HOME is always linkable
   */

  if (room == HOME)
    return HOME;

  /*
   * Make sure we can link to it
   */

  if (!Good_obj(room)) {
    notify_quiet(player, "That's not a valid object.");
    return NOTHING;
  } else if (!Has_contents(room) || !Linkable(player, room)) {
    notify_quiet(player, "You can't link to that.");
    return NOTHING;
  } else {
    return room;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * open_exit, do_open: Open a new exit and optionally link it somewhere.
 */

static void open_exit(DbRef player, DbRef loc, char *direction, char *linkto) {
  DbRef exit;

  if (!Good_obj(loc))
    return;

  if (!direction || !*direction) {
    notify_quiet(player, "Open where?");
    return;
  } else if (!controls(player, loc)) {
    notify_quiet(player, "Permission denied.");
    return;
  }
  exit = create_obj(player, TYPE_EXIT, direction);
  if (exit == NOTHING)
    return;

  /*
   * Initialize everything and link it in.
   */

  s_Exits(exit, loc);
  s_Next(exit, Exits(loc));
  s_Exits(loc, exit);

  /*
   * and we're done
   */

  notify_quiet(player, "Opened.");

  /*
   * See if we should do a link
   */

  if (!linkto || !*linkto)
    return;

  loc = parse_linkable_room(player, linkto);
  if (loc != NOTHING) {

    /*
     * Make sure the player passes the link lock
     */

    if (!could_doit(player, loc, A_LLINK)) {
      notify_quiet(player, "You can't link to there.");
      return;
    }
    s_Location(exit, loc);
    notify_quiet(player, "Linked.");
  }
}

void do_open(DbRef player, DbRef cause, int key, char *direction, char *links[],
             int nlinks) {
  DbRef loc, destnum;
  char *dest;

  /*
   * Create the exit and link to the destination, if there is one
   */

  if (nlinks >= 1)
    dest = links[0];
  else
    dest = NULL;

  if (key == OPEN_INVENTORY)
    loc = player;
  else
    loc = Location(player);

  open_exit(player, loc, direction, dest);

  /*
   * Open the back link if we can
   */

  if (nlinks >= 2) {
    destnum = parse_linkable_room(player, dest);
    if (destnum != NOTHING) {
      open_exit(player, destnum, links[1], tprintf("%d", loc));
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * link_exit, do_link: Set destination(exits), dropto(rooms) or
 * * home(player,thing)
 */

static void link_exit(DbRef player, DbRef exit, DbRef dest) {

  /*
   * Make sure we can link there
   */

  if ((dest != HOME) &&
      (!controls(player, dest) || !could_doit(player, dest, A_LLINK))) {
    notify_quiet(player, "Permission denied.");
    return;
  }
  /*
   * Exit must be unlinked or controlled by you
   */

  if ((Location(exit) != NOTHING) && !controls(player, exit)) {
    notify_quiet(player, "Permission denied.");
    return;
  }
  if (Owner(exit) != Owner(player)) {
    s_Owner(exit, Owner(player));
    s_Flags(exit, (Flags(exit) & ~(INHERIT | WIZARD)) | HALT);
  }
  /*
   * link has been validated and paid for, do it and tell the player
   */

  s_Location(exit, dest);
  if (!Quiet(player))
    notify_quiet(player, "Linked.");
}

void do_link(DbRef player, DbRef cause, int key, char *what, char *where) {
  DbRef thing, room;

  /*
   * Find the thing to link
   */

  init_match(player, what, TYPE_EXIT);
  match_everything(0);
  thing = noisy_match_result();
  if (thing == NOTHING)
    return;

  /*
   * Allow unlink if where is not specified
   */

  if (!where || !*where) {
    do_unlink(player, cause, key, what);
    return;
  }
  switch (Typeof(thing)) {
  case TYPE_EXIT:

    /*
     * Set destination
     */

    room = parse_linkable_room(player, where);
    if (room != NOTHING)
      link_exit(player, thing, room);
    break;
  case TYPE_PLAYER:
  case TYPE_THING:

    /*
     * Set home
     */

    if (!Controls(player, thing)) {
      notify_quiet(player, "Permission denied.");
      break;
    }
    init_match(player, where, NOTYPE);
    match_everything(MAT_NO_EXITS);
    room = noisy_match_result();
    if (!Good_obj(room))
      break;
    if (!Has_contents(room)) {
      notify_quiet(player, "Can't link to an exit.");
      break;
    }
    if (!can_set_home(player, thing, room) ||
        !could_doit(player, room, A_LLINK)) {
      notify_quiet(player, "Permission denied.");
    } else if (room == HOME) {
      notify_quiet(player, "Can't set home to home.");
    } else {
      s_Home(thing, room);
      if (!Quiet(player))
        notify_quiet(player, "Home set.");
    }
    break;
  case TYPE_ROOM:

    /*
     * Set dropto
     */

    if (!Controls(player, thing)) {
      notify_quiet(player, "Permission denied.");
      break;
    }
    room = parse_linkable_room(player, where);
    if (!(Good_obj(room) || (room == HOME)))
      break;

    if ((room != HOME) && !isRoom(room)) {
      notify_quiet(player, "That is not a room!");
    } else if ((room != HOME) && (!controls(player, room) ||
                                  !could_doit(player, room, A_LLINK))) {
      notify_quiet(player, "Permission denied.");
    } else {
      s_Dropto(thing, room);
      if (!Quiet(player))
        notify_quiet(player, "Dropto set.");
    }
    break;
  case TYPE_GARBAGE:
    notify_quiet(player, "Permission denied.");
    break;
  default:
    log_error(LOG_BUGS, "BUG", "OTYPE", "Strange object type: object #%d = %d",
              thing, Typeof(thing));
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_parent: Set an object's parent field.
 */

void do_parent(DbRef player, DbRef cause, int key, char *tname, char *pname) {
  DbRef thing, parent, curr;
  int lev;

  /*
   * get victim
   */

  init_match(player, tname, NOTYPE);
  match_everything(0);
  thing = noisy_match_result();
  if (thing == NOTHING)
    return;

  /*
   * Make sure we can do it
   */

  if (!Controls(player, thing)) {
    notify_quiet(player, "Permission denied.");
    return;
  }
  /*
   * Find out what the new parent is
   */

  if (*pname) {
    init_match(player, pname, Typeof(thing));
    match_everything(0);
    parent = noisy_match_result();
    if (parent == NOTHING)
      return;

    /*
     * Make sure we have rights to set parent
     */

    if (!Parentable(player, parent)) {
      notify_quiet(player, "Permission denied.");
      return;
    }
    /*
     * Verify no recursive reference
     */

    ITER_PARENTS(parent, curr, lev) {
      if (curr == thing) {
        notify_quiet(player, "You can't have yourself as a parent!");
        return;
      }
    }
  } else {
    parent = NOTHING;
  }

  s_Parent(thing, parent);
  if (!Quiet(thing) && !Quiet(player)) {
    if (parent == NOTHING)
      notify_quiet(player, "Parent cleared.");
    else
      notify_quiet(player, "Parent set.");
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_dig: Create a new room.
 */

void do_dig(DbRef player, DbRef cause, int key, char *name, char *args[],
            int nargs) {
  DbRef room;
  char *buff;

  /*
   * we don't need to know player's location!  hooray!
   */

  if (!name || !*name) {
    notify_quiet(player, "Dig what?");
    return;
  }
  room = create_obj(player, TYPE_ROOM, name);
  if (room == NOTHING)
    return;

  notify_printf(player, "%s created with room number %d.", name, room);

  buff = alloc_sbuf("do_dig");
  if ((nargs >= 1) && args[0] && *args[0]) {
    snprintf(buff, SBUF_SIZE, "%ld", room);
    open_exit(player, Location(player), args[0], buff);
  }
  if ((nargs >= 2) && args[1] && *args[1]) {
    snprintf(buff, SBUF_SIZE, "%ld", Location(player));
    open_exit(player, room, args[1], buff);
  }
  free_sbuf(buff);
  if (key == DIG_TELEPORT)
    (void)move_via_teleport(player, room, cause, 0);
}

/*
 * ---------------------------------------------------------------------------
 * * do_create: Make a new object.
 */

void do_create(DbRef player, DbRef cause, int key, char *name, char *coststr) {
  DbRef thing;
  char clearbuffer[MBUF_SIZE];

  (void)coststr;
  strip_ansi_r(clearbuffer, name, MBUF_SIZE);
  if (!name || !*name || (strlen(clearbuffer) == 0)) {
    notify_quiet(player, "Create what?");
    return;
  }
  thing = create_obj(player, TYPE_THING, name);
  if (thing == NOTHING)
    return;

  move_via_generic(thing, player, NOTHING, 0);
  s_Home(thing, new_home(player));
  if (!Quiet(player)) {
    notify_printf(player, "%s created as object #%d", Name(thing), thing);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_clone: Create a copy of an object.
 */

void do_clone(DbRef player, DbRef cause, int key, char *name, char *arg2) {
  DbRef clone, thing, new_owner, loc;
  Flag rmv_flags;

  if ((key & CLONE_INVENTORY) || !Has_location(player))
    loc = player;
  else
    loc = Location(player);

  if (!Good_obj(loc))
    return;

  init_match(player, name, NOTYPE);
  match_everything(0);
  thing = noisy_match_result();
  if ((thing == NOTHING) || (thing == AMBIGUOUS))
    return;

  /* Cloning requires examination permission. */

  if (!Examinable(player, thing)) {
    notify_quiet(player, "Permission denied.");
    return;
  }
  if (isPlayer(thing)) {
    notify_quiet(player, "You cannot clone players!");
    return;
  }
  /*
   * You can only make a parent link to what you control
   */

  if (!Controls(player, thing) && (key & CLONE_PARENT)) {
    notify_quiet(player, tprintf("You don't control %s, ignoring /parent.",
                                 Name(thing)));
    key &= ~CLONE_PARENT;
  }
  new_owner = (key & CLONE_PRESERVE) ? Owner(thing) : Owner(player);
  if ((Typeof(thing) == TYPE_EXIT) && !Controls(player, loc)) {
    notify_quiet(player, "Permission denied.");
    return;
  }

  /*
   * Go make the clone object
   */

  if ((arg2 && *arg2) && ok_name(arg2))
    clone = create_obj(new_owner, Typeof(thing), arg2);
  else
    clone = create_obj(new_owner, Typeof(thing), Name(thing));
  if (clone == NOTHING)
    return;

  /*
   * Wipe out any old attributes and copy in the new data
   */

  attribute_free(clone);
  if (key & CLONE_PARENT)
    s_Parent(clone, thing);
  else
    attribute_copy(player, clone, thing);

  /*
   * Reset the name, since we cleared the attributes
   */

  if ((arg2 && *arg2) && ok_name(arg2))
    object_name_set(clone, arg2);
  else
    object_name_set(clone, Name(thing));

  /*
   * Clear out problem flags from the original
   */

  rmv_flags = WIZARD;
  if (!(key & CLONE_INHERIT) || (!Inherits(player)))
    rmv_flags |= INHERIT;
  s_Flags(clone, Flags(thing) & ~rmv_flags);

  /*
   * Tell creator about it
   */

  if (!Quiet(player)) {
    if (arg2 && *arg2)
      notify_printf(player, "%s cloned as %s, new copy is object #%d.",
                    Name(thing), arg2, clone);
    else
      notify_printf(player, "%s cloned, new copy is object #%d.", Name(thing),
                    clone);
  }
  /*
   * Put the new thing in its new home.  Break any dropto or link, then
   * * * * * * * try to re-establish it.
   */

  switch (Typeof(thing)) {
  case TYPE_THING:
    s_Home(clone, clone_home(player, thing));
    move_via_generic(clone, loc, player, 0);
    break;
  case TYPE_ROOM:
    s_Dropto(clone, NOTHING);
    if (Dropto(thing) != NOTHING)
      link_exit(player, clone, Dropto(thing));
    break;
  case TYPE_EXIT:
    s_Exits(loc, insert_first(Exits(loc), clone));
    s_Exits(clone, loc);
    s_Location(clone, NOTHING);
    if (Location(thing) != NOTHING)
      link_exit(player, clone, Location(thing));
    break;
  }

  /*
   * If same owner run ACLONE, else halt it.  Also copy parent * if we
   * * * * * * can
   */

  if (new_owner == Owner(thing)) {
    if (!(key & CLONE_PARENT))
      s_Parent(clone, Parent(thing));
    did_it(player, clone, 0, NULL, 0, NULL, A_ACLONE, (char **)NULL, 0);
  } else {
    if (!(key & CLONE_PARENT) && Controls(player, thing))
      s_Parent(clone, Parent(thing));
    s_Halted(clone);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_pcreate: Create new players and robots.
 */

void do_pcreate(DbRef player, DbRef cause, int key, char *name, char *pass) {
  int isrobot;
  DbRef newplayer;

  isrobot = (key == PCRE_ROBOT) ? 1 : 0;
  newplayer = create_player(name, pass, player, isrobot);
  if (newplayer == NOTHING) {
    notify_quiet(player, tprintf("Failure creating '%s'", name));
    return;
  }
  if (isrobot) {
    move_object(newplayer, Location(player));
    notify_quiet(player,
                 tprintf("New robot '%s' (#%d) created with password '%s'",
                         name, newplayer, pass));

    notify_quiet(player, "Your robot has arrived.");
    STARTLOG(LOG_PCREATES, "CRE", "ROBOT") {
      log_name(newplayer);
      log_text((char *)" created by ");
      log_name(player);
      ENDLOG;
    }
  } else {
    move_object(newplayer, mudconf.start_room);
    notify_quiet(player,
                 tprintf("New player '%s' (#%d) created with password '%s'",
                         name, newplayer, pass));

    STARTLOG(LOG_PCREATES | LOG_WIZARD, "WIZ", "PCREA") {
      log_name(newplayer);
      log_text((char *)" created by ");
      log_name(player);
      ENDLOG;
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * can_destroy_exit, can_destroy_player, do_destroy:
 * * Destroy things.
 */

static int can_destroy_exit(DbRef player, DbRef exit) {
  DbRef loc;

  loc = Exits(exit);
  if ((loc != Location(player)) && (loc != player) && !Wizard(player)) {
    notify_quiet(player, "You can not destroy exits in another room.");
    return 0;
  }
  return 1;
}

/*
 * ---------------------------------------------------------------------------
 * * destroyable: Indicates if target of a @destroy is a 'special' object in
 * * the database.
 */

static int destroyable(DbRef victim) {
  if ((victim == mudconf.default_home) || (victim == mudconf.start_home) ||
      (victim == mudconf.start_room) || (victim == mudconf.master_room) ||
      (victim == (DbRef)0) || (God(victim)))
    return 0;
  return 1;
}

static int can_destroy_player(DbRef player, DbRef victim) {
  if (!Wizard(player)) {
    notify_quiet(player, "Sorry, no suicide allowed.");
    return 0;
  }
  if (Wizard(victim)) {
    notify_quiet(player, "You may not destroy Wizards!");
    return 0;
  }
  return 1;
}

void do_destroy(DbRef player, DbRef cause, int key, char *what) {
  DbRef thing;

  /*
   * You can destroy anything you control
   */

  thing = match_controlled_quiet(player, what);

  /*
   * If you own a location, you can destroy its exits
   */

  if ((thing == NOTHING) && controls(player, Location(player))) {
    init_match(player, what, TYPE_EXIT);
    match_exit();
    thing = last_match_result();
  }
  /*
   * Return an error if we didn't find anything to destroy
   */

  if (match_status(player, thing) == NOTHING) {
    return;
  }
  if (Safe(thing, player) && !(key & DEST_OVERRIDE)) {
    notify_quiet(player, "Sorry, that object is protected. Use "
                         "@destroy/override to destroy it.");
    return;
  }
  /*
   * Make sure we're not trying to destroy a special object
   */

  if (!destroyable(thing)) {
    notify_quiet(player, "You can't destroy that!");
    return;
  }
  /*
   * Go do it
   */

  switch (Typeof(thing)) {
  case TYPE_EXIT:
    if (can_destroy_exit(player, thing)) {
      if (Going(thing)) {
        notify_quiet(player, "No sense beating a dead exit.");
      } else {
        if (Hardcode(thing)) {
          DisposeSpecialObject(player, thing);
          c_Hardcode(thing);
        }
        if (0) {
          destroy_exit(thing);
        } else {
          notify(player, "The exit shakes and begins to crumble.");
          if (!Quiet(thing) && !Quiet(Owner(thing)))
            notify_quiet(Owner(thing),
                         tprintf("You will be rewarded shortly for %s(#%d).",
                                 Name(thing), thing));
          if ((Owner(thing) != player) && !Quiet(player))
            notify_quiet(player, tprintf("Destroyed. #%d's %s(#%d)",
                                         Owner(thing), Name(thing), thing));
          s_Going(thing);
        }
      }
    }
    break;
  case TYPE_THING:
    if (Going(thing)) {
      notify_quiet(player, "No sense beating a dead object.");
    } else {
      if (Hardcode(thing)) {
        DisposeSpecialObject(player, thing);
        c_Hardcode(thing);
      }
      if (0) {
        destroy_thing(thing);
      } else {
        notify(player, "The object shakes and begins to crumble.");
        if (!Quiet(thing) && !Quiet(Owner(thing)))
          notify_quiet(Owner(thing),
                       tprintf("You will be rewarded shortly for %s(#%d).",
                               Name(thing), thing));
        if ((Owner(thing) != player) && !Quiet(player))
          notify_quiet(player, tprintf("Destroyed. %s's %s(#%d)",
                                       Name(Owner(thing)), Name(thing), thing));
        s_Going(thing);
      }
    }
    break;
  case TYPE_PLAYER:
    if (can_destroy_player(player, thing)) {
      if (Going(thing)) {
        notify_quiet(player, "No sense beating a dead player.");
      } else {
        if (Hardcode(thing)) {
          DisposeSpecialObject(player, thing);
          c_Hardcode(thing);
        }
        if (0) {
          attribute_add_raw(thing, A_DESTROYER, tprintf("%d", player));
          destroy_player(thing);
        } else {
          notify(player, "The player shakes and begins to crumble.");
          s_Going(thing);
          attribute_add_raw(thing, A_DESTROYER, tprintf("%d", player));
        }
      }
    }
    break;
  case TYPE_ROOM:
    if (Going(thing)) {
      notify_quiet(player, "No sense beating a dead room.");
    } else {
      if (0) {
        empty_obj(thing);
        destroy_obj(NOTHING, thing);
      } else {
        notify_all(thing, player, "The room shakes and begins to crumble.");
        if (!Quiet(thing) && !Quiet(Owner(thing)))
          notify_quiet(Owner(thing),
                       tprintf("You will be rewarded shortly for %s(#%d).",
                               Name(thing), thing));
        if ((Owner(thing) != player) && !Quiet(player))
          notify_quiet(player, tprintf("Destroyed. %s's %s(#%d)",
                                       Name(Owner(thing)), Name(thing), thing));
        s_Going(thing);
      }
    }
  }
}
void do_chzone(DbRef player, DbRef cause, int key, char *name, char *newobj) {
  DbRef thing;
  DbRef zone;

  if (!mudconf.have_zones) {
    notify(player, "Zones disabled.");
    return;
  }
  init_match(player, name, NOTYPE);
  match_everything(0);
  if ((thing = noisy_match_result()) == NOTHING)
    return;

  if (!strcasecmp(newobj, "none"))
    zone = NOTHING;
  else {
    init_match(player, newobj, NOTYPE);
    match_everything(0);
    if ((zone = noisy_match_result()) == NOTHING)
      return;

    if ((Typeof(zone) != TYPE_THING) && (Typeof(zone) != TYPE_ROOM)) {
      notify(player, "Invalid zone object type.");
      return;
    }
  }

  if (!Wizard(player) && !(Controls(player, thing)) &&
      !(check_zone_for_player(player, thing)) &&
      !(db[player].owner == db[thing].owner)) {
    notify(player, "You don't have the power to shift reality.");
    return;
  }
  /*
   * a player may change an object's zone to NOTHING or to an object he
   *
   * *  * *  * * owns
   */
  if ((zone != NOTHING) && !Wizard(player) && !(Controls(player, zone)) &&
      !(db[player].owner == db[zone].owner)) {
    notify(player, "You cannot move that object to that zone.");
    return;
  }
  /*
   * only rooms may be zoned to other rooms
   */
  if ((zone != NOTHING) && (Typeof(zone) == TYPE_ROOM) &&
      Typeof(thing) != TYPE_ROOM) {
    notify(player, "Only rooms may have parent rooms.");
    return;
  }
  /*
   * everything is okay, do the change
   */
  db[thing].zone = zone;
  if (Typeof(thing) != TYPE_PLAYER) {
    /*
     * if the object is a player, resetting these flags is rather
     * * * * * inconvenient -- although this may pose a bit of a *
     * *  * security * risk. Be careful when @chzone'ing wizard players.
     */
    Flags(thing) &= ~WIZARD;
    Flags(thing) &= ~INHERIT;
#ifdef USE_POWERS
    Powers(thing) = 0; /*
                        * wipe out all powers
                        */
#endif
  }
  notify(player, "Zone changed.");
}
void do_name(DbRef player, DbRef cause, int key, char *name, char *newname) {
  DbRef thing;
  char *buff;
  char *buff2;
  char new[LBUF_SIZE];

  if ((thing = match_controlled(player, name)) == NOTHING)
    return;

  /*
   * check for bad name
   */
  strncpy(new, newname, LBUF_SIZE - 1);
  if ((*newname == '\0') ||
      (strlen(strip_ansi_r(new, newname, strlen(newname))) == 0)) {
    notify_quiet(player, "Give it what new name?");
    return;
  }
  /*
   * check for renaming a player
   */
  if (isPlayer(thing)) {

    buff = trim_spaces((char *)newname);
    if (!ok_player_name(buff) || !badname_check(buff)) {
      notify_quiet(player, "You can't use that name.");
      free_lbuf(buff);
      return;
    } else if (string_compare(buff, Name(thing)) &&
               (lookup_player(NOTHING, buff, 0) != NOTHING)) {

      /*
       * string_compare allows changing foo to Foo, etc.
       */

      notify_quiet(player, "That name is already in use.");
      free_lbuf(buff);
      return;
    }

    if (player == thing && In_Character(player) && !Wizard(player)) {
      buff2 = silly_atr_get(player, A_LASTNAME);
      if (!(buff2 && atoi(buff2) &&
            ((atoi(buff2) + (mudconf.namechange_days * 86400)) < mudstate.now)))
        lower_xp(player, 900);

      silly_atr_set(player, A_LASTNAME, tprintf("%u", mudstate.now));
    }
    /*
     * everything ok, notify
     */
    STARTLOG(LOG_SECURITY, "SEC", "CNAME") {
      log_name(thing), log_text((char *)" renamed to ");
      log_text(buff);
      ENDLOG;
    }
    if (Suspect(thing)) {
      send_channel("Suspect", tprintf("%s renamed to %s", Name(thing), buff));
    }
    delete_player_name(thing, Name(thing));

    object_name_set(thing, buff);
    add_player_name(thing, Name(thing));
    if (!Quiet(player) && !Quiet(thing))
      notify_quiet(player, "Name set.");
    free_lbuf(buff);
    return;
  } else {
    if (!ok_name(newname)) {
      notify_quiet(player, "That is not a reasonable name.");
      return;
    }
    /*
     * everything ok, change the name
     */
    object_name_set(thing, newname);
    if (!Quiet(player) && !Quiet(thing))
      notify_quiet(player, "Name set.");
  }
}
/*
 * ---------------------------------------------------------------------------
 * * do_unlink: Unlink an exit from its destination or remove a dropto.
 */

void do_unlink(DbRef player, DbRef cause, int key, char *name) {
  DbRef exit;

  init_match(player, name, TYPE_EXIT);
  match_everything(0);
  exit = match_result();

  switch (exit) {
  case NOTHING:
    notify_quiet(player, "Unlink what?");
    break;
  case AMBIGUOUS:
    notify_quiet(player, "I don't know which one you mean!");
    break;
  default:
    if (!controls(player, exit)) {
      notify_quiet(player, "Permission denied.");
    } else {
      switch (Typeof(exit)) {
      case TYPE_EXIT:
        s_Location(exit, NOTHING);
        if (!Quiet(player))
          notify_quiet(player, "Unlinked.");
        break;
      case TYPE_ROOM:
        s_Dropto(exit, NOTHING);
        if (!Quiet(player))
          notify_quiet(player, "Dropto removed.");
        break;
      default:
        notify_quiet(player, "You can't unlink that!");
        break;
      }
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_chown: Change ownership of an object or attribute.
 */

void do_chown(DbRef player, DbRef cause, int key, char *name, char *newown) {
  DbRef thing, owner, aowner;
  int atr, do_it;
  long aflags;
  Attribute *ap;

  if (parse_attrib(player, name, &thing, &atr)) {
    if (atr != NOTHING) {
      if (!*newown) {
        owner = Owner(thing);
      } else if (!(string_compare(newown, "me"))) {
        owner = Owner(player);
      } else {
        owner = lookup_player(player, newown, 1);
      }

      /*
       * You may chown an attr to yourself if you own the *
       *
       * *  * *  * * object and the attr is not locked. *
       * You * may * chown  * an attr to the owner of the
       * object * if * * you own * the attribute. * To do
       * anything * else you  * must be a  * wizard. * Only
       * #1 can * chown * attributes on #1.
       */

      if (!attribute_get_info(thing, atr, &aowner, &aflags)) {
        notify_quiet(player, "Attribute not present on object.");
        return;
      }
      do_it = 0;
      if (owner == NOTHING) {
        notify_quiet(player, "I couldn't find that player.");
      } else if (God(thing) && !God(player)) {
        notify_quiet(player, "Permission denied.");
      } else if (Wizard(player)) {
        do_it = 1;
      } else if (owner == Owner(player)) {

        /*
         * chown to me: only if I own the obj and * *
         *
         * *  * * !locked
         */

        if (!Controls(player, thing) || (aflags & AF_LOCK)) {
          notify_quiet(player, "Permission denied.");
        } else {
          do_it = 1;
        }
      } else if (owner == Owner(thing)) {

        /*
         * chown to obj owner: only if I own attr * *
         *
         * *  * * and !locked
         */

        if ((Owner(player) != aowner) || (aflags & AF_LOCK)) {
          notify_quiet(player, "Permission denied.");
        } else {
          do_it = 1;
        }
      } else {
        notify_quiet(player, "Permission denied.");
      }

      if (!do_it)
        return;

      ap = attribute_by_number(atr);
      if (!ap || !Set_attr(player, player, ap, aflags)) {
        notify_quiet(player, "Permission denied.");
        return;
      }
      attribute_set_owner(thing, atr, owner);
      if (!Quiet(player))
        notify_quiet(player, "Attribute owner changed.");
      return;
    }
  }
  init_match(player, name, TYPE_THING);
  match_possession();
  match_here();
  match_exit();
  match_me();
  if (Wizard(player)) {
    match_player();
    match_absolute();
  }
  switch (thing = match_result()) {
  case NOTHING:
    notify_quiet(player, "You don't have that!");
    return;
  case AMBIGUOUS:
    notify_quiet(player, "I don't know which you mean!");
    return;
  }

  if (!*newown || !(string_compare(newown, "me"))) {
    owner = Owner(player);
  } else {
    owner = lookup_player(player, newown, 1);
  }

  if (owner == NOTHING) {
    notify_quiet(player, "I couldn't find that player.");
  } else if (isPlayer(thing) && !God(player)) {
    notify_quiet(player, "Players always own themselves.");
  } else if (((!controls(player, thing) && !Wizard(player)) ||
              (isThing(thing) && (Location(thing) != player) &&
               !Wizard(player))) ||
             (!controls(player, owner))) {
    notify_quiet(player, "Permission denied.");
  } else {
    if (God(player)) {
      s_Owner(thing, owner);
    } else {
      s_Owner(thing, Owner(owner));
    }
    attribute_change_owner(thing);
    s_Flags(thing, (Flags(thing) & ~INHERIT) | HALT);
    s_Powers(thing, 0);
    s_Powers2(thing, 0);
    halt_que(NOTHING, thing);
    if (!Quiet(player))
      notify_quiet(player, "Owner changed.");
  }
}
/*
 * ---------------------------------------------------------------------------
 * * do_set: Set flags or attributes on objects, or flags on attributes.
 */
void do_set(DbRef player, DbRef cause, int key, char *name, char *flag) {
  DbRef thing, thing2, aowner;
  char *p, *buff;
  int atr, atr2, clear, flagvalue, could_hear, have_xcode;
  long aflags;
  Attribute *attr, *attr2;

  /*
   * See if we have the <obj>/<attr> form, which is how you set * * *
   * attribute * flags.
   */

  if (parse_attrib(player, name, &thing, &atr)) {
    if (atr != NOTHING) {

      /*
       * You must specify a flag name
       */

      if (!flag || !*flag) {
        notify_quiet(player, "I don't know what you want to set!");
        return;
      }
      /*
       * Check for clearing
       */

      clear = 0;
      if (*flag == NOT_TOKEN) {
        flag++;
        clear = 1;
      }
      /*
       * Make sure player specified a valid attribute flag
       */

      flagvalue = name_table_search(player, indiv_attraccess_nametab, flag);
      if (flagvalue < 0) {
        notify_quiet(player, "You can't set that!");
        return;
      }
      /*
       * Make sure the object has the attribute present
       */

      if (!attribute_get_info(thing, atr, &aowner, &aflags)) {
        notify_quiet(player, "Attribute not present on object.");
        return;
      }
      /*
       * Make sure we can write to the attribute
       */

      attr = attribute_by_number(atr);
      if (!attr || !Set_attr(player, thing, attr, aflags)) {
        notify_quiet(player, "Permission denied.");
        return;
      }
      /*
       * Go do it
       */

      if (clear)
        aflags &= ~flagvalue;
      else
        aflags |= flagvalue;
      have_xcode = Hardcode(thing);
      attribute_set_flags(thing, atr, aflags);

      /*
       * Tell the player about it.
       */

      if (mudconf.have_specials)
        handle_xcode(player, thing, have_xcode, Hardcode(thing));
      if (!(key & SET_QUIET) && !Quiet(player) && !Quiet(thing)) {
        NameTable *nt;
        nt = name_table_find_entry(player, indiv_attraccess_nametab, flag);
        notify_printf(player, "%s/%s - %s %s", Name(thing), attr->name,
                      nt->name, clear ? "cleared." : "set.");
      }
      could_hear = is_hearer(thing);
      handle_ears(thing, could_hear, is_hearer(thing));
      return;
    }
  }
  /*
   * find thing
   */

  if ((thing = match_controlled(player, name)) == NOTHING)
    return;

  /*
   * check for attribute set first
   */
  for (p = flag; *p && (*p != ':'); p++)
    ;

  if (*p) {
    *p++ = 0;
    atr = mkattr(flag);
    if (atr <= 0) {
      notify_quiet(player, "Couldn't create attribute.");
      return;
    }
    attr = attribute_by_number(atr);
    if (!attr) {
      notify_quiet(player, "Permission denied.");
      return;
    }
    attribute_get_info(thing, atr, &aowner, &aflags);
    if (!Set_attr(player, thing, attr, aflags)) {
      notify_quiet(player, "Permission denied.");
      return;
    }
    buff = alloc_lbuf("do_set");

    /*
     * check for _
     */
    if (*p == '_') {
      StringCopy(buff, p + 1);
      if (!parse_attrib(player, p + 1, &thing2, &atr2) || (atr2 == NOTHING)) {
        notify_quiet(player, "No match.");
        free_lbuf(buff);
        return;
      }
      attr2 = attribute_by_number(atr2);
      p = buff;
      attribute_parent_get_string(buff, thing2, atr2, &aowner, &aflags);

      if (!attr2 || !See_attr(player, thing2, attr2, aowner, aflags)) {
        notify_quiet(player, "Permission denied.");
        free_lbuf(buff);
        return;
      }
    }
    /*
     * Go set it
     */

    object_attribute_set(player, thing, atr, p, key);
    free_lbuf(buff);
    return;
  }
  /*
   * Set or clear a flag
   */

  flag_set(thing, player, flag, key);
}
void do_cpattr(DbRef player, DbRef cause, int key, char *oldpair,
               char *newpair[], int nargs) {
  int i;
  char *oldthing, *oldattr, *newthing, *newattr;

  if (!*oldpair || !**newpair || !oldpair || !*newpair)
    return;

  if (nargs < 1)
    return;

  oldattr = oldpair;
  oldthing = parse_to(&oldattr, '/', 1);

  for (i = 0; i < nargs; i++) {
    newattr = newpair[i];
    newthing = parse_to(&newattr, '/', 1);

    if (!oldattr) {
      if (!newattr) {
        do_set(player, cause, 0, newthing,
               tprintf("%s:_%s/%s", oldthing, "me", oldthing));
      } else {
        do_set(player, cause, 0, newthing,
               tprintf("%s:_%s/%s", newattr, "me", oldthing));
      }
    } else {
      if (!newattr) {
        do_set(player, cause, 0, newthing,
               tprintf("%s:_%s/%s", oldattr, oldthing, oldattr));
      } else {
        do_set(player, cause, 0, newthing,
               tprintf("%s:_%s/%s", newattr, oldthing, oldattr));
      }
    }
  }
}

void do_mvattr(DbRef player, DbRef cause, int key, char *what, char *args[],
               int nargs) {
  DbRef thing, aowner, axowner;
  Attribute *in_attr, *out_attr;
  int i, anum, in_anum, no_delete;
  long aflags, axflags;
  char *astr;

  aflags = 0;

  /*
   * Make sure we have something to do.
   */

  if (nargs < 2) {
    notify_quiet(player, "Nothing to do.");
    return;
  }
  /*
   * FInd and make sure we control the target object.
   */

  thing = match_controlled(player, what);
  if (thing == NOTHING)
    return;

  /*
   * Look up the source attribute.  If it either doesn't exist or isn't
   * * * * * readable, use an empty string.
   */

  in_anum = -1;
  astr = alloc_lbuf("do_mvattr");
  in_attr = attribute_by_name(args[0]);
  if (in_attr == NULL) {
    *astr = '\0';
  } else {
    attribute_get_string(astr, thing, in_attr->number, &aowner, &aflags);
    if (!See_attr(player, thing, in_attr, aowner, aflags)) {
      *astr = '\0';
    } else {
      in_anum = in_attr->number;
    }
  }

  /*
   * Copy the attribute to each target in turn.
   */

  no_delete = 0;
  for (i = 1; i < nargs; i++) {
    anum = mkattr(args[i]);
    if (anum <= 0) {
      notify_quiet(
          player,
          tprintf("%s: That's not a good name for an attribute.", args[i]));
      continue;
    }
    out_attr = attribute_by_number(anum);
    if (!out_attr) {
      notify_quiet(player, tprintf("%s: Permission denied.", args[i]));
    } else if (out_attr->number == in_anum) {
      no_delete = 1;
    } else {
      attribute_get_info(thing, out_attr->number, &axowner, &axflags);
      if (!Set_attr(player, thing, out_attr, axflags)) {
        notify_quiet(player, tprintf("%s: Permission denied.", args[i]));
      } else {
        attribute_add(thing, out_attr->number, astr, Owner(player), aflags);
        if (!Quiet(player))
          notify_printf(player, "%s/%s - Set.", Name(thing), out_attr->name);
      }
    }
  }

  /*
   * Remove the source attribute if we can.
   */

  if ((in_anum > 0) && !no_delete) {
    in_attr = attribute_by_number(in_anum);
    if (in_attr && Set_attr(player, thing, in_attr, aflags)) {
      attribute_clear(thing, in_attr->number);
      if (!Quiet(player))
        notify_printf(player, "%s/%s - Cleared.", Name(thing), in_attr->name);
    } else {
      if (in_attr)
        notify_quiet(
            player,
            tprintf("%s: Could not remove old attribute.  Permission denied.",
                    in_attr->name));
    }
  }
  free_lbuf(astr);
}
void do_wipe(DbRef player, DbRef cause, int key, char *it) {
  DbRef thing, aowner;
  int attr, got_one;
  long aflags;
  Attribute *ap;
  char *atext;

  olist_push();
  if (!it || !*it || !parse_attrib_wild(player, it, &thing, 0, 0, 1)) {
    notify_quiet(player, "No match.");
    olist_pop();
    return;
  }
  /*
   * Iterate through matching attributes, zapping the writable ones
   */

  got_one = 0;
  atext = alloc_lbuf("do_wipe.atext");
  for (attr = olist_first(); attr != NOTHING; attr = olist_next()) {
    ap = attribute_by_number(attr);
    if (ap) {

      /*
       * Get the attr and make sure we can modify it.
       */

      attribute_get_string(atext, thing, ap->number, &aowner, &aflags);
      if (Set_attr(player, thing, ap, aflags)) {
        attribute_clear(thing, ap->number);
        got_one++;
      }
    }
  }
  /*
   * Clean up
   */

  if (!got_one) {
    notify_quiet(player, "No matching attributes.");
  } else {
    if (!Quiet(player))
      notify_printf(player, "%s - %d attribute(s) wiped.", Name(thing),
                    got_one);
  }

  free_lbuf(atext);
  olist_pop();
}
