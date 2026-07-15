/* commac.h - Communication macro persistence and lookup declarations. */

#pragma once

#include "mux/server/platform.h"

struct commac {
  DbRef who;

  int numchannels;
  int maxchannels;
  char *alias;
  char **channels;

  int curmac;
  int macros[5];

  struct commac *next;
};

constexpr int NUM_COMMAC = 500;

extern struct commac *commac_table[NUM_COMMAC];

void purge_commac(void);

void sort_com_aliases(struct commac *c);
struct commac *get_commac(DbRef which);
struct commac *create_new_commac(void);
void destroy_commac(struct commac *c);
void add_commac(struct commac *c);
void del_commac(DbRef who);
