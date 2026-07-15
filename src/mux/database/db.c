/* db.c - In-memory game-object and attribute database operations. */

#include "mux/server/platform.h"

#include <sys/file.h>
#include <sys/stat.h>

#define __DB_C
#include "mux/commands/command.h"
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/database/vattr.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"
#include "mux/world/player_cache.h"

#ifndef O_ACCMODE
#define O_ACCMODE (O_RDONLY | O_WRONLY | O_RDWR)
#endif

/*
 * Restart definitions
 */

GameObject *db = nullptr;
NAME *names = nullptr;
NAME *purenames = nullptr;

int corrupt;

extern void del_commac(DbRef);
extern void do_clear_macro(DbRef player, char *s);

#ifdef TEST_MALLOC
int malloc_count = 0;

#endif /*                                                                      \
        * * TEST_MALLOC                                                        \
        */

extern VATTR *vattr_rename(char *, char *);

typedef struct atrcount ATRCOUNT;
struct atrcount {
  DbRef thing;
  int count;
};

/*
 * #define GNU_MALLOC_TEST 1
 */

#ifdef GNU_MALLOC_TEST
extern unsigned int malloc_sbrk_used; /* Amount of data space used now */
#endif

/*
 * Check routine forward declaration.
 */
extern int fwdlist_ck(int, DbRef, DbRef, int, char *);

// Flags for character stats/skills attributes.
constexpr int PLSTAT_MODE = AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL;

/*
 * list of attributes
 */
Attribute attr_table[] = {
    {"Aahear", A_AAHEAR, AF_ODARK, nullptr},
    {"Aclone", A_ACLONE, AF_ODARK, nullptr},
    {"Aconnect", A_ACONNECT, AF_ODARK, nullptr},
    {"Adesc", A_ADESC, AF_ODARK, nullptr},
    {"Adfail", A_ADFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Adisconnect", A_ADISCONNECT, AF_ODARK, nullptr},
    {"Adrop", A_ADROP, AF_ODARK, nullptr},
    {"Aefail", A_AEFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Aenter", A_AENTER, AF_ODARK, nullptr},
    {"Afail", A_AFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Agfail", A_AGFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Ahear", A_AHEAR, AF_ODARK, nullptr},
    {"Aleave", A_ALEAVE, AF_ODARK, nullptr},
    {"Alfail", A_ALFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Alias", A_ALIAS, AF_NOPROG | AF_NOCMD | AF_GOD, nullptr},
    {"Amhear", A_AMHEAR, AF_ODARK, nullptr},
    {"Amove", A_AMOVE, AF_ODARK, nullptr},
    {"Arfail", A_ARFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Asucc", A_ASUCC, AF_ODARK, nullptr},
    {"Atfail", A_ATFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Atport", A_ATPORT, AF_ODARK | AF_NOPROG, nullptr},
    {"Atofail", A_ATOFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Aufail", A_AUFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Ause", A_AUSE, AF_ODARK, nullptr},
    {"Away", A_AWAY, AF_ODARK | AF_NOPROG, nullptr},
    {"Buildcoord", A_BUILDCOORD, AF_MDARK | AF_WIZARD, nullptr},
    {"Buildentrance", A_BUILDENTRANCE, AF_MDARK | AF_WIZARD, nullptr},
    {"Buildlinks", A_BUILDLINKS, AF_MDARK | AF_WIZARD, nullptr},
    {"Charges", A_CHARGES, AF_ODARK | AF_NOPROG, nullptr},
    {"Comment", A_COMMENT, AF_MDARK | AF_WIZARD, nullptr},
    {"Contactoptions", A_CONTACTOPT, AF_ODARK, nullptr},
    {"Daily", A_DAILY, AF_ODARK, nullptr},
    {"HHourly", A_HOURLY, AF_MDARK, nullptr},
    {"Desc", A_DESC, AF_NOPROG, nullptr},
    {"DefaultLock", A_LOCK, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"Destroyer", A_DESTROYER, AF_MDARK | AF_WIZARD | AF_NOPROG, nullptr},
    {"Dfail", A_DFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Drop", A_DROP, AF_ODARK | AF_NOPROG, nullptr},
    {"DropLock", A_LDROP, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"Ealias", A_EALIAS, AF_ODARK | AF_NOPROG, nullptr},
    {"Efail", A_EFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Enter", A_ENTER, AF_ODARK, nullptr},
    {"EnterLock", A_LENTER, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"Faction", A_FACTION, AF_MDARK | AF_WIZARD, nullptr},
    {"Fail", A_FAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Filter", A_FILTER, AF_ODARK | AF_NOPROG, nullptr},
    {"Forwardlist", A_FORWARDLIST, AF_ODARK | AF_NOPROG, fwdlist_ck},
    {"Gfail", A_GFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"GiveLock", A_LGIVE, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"Idesc", A_IDESC, AF_ODARK | AF_NOPROG, nullptr},
    {"Idle", A_IDLE, AF_ODARK | AF_NOPROG, nullptr},
    {"Infilter", A_INFILTER, AF_ODARK | AF_NOPROG, nullptr},
    {"Inprefix", A_INPREFIX, AF_ODARK | AF_NOPROG, nullptr},
    {"Job", A_JOB, AF_MDARK | AF_WIZARD, nullptr},
    {"Lalias", A_LALIAS, AF_ODARK | AF_NOPROG, nullptr},
    {"Last", A_LAST, AF_WIZARD | AF_NOCMD | AF_NOPROG, nullptr},
    {"Lastname", A_LASTNAME, AF_WIZARD | AF_NOPROG | AF_MDARK, nullptr},
    {"Luaparent", A_LUAPARENT,
     AF_WIZARD | AF_MDARK | AF_NOCMD | AF_NOPROG | AF_LOCK, nullptr},
    {"Lastpage", A_LASTPAGE,
     AF_INTERNAL | AF_NOCMD | AF_NOPROG | AF_GOD | AF_PRIVATE, nullptr},
    {"Lastsite", A_LASTSITE, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_GOD, nullptr},
    {"Leave", A_LEAVE, AF_ODARK, nullptr},
    {"LeaveLock", A_LLEAVE, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"Lfail", A_LFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"LinkLock", A_LLINK, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"Listen", A_LISTEN, AF_ODARK, nullptr},

    {"Logindata", A_LOGINDATA, AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL,
     nullptr},
    {"LRSheight", A_LRSHEIGHT, AF_ODARK, nullptr},
    {"Unused1", A_UNUSED1, AF_WIZARD | AF_MDARK, nullptr},
    {"Mapcolor", A_MAPCOLOR, AF_ODARK, nullptr},
    {"Mapvis", A_MAPVIS, AF_MDARK | AF_WIZARD, nullptr},
    {"Mechdesc", A_MECHDESC, AF_MDARK, nullptr},
    {"Mechname", A_MECHNAME, AF_MDARK, nullptr},
    {"Mechstatus", A_MECHSTATUS, AF_MDARK | AF_WIZARD, nullptr},
    {"Mechtype", A_MECHTYPE, AF_MDARK, nullptr},
    {"MechPrefID", A_MECHPREFID, AF_MDARK | AF_WIZARD, nullptr},
    {"Mechskills", A_MECHSKILLS, AF_MDARK, nullptr},
    {"Mwtemplate", A_MWTEMPLATE, AF_MDARK, nullptr},
    {"Move", A_MOVE, AF_ODARK, nullptr},
    {"Name", A_NAME, AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL, nullptr},
    {"Odesc", A_ODESC, AF_ODARK | AF_NOPROG, nullptr},
    {"Odfail", A_ODFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Odrop", A_ODROP, AF_ODARK | AF_NOPROG, nullptr},
    {"Oefail", A_OEFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Oenter", A_OENTER, AF_ODARK, nullptr},
    {"Ofail", A_OFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Ogfail", A_OGFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Amechdest", A_AMECHDEST, AF_MDARK, nullptr},
    {"Aaeroland", A_AAEROLAND, AF_MDARK, nullptr},
    {"Aoodland", A_AOODLAND, AF_MDARK, nullptr},
    {"Aminetrigger", A_AMINETRIGGER, AF_MDARK, nullptr},
    {"Oleave", A_OLEAVE, AF_ODARK, nullptr},
    {"Olfail", A_OLFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Omove", A_OMOVE, AF_ODARK, nullptr},
    {"Orfail", A_ORFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Osucc", A_OSUCC, AF_ODARK | AF_NOPROG, nullptr},
    {"Otfail", A_OTFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Otport", A_OTPORT, AF_ODARK | AF_NOPROG, nullptr},
    {"Otofail", A_OTOFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Oufail", A_OUFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Ouse", A_OUSE, AF_ODARK, nullptr},
    {"Oxenter", A_OXENTER, AF_ODARK, nullptr},
    {"Oxleave", A_OXLEAVE, AF_ODARK, nullptr},
    {"Oxtport", A_OXTPORT, AF_ODARK | AF_NOPROG, nullptr},
    {"PageLock", A_LPAGE, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"ParentLock", A_LPARENT, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"PCequip", A_PCEQUIP, AF_MDARK, nullptr},
    {"Pilot", A_PILOTNUM, AF_MDARK, nullptr},
    {"Prefix", A_PREFIX, AF_ODARK | AF_NOPROG, nullptr},
    {"ProgCmd", A_PROGCMD, AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL,
     nullptr},
    {"QueueMax", A_QUEUEMAX, AF_MDARK | AF_WIZARD | AF_NOPROG, nullptr},
    {"Ranknum", A_RANKNUM, AF_MDARK | AF_WIZARD, nullptr},
    {"ReceiveLock", A_LRECEIVE, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"Reject", A_REJECT, AF_ODARK | AF_NOPROG, nullptr},
    {"Rfail", A_RFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Runout", A_RUNOUT, AF_ODARK, nullptr},

    {"Semaphore", A_SEMAPHORE, AF_ODARK | AF_NOPROG | AF_WIZARD | AF_NOCMD,
     nullptr},
    {"SpeechLock", A_LSPEECH, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"Startup", A_STARTUP, AF_ODARK, nullptr},
    {"Succ", A_SUCC, AF_ODARK | AF_NOPROG, nullptr},
    {"Tacsize", A_TACSIZE, AF_ODARK, nullptr},
    {"TeloutLock", A_LTELOUT, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"Tfail", A_TFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Timeout", A_TIMEOUT, AF_MDARK | AF_NOPROG | AF_WIZARD, nullptr},
    {"Tport", A_TPORT, AF_ODARK | AF_NOPROG, nullptr},
    {"TportLock", A_LTPORT, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"Tofail", A_TOFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Tz", A_TZ, AF_NOPROG, nullptr},
    {"Ufail", A_UFAIL, AF_ODARK | AF_NOPROG, nullptr},
    {"Use", A_USE, AF_ODARK, nullptr},
    {"UseLock", A_LUSE, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK, nullptr},
    {"UserLock", A_LUSER, AF_ODARK | AF_NOPROG | AF_NOCMD | AF_IS_LOCK,
     nullptr},
    {"VA", A_VA, AF_ODARK, nullptr},
    {"VB", A_VA + 1, AF_ODARK, nullptr},
    {"VC", A_VA + 2, AF_ODARK, nullptr},
    {"VD", A_VA + 3, AF_ODARK, nullptr},
    {"VE", A_VA + 4, AF_ODARK, nullptr},
    {"VF", A_VA + 5, AF_ODARK, nullptr},
    {"VG", A_VA + 6, AF_ODARK, nullptr},
    {"VH", A_VA + 7, AF_ODARK, nullptr},
    {"VI", A_VA + 8, AF_ODARK, nullptr},
    {"VJ", A_VA + 9, AF_ODARK, nullptr},
    {"VK", A_VA + 10, AF_ODARK, nullptr},
    {"VL", A_VA + 11, AF_ODARK, nullptr},
    {"VM", A_VA + 12, AF_ODARK, nullptr},
    {"VN", A_VA + 13, AF_ODARK, nullptr},
    {"VO", A_VA + 14, AF_ODARK, nullptr},
    {"VP", A_VA + 15, AF_ODARK, nullptr},
    {"VQ", A_VA + 16, AF_ODARK, nullptr},
    {"VR", A_VA + 17, AF_ODARK, nullptr},
    {"VS", A_VA + 18, AF_ODARK, nullptr},
    {"VT", A_VA + 19, AF_ODARK, nullptr},
    {"VU", A_VA + 20, AF_ODARK, nullptr},
    {"VV", A_VA + 21, AF_ODARK, nullptr},
    {"VW", A_VA + 22, AF_ODARK, nullptr},
    {"VX", A_VA + 23, AF_ODARK, nullptr},
    {"VY", A_VA + 24, AF_ODARK, nullptr},
    {"VZ", A_VA + 25, AF_ODARK, nullptr},
    {"Xtype", A_XTYPE, AF_MDARK | AF_WIZARD, nullptr},
    {"*Password", A_PASS, AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL,
     nullptr},
    {"*Privileges", A_PRIVS, AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL,
     nullptr},
    {"Techtime", A_TECHTIME, AF_MDARK | AF_WIZARD, nullptr},
    {"*EconParts", A_ECONPARTS, AF_DARK | AF_NOPROG | AF_NOCMD | AF_INTERNAL,
     nullptr},
    {"PLHEALTH", A_HEALTH, PLSTAT_MODE, nullptr},
    {"PLATTRS", A_ATTRS, PLSTAT_MODE, nullptr},
    {"PLADVS", A_ADVS, PLSTAT_MODE, nullptr},
    {"PLSKILLS", A_SKILLS, PLSTAT_MODE, nullptr},

    {nullptr, 0, 0, nullptr}};

/*
 * ---------------------------------------------------------------------------
 * * fwdlist_set, fwdlist_clr: Manage cached forwarding lists
 */
void fwdlist_set(DbRef thing, FWDLIST *ifp) {
  FWDLIST *fp, *xfp;
  int i;

  /*
   * If fwdlist is null, clear
   */

  if (!ifp || (ifp->count <= 0)) {
    fwdlist_clr(thing);
    return;
  }
  /*
   * Copy input forwardlist to a correctly-sized buffer
   */

  fp = (FWDLIST *)XMALLOC(sizeof(FWDLIST), "fwdlist_set");

  for (i = 0; i < ifp->count; i++) {
    fp->data[i] = ifp->data[i];
  }
  fp->count = ifp->count;

  /*
   * Replace an existing forwardlist, or add a new one
   */

  xfp = fwdlist_get(thing);
  if (xfp) {
    XFREE(xfp, "fwdlist_set");
    numeric_hash_table_replace(thing, (int *)fp, &mudstate.fwdlist_htab);
  } else {
    numeric_hash_table_add(thing, (int *)fp, &mudstate.fwdlist_htab);
  }
}

void fwdlist_clr(DbRef thing) {
  FWDLIST *xfp;

  /*
   * If a forwardlist exists, delete it
   */

  xfp = fwdlist_get(thing);
  if (xfp) {
    XFREE(xfp, "fwdlist_clr");
    numeric_hash_table_delete(thing, &mudstate.fwdlist_htab);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * fwdlist_load: Load text into a forwardlist.
 */
int fwdlist_load(FWDLIST *fp, DbRef player, char *atext) {
  DbRef target;
  char *tp, *bp, *dp;
  int count, errors, fail;

  if (!atext)
    atext = "";

  count = 0;
  errors = 0;
  bp = tp = alloc_lbuf("fwdlist_load.str");
  StringCopy(tp, atext);

  do {
    for (; *bp && isspace(*bp); bp++)
      ; /*
         * skip spaces
         */
    for (dp = bp; *bp && !isspace(*bp); bp++)
      ; /*
         * remember string
         */
    if (*bp)
      *bp++ = '\0'; /*
                     * terminate string
                     */
    if ((*dp++ == '#') && isdigit(*dp)) {
      target = atoi(dp);
      fail = (!is_good_obj(target) ||
              (!is_god(player) && !is_controls(player, target)));
      if (fail) {
        notify_printf(player, "Cannot forward to #%ld: Permission denied.",
                      target);
        errors++;
      } else {
        fp->data[count++] = target;
      }
    }
  } while (*bp);

  free_lbuf(tp);
  fp->count = count;
  return errors;
}

/*
 * ---------------------------------------------------------------------------
 * * fwdlist_rewrite: Generate a text string from a FWDLIST buffer.
 */
int fwdlist_rewrite(FWDLIST *fp, char *atext) {
  char *tp, *bp;
  int i, count;

  if (fp && fp->count) {
    count = fp->count;
    tp = alloc_sbuf("fwdlist_rewrite.errors");
    bp = atext;
    for (i = 0; i < fp->count; i++) {
      if (is_good_obj(fp->data[i])) {
        snprintf(tp, SBUF_SIZE, "#%d ", fp->data[i]);
        safe_str(tp, atext, &bp);
      } else {
        count--;
      }
    }
    *bp = '\0';
    free_sbuf(tp);
  } else {
    count = 0;
    if (atext)
      *atext = '\0';
  }
  return count;
}

/*
 * ---------------------------------------------------------------------------
 * * fwdlist_ck:  Check a list of dbref numbers to forward to for AUDIBLE
 */

int fwdlist_ck(int key, DbRef player, DbRef thing, int anum, char *atext) {
  FWDLIST *fp;
  int count;

  count = 0;

  if (atext && *atext) {
    fp = (FWDLIST *)alloc_lbuf("fwdlist_ck.fp");
    fwdlist_load(fp, player, atext);
  } else {
    fp = nullptr;
  }

  /*
   * Set the cached forwardlist
   */

  fwdlist_set(thing, fp);
  count = atext ? fwdlist_rewrite(fp, atext) : (fp ? fp->count : 0);
  if (fp)
    free_lbuf(fp);
  return ((count > 0) || !atext || !*atext);
}

FWDLIST *fwdlist_get(DbRef thing) {
  FWDLIST *fp;

  fp = ((FWDLIST *)numeric_hash_table_find(thing, &mudstate.fwdlist_htab));

  return fp;
}

static char *set_string(char **ptr, char *new) {
  /*
   * if pointer not null unalloc it
   */

  if (*ptr)
    XFREE(*ptr, "set_string");

  /*
   * if new string is not null allocate space for it and copy it
   */

  if (!new)                  /*
                              * * || !*new
                              */
    return (*ptr = nullptr); /*
                              * Check with GAC about this
                              */
  *ptr = (char *)XMALLOC(strlen(new) + 1, "set_string");
  StringCopy(*ptr, new);
  return (*ptr);
}

/*
 * ---------------------------------------------------------------------------
 * * Name, s_Name: Get or set an object's name.
 */
INLINE char *Name(DbRef thing) {
  DbRef aowner;
  long aflags;
  char *buff;
  static char *tbuff[MBUF_SIZE];
  char buffer[MBUF_SIZE];

  if (mudconf.cache_names) {
    if (thing > mudstate.db_top || thing < 0) {
      return "#-1 INVALID DBREF";
    }
    if (!purenames[thing]) {
      buff = attribute_get(thing, A_NAME, &aowner, &aflags);
      strip_ansi_r(buffer, buff, MBUF_SIZE);
      set_string(&purenames[thing], buffer);
      free_lbuf(buff);
    }
  }

  attribute_get_string((char *)tbuff, thing, A_NAME, &aowner, &aflags);
  return ((char *)tbuff);
}

INLINE char *PureName(DbRef thing) {
  DbRef aowner;
  long aflags;
  char *buff;
  static char *tbuff[LBUF_SIZE];
  char new[LBUF_SIZE];

  if (mudconf.cache_names) {
    if (thing > mudstate.db_top || thing < 0) {
      return "#-1 INVALID DBREF";
    }
    if (!purenames[thing]) {
      buff = attribute_get(thing, A_NAME, &aowner, &aflags);
      set_string(&purenames[thing], strip_ansi_r(new, buff, strlen(buff)));
      free_lbuf(buff);
    }
    return purenames[thing];
  }

  attribute_get_string((char *)tbuff, thing, A_NAME, &aowner, &aflags);
  return (strip_ansi_r(new, (char *)tbuff, strlen((char *)tbuff)));
}

INLINE void object_name_set(DbRef thing, char *s) {
  char new[MBUF_SIZE];
  /* Truncate the name if we have to */

  strncpy(new, s, MBUF_SIZE - 1);
  if (s && (strlen(s) > MBUF_SIZE))
    s[MBUF_SIZE] = '\0';

  attribute_add_raw(thing, A_NAME, (char *)s);

  if (mudconf.cache_names) {
    set_string(&purenames[thing], strip_ansi_r(new, s, strlen(s)));
  }
}

void object_password_set(DbRef thing, const char *s) {
  attribute_add_raw(thing, A_PASS, (char *)s);
}

/*
 * ---------------------------------------------------------------------------
 * * do_attrib: Manage user-named attributes.
 */

extern NameTable attraccess_nametab[];

void do_attribute(DbRef player, DbRef cause, int key, char *aname,
                  char *value) {
  int success, negate, f;
  char *buff, *sp, *p, *q;
  VATTR *va;
  Attribute *va2;

  /*
   * Look up the user-named attribute we want to play with
   */

  buff = alloc_sbuf("do_attribute");
  for (p = buff, q = aname; *q && ((p - buff) < (SBUF_SIZE - 1)); p++, q++)
    *p = ToUpper(*q);
  *p = '\0';

  va = (VATTR *)vattr_find(buff);
  if (!va) {
    notify_printf(player, "No such user-named attribute: %s", buff);
    free_sbuf(buff);
    return;
  }
  switch (key) {
  case ATTRIB_ACCESS:

    /*
     * Modify access to user-named attribute
     */

    for (sp = value; *sp; sp++)
      *sp = ToUpper(*sp);
    sp = strtok(value, " ");
    success = 0;
    while (sp != nullptr) {

      /*
       * Check for negation
       */

      negate = 0;
      if (*sp == '!') {
        negate = 1;
        sp++;
      }
      /*
       * Set or clear the appropriate bit
       */

      f = name_table_search(player, attraccess_nametab, sp);
      if (f > 0) {
        success = 1;
        if (negate)
          va->flags &= ~f;
        else
          va->flags |= f;
      } else {
        notify_printf(player, "Unknown permission: %s.", sp);
      }

      /*
       * Get the next token
       */

      sp = strtok(nullptr, " ");
    }
    if (success && !is_quiet(player))
      notify_printf(player, "Attribute access for %s changed to %s.", va->name,
                    value);
    break;

  case ATTRIB_RENAME:

    /*
     * Make sure the new name doesn't already exist
     */

    va2 = attribute_by_name(value);
    if (va2) {
      notify(player, "An attribute with that name already exists.");
      free_sbuf(buff);
      return;
    }
    if (vattr_rename(va->name, value) == nullptr)
      notify(player, "Attribute rename failed.");
    else
      notify(player, "Attribute renamed.");
    break;

  case ATTRIB_DELETE:

    /*
     * Remove the attribute
     */

    vattr_delete(buff);
    notify(player, "Attribute deleted.");
    break;
  }
  free_sbuf(buff);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * do_fixdb: Directly edit database fields
 */

void do_fixdb(DbRef player, DbRef cause, int key, char *arg1, char *arg2) {
  DbRef thing, res;

  init_match(player, arg1, NOTYPE);
  match_everything(0);
  thing = noisy_match_result();
  if (thing == NOTHING)
    return;

  res = NOTHING;
  switch (key) {
  case FIXDB_OWNER:
  case FIXDB_LOC:
  case FIXDB_CON:
  case FIXDB_EXITS:
  case FIXDB_NEXT:
    init_match(player, arg2, NOTYPE);
    match_everything(0);
    res = noisy_match_result();
    break;
  }

  switch (key) {
  case FIXDB_OWNER:
    s_owner(thing, res);
    if (!is_quiet(player))
      notify_printf(player, "Owner set to #%ld", res);
    break;
  case FIXDB_LOC:
    s_location(thing, res);
    if (!is_quiet(player))
      notify_printf(player, "Location set to #%ld", res);
    break;
  case FIXDB_CON:
    s_contents(thing, res);
    if (!is_quiet(player))
      notify_printf(player, "Contents set to #%ld", res);
    break;
  case FIXDB_EXITS:
    s_exits(thing, res);
    if (!is_quiet(player))
      notify_printf(player, "Exits set to #%ld", res);
    break;
  case FIXDB_NEXT:
    s_next(thing, res);
    if (!is_quiet(player))
      notify_printf(player, "Next set to #%ld", res);
    break;
  case FIXDB_NAME:
    if (typeof_obj(thing) == TYPE_PLAYER) {
      if (!ok_player_name(arg2)) {
        notify(player, "That's not a good name for a player.");
        return;
      }
      if (lookup_player(NOTHING, arg2, 0) != NOTHING) {
        notify(player, "That name is already in use.");
        return;
      }
      STARTLOG(LOG_SECURITY, "SEC", "CNAME") {
        log_name(thing), log_text((char *)" renamed to ");
        log_text(arg2);
        ENDLOG;
      }
      if (is_suspect(player)) {
        send_channel("Suspect", "%s renamed to %s", Name(thing), arg2);
      }
      delete_player_name(thing, Name(thing));

      object_name_set(thing, arg2);
      add_player_name(thing, arg2);
    } else {
      if (!ok_name(arg2)) {
        notify(player, "Warning: That is not a reasonable name.");
      }
      object_name_set(thing, arg2);
    }
    if (!is_quiet(player))
      notify_printf(player, "Name set to %s", arg2);
    break;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * init_attrtab: Initialize the attribute hash tables.
 */

void init_attrtab(void) {
  Attribute *a;
  char *buff, *p, *q;

  hash_table_initialize(&mudstate.attr_name_htab, 512);
  buff = alloc_sbuf("init_attrtab");
  for (a = attr_table; a->number; a++) {
    anum_extend(a->number);
    anum_set(a->number, a);
    for (p = buff, q = (char *)a->name; *q; p++, q++)
      *p = ToUpper(*q);
    *p = '\0';
    hash_table_add(buff, (int *)a, &mudstate.attr_name_htab);
  }
  free_sbuf(buff);
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_by_name: Look up an attribute by name.
 */

Attribute *attribute_by_name(char *s) {
  char *buff, *p, *q;
  Attribute *a;
  VATTR *va;
  static Attribute tattr;

  if (!s || !*s) {
    return (nullptr);
  }

  /*
   * Convert the buffer name to lowercase
   */

  buff = alloc_sbuf("attribute_by_name");
  for (p = buff, q = s; *q && ((p - buff) < (SBUF_SIZE - 1)); p++, q++)
    *p = ToUpper(*q);
  *p = '\0';

  /*
   * Look for a predefined attribute
   */

  a = (Attribute *)hash_table_find(buff, &mudstate.attr_name_htab);
  if (a != nullptr) {
    free_sbuf(buff);
    return a;
  }
  /*
   * Nope, look for a user attribute
   */

  va = (VATTR *)vattr_find(buff);
  free_sbuf(buff);

  /*
   * If we got one, load tattr and return a pointer to it.
   */

  if (va != nullptr) {
    tattr.name = va->name;
    tattr.number = va->number;
    tattr.flags = va->flags;
    tattr.check = nullptr;
    return &tattr;
  }
  /*
   * All failed, return NULL
   */

  return nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * anum_extend: Grow the attr num lookup table.
 */

Attribute **anum_table = nullptr;
int anum_alc_top = 0;

void anum_extend(int newtop) {
  Attribute **anum_table2;
  int delta, i;

  delta = mudconf.init_size;

  if (newtop <= anum_alc_top)
    return;
  if (newtop < anum_alc_top + delta)
    newtop = anum_alc_top + delta;
  if (anum_table == nullptr) {
    anum_table = malloc((newtop + 1) * sizeof(Attribute *));
    for (i = 0; i <= newtop; i++)
      anum_table[i] = nullptr;
  } else {
    anum_table2 = malloc((newtop + 1) * sizeof(Attribute *));
    for (i = 0; i <= anum_alc_top; i++)
      anum_table2[i] = anum_table[i];
    for (i = anum_alc_top + 1; i <= newtop; i++)
      anum_table2[i] = nullptr;
    free((char *)anum_table);
    anum_table = anum_table2;
  }
  anum_alc_top = newtop;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_by_number: Look up an attribute by number.
 */

Attribute *attribute_by_number(int anum) {
  VATTR *va;
  static Attribute tattr;

  /*
   * Look for a predefined attribute
   */

  if (anum < A_USER_START)
    return anum_get(anum);

  if (anum >= anum_alc_top)
    return nullptr;

  /*
   * It's a user-defined attribute, we need to copy data
   */

  va = (VATTR *)anum_get(anum);
  if (va != nullptr) {
    tattr.name = va->name;
    tattr.number = va->number;
    tattr.flags = va->flags;
    tattr.check = nullptr;
    return &tattr;
  }
  /*
   * All failed, return NULL
   */

  return nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * mkattr: Lookup attribute by name, creating if needed.
 */

int mkattr(char *buff) {
  Attribute *ap;
  VATTR *va;

  if (!(ap = attribute_by_name(buff))) {

    /*
     * Unknown attr, create a new one
     */

    va = vattr_alloc(buff, mudconf.vattr_flags);
    if (!va || !(va->number))
      return -1;
    return va->number;
  }
  if (!(ap->number))
    return -1;
  return ap->number;
}

/*
 * ---------------------------------------------------------------------------
 * * Commer: check if an object has any $-commands in its attributes.
 */

int has_commands(DbRef thing) {
  char *s, *as, c;
  int attr;
  long aflags;
  DbRef aowner;
  Attribute *ap;

  for (attr = attribute_list_first(thing, &as); attr;
       attr = attribute_list_next(&as)) {
    ap = attribute_by_number(attr);
    if (!ap || (ap->flags & AF_NOPROG))
      continue;

    s = attribute_get(thing, attr, &aowner, &aflags);
    c = *s;
    free_lbuf(s);
    if ((c == '$') && !(aflags & AF_NOPROG)) {
      free(as);
      return 1;
    }
  }
  if (as)
    free(as);
  return 0;
}

/*
 * routines to handle object attribute lists
 */

/*
 * ---------------------------------------------------------------------------
 * * attribute_encode: Encode an attribute string.
 */
static char *attribute_encode(char *iattr, DbRef thing, DbRef owner, long flags,
                              int atr, char *dest_buffer) {

  /*
   * If using the default owner and flags (almost all attributes will),
   * * * * * * * just store the string.
   */

  if (((owner == obj_owner(thing)) || (owner == NOTHING)) && !flags) {
    memset(dest_buffer, 0, LBUF_SIZE);
    strncpy(dest_buffer, iattr, LBUF_SIZE - 1);
    return dest_buffer;
  }

  /*
   * Encode owner and flags into the attribute text
   */

  if (owner == NOTHING)
    owner = obj_owner(thing);
  memset(dest_buffer, 0, LBUF_SIZE);
  snprintf(dest_buffer, LBUF_SIZE - 1, "%c%ld:%ld:%s", ATR_INFO_CHAR, owner,
           flags, iattr);
  return dest_buffer;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_decode: Decode an attribute string.
 */

static void attribute_decode(char *iattr, char *oattr, DbRef thing,
                             DbRef *owner, long *flags, int atr) {
  char *cp;
  int neg;
  int attrOwner;

  /*
   * See if the first char of the attribute is the special character
   */

  if (*iattr == ATR_INFO_CHAR) {

    /*
     * It is, crack the attr apart
     */

    cp = &iattr[1];

    /*
     * Get the attribute owner
     */

    attrOwner = 0;
    neg = 0;
    if (*cp == '-') {
      neg = 1;
      cp++;
    }
    while (isdigit(*cp)) {
      attrOwner = (attrOwner * 10) + (*cp++ - '0');
    }
    if (neg)
      attrOwner = 0 - attrOwner;

    *owner = attrOwner;
    /*
     * If delimiter is not ':', just return attribute
     */

    if (*cp++ != ':') {
      if (owner)
        *owner = obj_owner(thing);
      if (flags)
        *flags = 0;
      if (oattr) {
        StringCopy(oattr, iattr);
      }
      return;
    }
    /*
     * Get the attribute flags
     */

    *flags = 0;
    while (isdigit(*cp)) {
      *flags = (*flags * 10) + (*cp++ - '0');
    }

    /*
     * If delimiter is not ':', just return attribute
     */

    if (*cp++ != ':') {
      if (owner)
        *owner = obj_owner(thing);
      if (flags)
        *flags = 0;
      if (oattr) {
        StringCopy(oattr, iattr);
      }
    }
    /*
     * Get the attribute text
     */

    if (oattr)
      StringCopy(oattr, cp);
    if (attrOwner == NOTHING && owner)
      *owner = obj_owner(thing);
  } else {

    /*
     * Not the special character, return normal info
     */

    if (owner)
      *owner = obj_owner(thing);
    if (flags)
      *flags = 0;
    if (oattr)
      StringCopy(oattr, iattr);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_clear: clear an attribute in the list.
 */

void attribute_clear(DbRef thing, int atr) {
  AttributeList *list;
  int hi, lo, mid;

  if (!db[thing].at_count || !db[thing].ahead)
    return;

  if (db[thing].at_count < 0)
    abort();

  /*
   * Binary search for the attribute.
   */
  lo = 0;
  hi = db[thing].at_count - 1;
  list = db[thing].ahead;
  while (lo <= hi) {
    mid = ((hi - lo) >> 1) + lo;
    if (list[mid].number == atr) {
      free(list[mid].data);
      db[thing].at_count -= 1;
      if (mid != db[thing].at_count)
        bcopy((char *)(list + mid + 1), (char *)(list + mid),
              (db[thing].at_count - mid) * sizeof(AttributeList));
      break;
    } else if (list[mid].number > atr) {
      hi = mid - 1;
    } else {
      lo = mid + 1;
    }
  }

  switch (atr) {
  case A_STARTUP:
    s_flags(thing, obj_flags(thing) & ~HAS_STARTUP);
    break;
  case A_DAILY:
    s_flags2(thing, obj_flags2(thing) & ~HAS_DAILY);
    break;
  case A_HOURLY:
    s_flags2(thing, obj_flags2(thing) & ~HAS_HOURLY);
    break;
  case A_FORWARDLIST:
    s_flags2(thing, obj_flags2(thing) & ~HAS_FWDLIST);
    break;
  case A_LISTEN:
    s_flags2(thing, obj_flags2(thing) & ~HAS_LISTEN);
    break;
  case A_TIMEOUT:
    descriptor_reload(thing);
    break;
  case A_QUEUEMAX:
    pcache_reload(thing);
    break;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_add_raw, attribute_add: add attribute of type atr to list
 */

void attribute_add_raw(DbRef thing, int atr, char *buff) {
  AttributeList *list;
  char *text;
  int found = 0;
  int hi, lo, mid;

  if (!buff || !*buff) {
    attribute_clear(thing, atr);
    return;
  }
  if (strlen(buff) >= LBUF_SIZE) {
    buff[LBUF_SIZE - 1] = '\0';
  }
  if ((text = malloc(strlen(buff) + 1)) == nullptr) {
    return;
  }
  StringCopy(text, buff);

  if (!db[thing].ahead) {
    if ((list = malloc(sizeof(AttributeList))) == nullptr) {
      free(text);
      return;
    }
    db[thing].ahead = list;
    db[thing].at_count = 1;
    list[0].number = atr;
    list[0].data = text;
    list[0].size = strlen(text) + 1;
    found = 1;
  } else {

    /*
     * Binary search for the attribute
     */
    lo = 0;
    hi = db[thing].at_count - 1;

    list = db[thing].ahead;
    while (lo <= hi) {
      mid = ((hi - lo) >> 1) + lo;
      if (list[mid].number == atr) {
        free(list[mid].data);
        list[mid].data = text;
        list[mid].size = strlen(text) + 1;
        found = 1;
        break;
      } else if (list[mid].number > atr) {
        hi = mid - 1;
      } else {
        lo = mid + 1;
      }
    }

    if (!found) {
      /*
       * If we got here, we didn't find it, so lo = hi + 1,
       * and the attribute should be inserted between them.
       */

      list = realloc(db[thing].ahead,
                     (db[thing].at_count + 1) * sizeof(AttributeList));

      if (!list)
        return;

      /*
       * Move the stuff upwards one slot
       */
      if (lo < db[thing].at_count)
        bcopy((char *)(list + lo), (char *)(list + lo + 1),
              (db[thing].at_count - lo) * sizeof(AttributeList));

      list[lo].data = text;
      list[lo].number = atr;
      list[lo].size = strlen(text) + 1;
      db[thing].at_count++;
      db[thing].ahead = list;
    }
  }

  switch (atr) {
  case A_STARTUP:
    s_flags(thing, obj_flags(thing) | HAS_STARTUP);
    break;
  case A_DAILY:
    s_flags2(thing, obj_flags2(thing) | HAS_DAILY);
    break;
  case A_HOURLY:
    s_flags2(thing, obj_flags2(thing) | HAS_HOURLY);
    break;
  case A_FORWARDLIST:
    s_flags2(thing, obj_flags2(thing) | HAS_FWDLIST);
    break;
  case A_LISTEN:
    s_flags2(thing, obj_flags2(thing) | HAS_LISTEN);
    break;
  case A_TIMEOUT:
    descriptor_reload(thing);
    break;
  case A_QUEUEMAX:
    pcache_reload(thing);
    break;
  }
}

void attribute_add(DbRef thing, int atr, char *buff, DbRef owner, long flags) {
  char *tbuff;
  char buffer[LBUF_SIZE];

  if (!buff || !*buff) {
    attribute_clear(thing, atr);
  } else {
    tbuff = attribute_encode(buff, thing, owner, flags, atr, buffer);
    attribute_add_raw(thing, atr, tbuff);
  }
}

void attribute_set_owner(DbRef thing, int atr, DbRef owner) {
  DbRef aowner;
  long aflags;
  char *buff;

  buff = attribute_get(thing, atr, &aowner, &aflags);
  attribute_add(thing, atr, buff, owner, aflags);
  free_lbuf(buff);
}

void attribute_set_flags(DbRef thing, int atr, DbRef flags) {
  DbRef aowner;
  long aflags;
  char *buff;

  buff = attribute_get(thing, atr, &aowner, &aflags);
  attribute_add(thing, atr, buff, aowner, flags);
  free_lbuf(buff);
}

/*
 * ---------------------------------------------------------------------------
 * * get_atr,attribute_get_raw, attribute_get_string, attribute_get: Get an
 * attribute from the database.
 */

int get_atr(char *name) {
  Attribute *ap;

  if (!(ap = attribute_by_name(name)))
    return 0;
  if (!(ap->number))
    return -1;
  return ap->number;
}

char *attribute_get_raw(DbRef thing, int atr) {
  int lo, mid, hi;
  AttributeList *list;

  if (thing < 0)
    return nullptr;

  /*
   * Binary search for the attribute
   */
  lo = 0;
  hi = db[thing].at_count - 1;
  list = db[thing].ahead;
  if (!list)
    return nullptr;

  while (lo <= hi) {
    mid = ((hi - lo) >> 1) + lo;
    if (list[mid].number == atr) {

      return list[mid].data;
    } else if (list[mid].number > atr) {
      hi = mid - 1;
    } else {
      lo = mid + 1;
    }
  }
  return nullptr;
}

char *attribute_get_string(char *s, DbRef thing, int atr, DbRef *owner,
                           long *flags) {
  char *buff;

  buff = attribute_get_raw(thing, atr);
  if (!buff) {
    if (owner)
      *owner = obj_owner(thing);
    if (flags)
      *flags = 0;
    *s = '\0';
  } else {
    attribute_decode(buff, s, thing, owner, flags, atr);
  }
  return s;
}

char *attribute_get(DbRef thing, int atr, DbRef *owner, long *flags) {
  char *buff;

  buff = alloc_lbuf("attribute_get");
  return attribute_get_string(buff, thing, atr, owner, flags);
}

int attribute_get_info(DbRef thing, int atr, DbRef *owner, long *flags) {
  char *buff;

  buff = attribute_get_raw(thing, atr);
  if (!buff) {
    *owner = obj_owner(thing);
    *flags = 0;
    return 0;
  }
  attribute_decode(buff, nullptr, thing, owner, flags, atr);
  return 1;
}

char *attribute_parent_get_string(char *s, DbRef thing, int atr, DbRef *owner,
                                  long *flags) {
  char *buff;
  DbRef parent;
  int lev;

  Attribute *ap;

  ITER_PARENTS(thing, parent, lev) {
    buff = attribute_get_raw(parent, atr);
    if (buff && *buff) {
      attribute_decode(buff, s, thing, owner, flags, atr);
      if ((lev == 0) || !(*flags & AF_PRIVATE))
        return s;
    }
    if ((lev == 0) && is_good_obj(obj_parent(parent))) {
      ap = attribute_by_number(atr);
      if (!ap || ap->flags & AF_PRIVATE)
        break;
    }
  }
  *owner = obj_owner(thing);
  *flags = 0;
  *s = '\0';
  return s;
}

char *attribute_parent_get(DbRef thing, int atr, DbRef *owner, long *flags) {
  char *buff;

  buff = alloc_lbuf("attribute_parent_get");
  return attribute_parent_get_string(buff, thing, atr, owner, flags);
}

int attribute_parent_get_info(DbRef thing, int atr, DbRef *owner, long *flags) {
  char *buff;
  DbRef parent;
  int lev;
  Attribute *ap;

  ITER_PARENTS(thing, parent, lev) {
    buff = attribute_get_raw(parent, atr);
    if (buff && *buff) {
      attribute_decode(buff, nullptr, thing, owner, flags, atr);
      if ((lev == 0) || !(*flags & AF_PRIVATE))
        return 1;
    }
    if ((lev == 0) && is_good_obj(obj_parent(parent))) {
      ap = attribute_by_number(atr);
      if (!ap || ap->flags & AF_PRIVATE)
        break;
    }
  }
  *owner = obj_owner(thing);
  *flags = 0;
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_free: Return all attributes of an object.
 */

void attribute_free(DbRef thing) {
  free(db[thing].ahead);
  db[thing].ahead = nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_copy: Copy all attributes from one object to another.  Takes the
 * * player argument to ensure that only attributes that COULD be set by
 * * the player are copied.
 */

void attribute_copy(DbRef player, DbRef dest, DbRef source) {
  int attr;
  long aflags;
  DbRef owner, aowner;
  char *as, *buf;
  Attribute *at;

  owner = obj_owner(dest);
  for (attr = attribute_list_first(source, &as); attr;
       attr = attribute_list_next(&as)) {
    buf = attribute_get(source, attr, &aowner, &aflags);
    if (!(aflags & AF_LOCK))
      aowner = owner; /*
                       * chg owner
                       */
    at = attribute_by_number(attr);
    if (attr && at) {
      if (write_attr(owner, dest, at, aflags))
        /*
         * Only set attrs that owner has perm to set
         */
        attribute_add(dest, attr, buf, aowner, aflags);
    }
    free_lbuf(buf);
  }
  if (as)
    free(as);
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_change_owner: Change the ownership of the attributes of an object
 * to the
 * * current owner if they are not locked.
 */

void attribute_change_owner(DbRef obj) {
  int attr;
  long aflags;
  DbRef owner, aowner;
  char *as, *buf;

  owner = obj_owner(obj);
  for (attr = attribute_list_first(obj, &as); attr;
       attr = attribute_list_next(&as)) {
    buf = attribute_get(obj, attr, &aowner, &aflags);
    if ((aowner != owner) && !(aflags & AF_LOCK))
      attribute_add(obj, attr, buf, owner, aflags);
    free_lbuf(buf);
  }
  if (as)
    free(as);
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_list_next: Return next attribute in attribute list.
 */

int attribute_list_next(char **attrp) {
  ATRCOUNT *atr;

  if (!attrp || !*attrp) {
    return 0;
  } else {
    atr = (ATRCOUNT *)*attrp;
    if (atr->count > db[atr->thing].at_count) {
      free(atr);
      *attrp = nullptr;
      return 0;
    }
    atr->count++;
    return db[atr->thing].ahead[atr->count - 2].number;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_list_first: Returns the head of the attr list for object 'thing'
 */

int attribute_list_first(DbRef thing, char **attrp) {
  ATRCOUNT *atr;

  if (db[thing].at_count) {
    atr = malloc(sizeof(ATRCOUNT));
    atr->thing = thing;
    atr->count = 2;
    *attrp = (char *)atr;
    if (!db[thing].ahead[0].number) {
      free(atr);
      *attrp = nullptr;
      return 0;
    }
    return db[thing].ahead[0].number;
  }
  *attrp = nullptr;
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * db_grow: Extend the struct database.
 */

// So mistaken refs to #-1 won't die.
constexpr int SIZE_HACK = 1;

static void initialize_objects(DbRef first, DbRef last) {
  DbRef thing;

  for (thing = first; thing < last; thing++) {
    memset(&db[thing], 0, sizeof(db[0]));
    s_owner(thing, GOD);
    s_flags(thing, (TYPE_GARBAGE | GOING));
    s_flags2(thing, 0);
    s_flags3(thing, 0);
    s_powers(thing, 0);
    s_powers2(thing, 0);
    s_location(thing, NOTHING);
    s_contents(thing, NOTHING);
    s_exits(thing, NOTHING);
    s_link(thing, NOTHING);
    s_next(thing, NOTHING);
    s_zone(thing, NOTHING);
    s_parent(thing, NOTHING);
    s_stack(thing, nullptr);
    db[thing].ahead = nullptr;
    db[thing].at_count = 0;
  }
}

void db_grow(DbRef newtop) {
  int newsize, marksize, delta, i;
  MARKBUF *newmarkbuf;
  GameObject *newdb;
  NAME *newpurenames;

  char *cp;

  delta = mudconf.init_size;

  /*
   * Determine what to do based on requested size, current top and  * *
   *
   * *  * *  * *  * * size.  Make sure we grow in reasonable-sized
   * chunks to * * prevent *  * *  * frequent reallocations of the db
   * array.
   */

  /*
   * If requested size is smaller than the current db size, ignore it
   */

  if (newtop <= mudstate.db_top) {
    return;
  }
  /*
   * If requested size is greater than the current db size but smaller
   * * * * * * * than the amount of space we have allocated, raise the
   * db  * *  * size * * and * initialize the new area.
   */

  if (newtop <= mudstate.db_size) {
    for (i = mudstate.db_top; i < newtop; i++) {
      if (mudconf.cache_names)
        purenames[i] = nullptr;
    }
    initialize_objects(mudstate.db_top, newtop);
    mudstate.db_top = newtop;
    return;
  }
  /*
   * Grow by a minimum of delta objects
   */

  if (newtop <= mudstate.db_size + delta) {
    newsize = mudstate.db_size + delta;
  } else {
    newsize = newtop;
  }

  /*
   * Enforce minimumdatabase size
   */

  if (newsize < mudstate.min_size)
    newsize = mudstate.min_size + delta;
  ;

  /*
   * Grow the name tables
   */

  if (mudconf.cache_names) {
    newpurenames = (NAME *)XMALLOC((newsize + SIZE_HACK) * sizeof(NAME),
                                   "db_grow.purenames");

    if (!newpurenames) {
      LOG_SIMPLE(
          LOG_ALWAYS, "ALC", "DB",
          tprintf("Could not allocate space for %d item name cache.", newsize));
      abort();
    }
    bzero((char *)newpurenames, (newsize + SIZE_HACK) * sizeof(NAME));

    if (purenames) {

      /*
       * An old name cache exists.  Copy it.
       */

      purenames -= SIZE_HACK;
      bcopy((char *)purenames, (char *)newpurenames,
            (newtop + SIZE_HACK) * sizeof(NAME));
      cp = (char *)purenames;
      XFREE(cp, "db_grow.purename");
    } else {

      /*
       * Creating a brand new struct database.  Fill in the
       * 'reserved' area in case it gets referenced.
       */

      purenames = newpurenames;
      for (i = 0; i < SIZE_HACK; i++) {
        purenames[i] = nullptr;
      }
    }
    purenames = newpurenames + SIZE_HACK;
    newpurenames = nullptr;
  }
  /*
   * Grow the db array
   */

  newdb = (GameObject *)XMALLOC((newsize + SIZE_HACK) * sizeof(GameObject),
                                "db_grow.db");
  if (!newdb) {

    LOG_SIMPLE(LOG_ALWAYS, "ALC", "DB",
               tprintf("Could not allocate space for %d item struct database.",
                       newsize));
    abort();
  }
  if (db) {

    /*
     * An old struct database exists.  Copy it to the new buffer
     */

    db -= SIZE_HACK;
    bcopy((char *)db, (char *)newdb,
          (mudstate.db_top + SIZE_HACK) * sizeof(GameObject));
    cp = (char *)db;
    XFREE(cp, "db_grow.db");
  } else {

    /*
     * Creating a brand new struct database.  Fill in the * * * *
     *
     * *  * * 'reserved' area in case it gets referenced.
     */

    db = newdb;
    for (i = 0; i < SIZE_HACK; i++) {
      s_owner(i, GOD);
      s_flags(i, (TYPE_GARBAGE | GOING));
      s_powers(i, 0);
      s_powers2(i, 0);
      s_location(i, NOTHING);
      s_contents(i, NOTHING);
      s_exits(i, NOTHING);
      s_link(i, NOTHING);
      s_next(i, NOTHING);
      s_zone(i, NOTHING);
      s_parent(i, NOTHING);
      s_stack(i, nullptr);
      db[i].ahead = nullptr;
      db[i].at_count = 0;
    }
  }
  db = newdb + SIZE_HACK;
  newdb = nullptr;

  for (i = mudstate.db_top; i < newtop; i++) {
    if (mudconf.cache_names) {
      purenames[i] = nullptr;
    }
  }
  initialize_objects(mudstate.db_top, newtop);
  mudstate.db_top = newtop;
  mudstate.db_size = newsize;

  /*
   * Grow the db mark buffer
   */

  marksize = (newsize + 7) >> 3;
  newmarkbuf = (MARKBUF *)XMALLOC(marksize, "db_grow");
  bzero((char *)newmarkbuf, marksize);
  if (mudstate.markbits) {
    marksize = (newtop + 7) >> 3;
    bcopy((char *)mudstate.markbits, (char *)newmarkbuf, marksize);
    cp = (char *)mudstate.markbits;
    XFREE(cp, "db_grow");
  }
  mudstate.markbits = newmarkbuf;
}

void db_free(void) {
  char *cp;

  if (db != nullptr) {
    db -= SIZE_HACK;
    cp = (char *)db;
    XFREE(cp, "db_grow");
    db = nullptr;
  }
  mudstate.db_top = 0;
  mudstate.db_size = 0;
  mudstate.freelist = NOTHING;
}

void db_make_minimal(void) {
  DbRef obj;

  db_free();
  db_grow(1);
  object_name_set(0, "Limbo");
  s_flags(0, TYPE_ROOM);
  s_powers(0, 0);
  s_powers2(0, 0);
  s_location(0, NOTHING);
  s_exits(0, NOTHING);
  s_link(0, NOTHING);
  s_parent(0, NOTHING);
  s_zone(0, NOTHING);
  s_owner(0, 1);
  db[0].ahead = nullptr;
  db[0].at_count = 0;
  /*
   * should be #1
   */
  load_player_names();
  obj = create_player((char *)"Wizard", (char *)"potrzebie", NOTHING, 0);
  s_flags(obj, obj_flags(obj) | WIZARD);
  s_powers(obj, 0);
  s_powers2(obj, 0);

  /*
   * Manually link to Limbo, just in case
   */
  s_location(obj, 0);
  s_next(obj, NOTHING);
  s_contents(0, obj);
  s_link(obj, 0);
}

DbRef parse_dbref(const char *s) {
  const char *p;
  int x;

  /*
   * Enforce completely numeric dbrefs
   */

  for (p = s; *p; p++) {
    if (!isdigit(*p))
      return NOTHING;
  }

  x = atoi(s);
  return ((x >= 0) ? x : NOTHING);
}

void boolean_expression_free(BooleanExpression *b) {
  if (b == TRUE_BOOLEXP)
    return;

  switch (b->type) {
  case BOOLEXP_AND:
  case BOOLEXP_OR:
    boolean_expression_free(b->sub1);
    boolean_expression_free(b->sub2);
    free_bool(b);
    break;
  case BOOLEXP_NOT:
  case BOOLEXP_CARRY:
  case BOOLEXP_IS:
  case BOOLEXP_OWNER:
  case BOOLEXP_INDIR:
    boolean_expression_free(b->sub1);
    free_bool(b);
    break;
  case BOOLEXP_CONST:
    free_bool(b);
    break;
  case BOOLEXP_ATR:
  case BOOLEXP_EVAL:
    free((char *)b->sub1);
    free_bool(b);
    break;
  }
}

BooleanExpression *boolean_expression_duplicate(BooleanExpression *b) {
  BooleanExpression *r;

  if (b == TRUE_BOOLEXP)
    return (TRUE_BOOLEXP);

  r = alloc_bool("boolean_expression_duplicate");
  switch (r->type = b->type) {
  case BOOLEXP_AND:
  case BOOLEXP_OR:
    r->sub2 = boolean_expression_duplicate(b->sub2);
    [[fallthrough]];
  case BOOLEXP_NOT:
  case BOOLEXP_CARRY:
  case BOOLEXP_IS:
  case BOOLEXP_OWNER:
  case BOOLEXP_INDIR:
    r->sub1 = boolean_expression_duplicate(b->sub1);
    [[fallthrough]];
  case BOOLEXP_CONST:
    r->thing = b->thing;
    break;
  case BOOLEXP_EVAL:
  case BOOLEXP_ATR:
    r->thing = b->thing;
    r->sub1 = (BooleanExpression *)strsave((char *)b->sub1);
    break;
  default:
    fprintf(stderr, "bad bool type!!\n");
    free_bool(r);
    return (TRUE_BOOLEXP);
  }
  return (r);
}

/*
 * check_zone - checks back through a zone tree for control
 */
int check_zone(DbRef player, DbRef thing) {
  mudstate.zone_nest_num++;

  if (!mudconf.have_zones || (obj_zone(thing) == NOTHING) ||
      (mudstate.zone_nest_num == mudconf.zone_nest_lim) || (is_player(thing))) {
    mudstate.zone_nest_num = 0;
    return 0;
  }

  /*
   * If the zone doesn't have an enterlock, DON'T allow control.
   */

  if (attribute_get_raw(obj_zone(thing), A_LENTER) &&
      could_doit(player, obj_zone(thing), A_LENTER)) {
    mudstate.zone_nest_num = 0;
    return 1;
  } else {
    return check_zone(player, obj_zone(thing));
  }
}

int check_zone_for_player(DbRef player, DbRef thing) {
  mudstate.zone_nest_num++;

  if (!mudconf.have_zones || (obj_zone(thing) == NOTHING) ||
      (mudstate.zone_nest_num == mudconf.zone_nest_lim) ||
      !(is_player(thing))) {
    mudstate.zone_nest_num = 0;
    return 0;
  }

  if (attribute_get_raw(obj_zone(thing), A_LENTER) &&
      could_doit(player, obj_zone(thing), A_LENTER)) {
    mudstate.zone_nest_num = 0;
    return 1;
  } else {
    return check_zone(player, obj_zone(thing));
  }
}

void toast_player(DbRef player) {
  do_clearcom(player, player, 0);
  do_channelnuke(player);
  del_commac(player);
  do_clear_macro(player, nullptr);
}
