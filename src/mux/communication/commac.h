/* commac.h - Communication macro persistence and lookup declarations. */

#pragma once

#include "mux/server/platform.h"

typedef struct GameDatabase GameDatabase;
typedef struct ChannelRegistry ChannelRegistry;

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

void purge_commac(ChannelRegistry *registry, GameDatabase *database);

void sort_com_aliases(struct commac *c);
struct commac *get_commac(ChannelRegistry *registry, DbRef which);
struct commac *create_new_commac(void);
void destroy_commac(struct commac *c);
void add_commac(ChannelRegistry *registry, struct commac *c);
void del_commac(ChannelRegistry *registry, DbRef who);
