/* commac.h - Communication macro persistence and lookup declarations. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/communication/channel_registry.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"

typedef struct GameDatabase GameDatabase;
typedef struct ChannelRegistry ChannelRegistry;

constexpr size_t COMMAC_ALIAS_SIZE = 6;
constexpr size_t COMMAC_ALIAS_MAX_LENGTH = COMMAC_ALIAS_SIZE - 1;

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
[[nodiscard]] struct Commac *create_new_commac(void);
void destroy_commac(struct Commac *c);
[[nodiscard]] bool commac_reserve_aliases(struct Commac *commac,
                                          size_t capacity);
void add_commac(ChannelRegistry *registry, struct Commac *c);
void del_commac(ChannelRegistry *registry, DbRef who);
char *commac_alias_at(const struct Commac *commac, size_t index);
char *commac_channel_at(const struct Commac *commac, size_t index);
/* Alias entries must be sorted case-insensitively before lookup. */
const char *commac_channel_for_alias(const struct Commac *commac,
                                     const char *alias);
char **commac_channel_slot(struct Commac *commac, size_t index);
int commac_macro_at(const struct Commac *commac, size_t index);
void commac_macro_set(struct Commac *commac, size_t index, int value);
