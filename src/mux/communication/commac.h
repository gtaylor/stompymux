/* commac.h - Communication macro persistence and lookup declarations. */

#pragma once

#include "mux/communication/channel_registry.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"

typedef struct GameDatabase GameDatabase;
typedef struct ChannelRegistry ChannelRegistry;

struct Commac {
  DbRef who;

  int numchannels;
  int maxchannels;
  char *alias;
  char **channels;

  int curmac;
  int macros[5];

  struct Commac *next;
};

void purge_commac(ChannelRegistry *registry, GameDatabase *database);

void sort_com_aliases(struct Commac *c);
struct Commac *get_commac(ChannelRegistry *registry, DbRef which);
struct Commac *create_new_commac(void);
void destroy_commac(struct Commac *c);
void add_commac(ChannelRegistry *registry, struct Commac *c);
void del_commac(ChannelRegistry *registry, DbRef who);
char *commac_alias_at(const struct Commac *commac, size_t index);
char *commac_channel_at(const struct Commac *commac, size_t index);
char **commac_channel_slot(struct Commac *commac, size_t index);
int commac_macro_at(const struct Commac *commac, size_t index);
void commac_macro_set(struct Commac *commac, size_t index, int value);
