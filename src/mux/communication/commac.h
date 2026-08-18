/** @file
 * Communication macro persistence and lookup declarations.
 */
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

/** Executes purge commac. @param[in,out] registry Registry to use.
 * @param[in,out] database Game database. */

void purge_commac(ChannelRegistry *registry, GameDatabase *database);

/** Executes sort com aliases. @param[in,out] c C. */

void sort_com_aliases(struct Commac *c);
/** Returns commac. @param[in] registry Registry to use. @param[in] which Which.
 */

struct Commac *get_commac(ChannelRegistry *registry, DbRef which);
/** Executes create new commac. */

[[nodiscard]] struct Commac *create_new_commac(void);
/** Executes destroy commac. @param[in,out] c C. */

void destroy_commac(struct Commac *c);
/** Executes commac reserve aliases. @param[in,out] commac Commac. @param[in]
 * capacity Capacity. */

[[nodiscard]] bool commac_reserve_aliases(struct Commac *commac,
                                          size_t capacity);
/** Executes add commac. @param[in,out] registry Registry to use. @param[in,out]
 * c C. */

void add_commac(ChannelRegistry *registry, struct Commac *c);
/** Executes del commac. @param[in,out] registry Registry to use. @param[in] who
 * Who. */

void del_commac(ChannelRegistry *registry, DbRef who);
/** Returns commac alias at. @param[in] commac Commac. @param[in] index
 * Zero-based index. */

char *commac_alias_at(const struct Commac *commac, size_t index);
/** Returns commac channel at. @param[in] commac Commac. @param[in] index
 * Zero-based index. */

char *commac_channel_at(const struct Commac *commac, size_t index);
/* Alias entries must be sorted case-insensitively before lookup. */
/** Executes commac channel for alias. @param[in] commac Commac. @param[in]
 * alias Alias. */

const char *commac_channel_for_alias(const struct Commac *commac,
                                     const char *alias);
/** Executes commac channel slot. @param[in,out] commac Commac. @param[in] index
 * Zero-based index. */

char **commac_channel_slot(struct Commac *commac, size_t index);
/** Returns commac macro at. @param[in] commac Commac. @param[in] index
 * Zero-based index. */

int commac_macro_at(const struct Commac *commac, size_t index);
/** Sets commac macro. @param[in,out] commac Commac. @param[in] index Zero-based
 * index. @param[in] value Value to use. */

void commac_macro_set(struct Commac *commac, size_t index, int value);
