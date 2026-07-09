/*
 * db_rw.c
 */

#include "config.h"

#include <sys/file.h>

#include "alloc.h"
#include "attrs.h"
#include "config.h"
#include "db.h"
#include "externs.h"
#include "mudconf.h"
#include "powers.h"
#include "vattr.h"

extern void db_grow(dbref);

extern struct object *db;

static int xml_putescaped(FILE *f, const char *string) {
  char emit_buffer[LBUF_SIZE * 7];
  char *r, *s;
  s = emit_buffer;
  memset(emit_buffer, 0, sizeof(emit_buffer));
  for (r = (char *)string; *r; r++) {
    switch (*r) {
    case '"':
      s = stpcpy(s, "&quot;");
      break;
    case '\'':
      s = stpcpy(s, "&apos;");
      break;
    case '&':
      s = stpcpy(s, "&amp;");
    case '<':
      s = stpcpy(s, "&lt;");
      break;
    case '>':
      s = stpcpy(s, "&gt;");
      break;
    case '\\':
      s = stpcpy(s, "\\\\");
      break;
    default:
      *s++ = *r;
    }
  }
  *s = '\0';
  return fprintf(f, "%s", emit_buffer);
}

static int xml_putobjstring(FILE *f, const char *name, const char *value) {
  fprintf(f, "\t\t<%s>", name);
  xml_putescaped(f, value);
  fprintf(f, "</%s>\n", name);
  return 1;
}

static int xml_putobjref(FILE *f, const char *name, long value) {
  return fprintf(f, "\t\t<%s>%ld</%s>\n", name, value, name);
}

static int xml_putattr(FILE *f, const char *name, const char *value, long owner,
                       long flags) {
  fprintf(f, "\t\t<Attribute name=\"");
  xml_putescaped(f, name);
  fprintf(f, "\" owner=\"%ld\" flags=\"%ld\">", owner, flags);
  xml_putescaped(f, value);
  fprintf(f, "</Attribute>\n");
  return 1;
}

static int xml_db_write_mux(FILE *f, dbref i, int db_format, int flags) {
  ATTR *a;
  char *got, *as;
  dbref aowner;
  int ca, save, j;
  long aflags;

  fprintf(f, "\t<Object dbref=\"%ld\">\n", (long)i);
  xml_putobjstring(f, "Name", Name(i));
  xml_putobjref(f, "Location", Location(i));
  xml_putobjref(f, "Zone", Zone(i));
  xml_putobjref(f, "Contents", Contents(i));
  xml_putobjref(f, "Exits", Exits(i));
  got = atr_get(i, A_LOCK, &aowner, &aflags);
  xml_putobjstring(f, "Lock", got);
  free_lbuf(got);

  xml_putobjref(f, "Link", Link(i));
  xml_putobjref(f, "Owner", Owner(i));
  xml_putobjref(f, "Parent", Parent(i));
  xml_putobjref(f, "Pennies", Pennies(i));
  xml_putobjref(f, "Flags", Flags(i));
  xml_putobjref(f, "Flags2", Flags2(i));
  xml_putobjref(f, "Flags3", Flags3(i));
  xml_putobjref(f, "Powers", Powers(i));
  xml_putobjref(f, "Powers2", Powers2(i));
  for (ca = atr_head(i, &as); ca; ca = atr_next(&as)) {
    save = 0;
    a = atr_num(ca);
    if (a)
      j = a->number;
    else
      j = -1;

    if (j > 0) {
      switch (j) {
      case A_NAME:
        if (flags & V_ATRNAME)
          save = 1;
        break;
      case A_LOCK:
        if (flags & V_ATRKEY)
          save = 1;
        break;
      case A_LIST:
      case A_MONEY:
        break;
      default:
        save = 1;
      }
    }
    if (save) {
      got = atr_get(i, j, &aowner, &aflags);
      xml_putattr(f, a->name, got, aowner, aflags);
    }
  }
  fprintf(f, "\t</Object>\n");
  return 0;
}

dbref xml_db_write(FILE *f, int format, int version) {
  dbref i;
  int flags = 0;

  fprintf(f, "<?xml version=\"1.0\" encoding=\"utf-8\" ?>\n");
  fprintf(f, "<TinyMUXDataBase dumptime=\"%lu\">\n", mudstate.now);
  DO_WHOLE_DB(i) {
    if (!(Going(i))) {
      xml_db_write_mux(f, i, format, flags);
    }
  }
  fprintf(f, "</TinyMUXDataBase>\n");
  fflush(f);
  return (mudstate.db_top);
}
