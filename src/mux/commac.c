/*
 * commac.c
 */

#include "config.h"
#include "db.h"
#include "externs.h"
#include "commac.h"

struct commac *commac_table[NUM_COMMAC];

void purge_commac(void) {
  struct commac *c;
  struct commac *d;
  int i;

#ifdef ABORT_PURGE_COMSYS
  return;
#endif /*                                                                      \
        * * ABORT_PURGE_COMSYS                                                 \
        */

  for (i = 0; i < NUM_COMMAC; i++) {
    c = commac_table[i];
    while (c) {
      d = c;
      c = c->next;
      if (d->numchannels == 0 && d->curmac == -1 && d->macros[1] == -1 &&
          d->macros[2] == -1 && d->macros[3] == -1 && d->macros[4] == -1 &&
          d->macros[0] == -1) {
        del_commac(d->who);
        continue;
      }

      /*
       * if ((Typeof(d->who) != TYPE_PLAYER) && (God(Owner(d->who))) &&
       * * (Going(d->who)))
       */
      if (Typeof(d->who) == TYPE_PLAYER)
        continue;
      if (God(Owner(d->who)) && Going(d->who)) {
        del_commac(d->who);
        continue;
      }
    }
  }
}


struct commac *create_new_commac(void) {
  struct commac *c;
  int i;

  c = (struct commac *)malloc(sizeof(struct commac));

  c->who = -1;
  c->numchannels = 0;
  c->maxchannels = 0;
  c->alias = NULL;
  c->channels = NULL;

  c->curmac = -1;
  for (i = 0; i < 5; i++)
    c->macros[i] = -1;

  c->next = NULL;
  return c;
}

struct commac *get_commac(dbref which) {
  struct commac *c;

  if (which < 0)
    return NULL;

  c = commac_table[which % NUM_COMMAC];

  while (c && (c->who != which))
    c = c->next;

  if (!c) {
    c = create_new_commac();
    c->who = which;
    add_commac(c);
  }
  return c;
}

void add_commac(struct commac *c) {
  if (c->who < 0)
    return;

  c->next = commac_table[c->who % NUM_COMMAC];
  commac_table[c->who % NUM_COMMAC] = c;
}

void del_commac(dbref who) {
  struct commac *c;
  struct commac *last;

  if (who < 0)
    return;

  c = commac_table[who % NUM_COMMAC];

  if (c == NULL)
    return;

  if (c->who == who) {
    commac_table[who % NUM_COMMAC] = c->next;
    destroy_commac(c);
    return;
  }
  last = c;
  c = c->next;
  while (c) {
    if (c->who == who) {
      last->next = c->next;
      destroy_commac(c);
      return;
    }
    last = c;
    c = c->next;
  }
}

void destroy_commac(struct commac *c) {
  int i;

  free(c->alias);
  for (i = 0; i < c->numchannels; i++)
    free(c->channels[i]);
  free(c->channels);
  free(c);
}

void sort_com_aliases(struct commac *c) {
  int i;
  int cont;
  char buffer[10];
  char *s;

  cont = 1;
  while (cont) {
    cont = 0;
    for (i = 0; i < c->numchannels - 1; i++)
      if (strcasecmp(c->alias + i * 6, c->alias + (i + 1) * 6) > 0) {
        StringCopy(buffer, c->alias + i * 6);
        StringCopy(c->alias + i * 6, c->alias + (i + 1) * 6);
        StringCopy(c->alias + (i + 1) * 6, buffer);
        s = c->channels[i];
        c->channels[i] = c->channels[i + 1];
        c->channels[i + 1] = s;
        cont = 1;
      }
  }
}
