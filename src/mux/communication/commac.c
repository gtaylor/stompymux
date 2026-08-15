/* commac.c - Player communication macro storage and lookup. */

#include "mux/communication/commac.h"

#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/support/checked_storage.h"

char *commac_alias_at(const struct Commac *commac, size_t index) {
  if (index >= (size_t)commac->maxchannels)
    abort();
  return checked_storage_at(commac->alias,
                            (size_t)commac->maxchannels * COMMAC_ALIAS_SIZE,
                            sizeof(char), index * COMMAC_ALIAS_SIZE);
}

char *commac_channel_at(const struct Commac *commac, size_t index) {
  return *(char *const *)checked_storage_at_const(
      (const void *)commac->channels, (size_t)commac->numchannels,
      sizeof(*commac->channels), index);
}

const char *commac_channel_for_alias(const struct Commac *commac,
                                     const char *alias) {
  if (commac == nullptr)
    return nullptr;

  int first = 0;
  int last = commac->numchannels - 1;
  int current = 0;
  int direction = 1;

  while (direction != 0 && first <= last) {
    current = (first + last) / 2;
    direction = strcasecmp(alias, commac_alias_at(commac, (size_t)current));
    if (direction < 0)
      last = current - 1;
    else
      first = current + 1;
  }

  return direction == 0 ? commac_channel_at(commac, (size_t)current) : nullptr;
}

char **commac_channel_slot(struct Commac *commac, size_t index) {
  return (char **)checked_storage_at((void *)commac->channels,
                                     (size_t)commac->maxchannels,
                                     sizeof(*commac->channels), index);
}

int commac_macro_at(const struct Commac *commac, size_t index) {
  return *(const int *)checked_storage_at_const(
      commac->macros, sizeof(commac->macros) / sizeof(commac->macros[0]),
      sizeof(*commac->macros), index);
}

void commac_macro_set(struct Commac *commac, size_t index, int value) {
  *(int *)checked_storage_at(commac->macros,
                             sizeof(commac->macros) / sizeof(commac->macros[0]),
                             sizeof(*commac->macros), index) = value;
}

#include "mux/communication/channel_registry.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"

void purge_commac(ChannelRegistry *registry, GameDatabase *database) {
  struct Commac *c;
  struct Commac *d;
  int i;

  for (i = 0; i < COMMAC_BUCKET_COUNT; i++) {
    c = channel_registry_bucket_at(registry, (size_t)i);
    while (c) {
      d = c;
      c = c->next;
      if (d->numchannels == 0 && d->curmac == -1 &&
          commac_macro_at(d, 1) == -1 && commac_macro_at(d, 2) == -1 &&
          commac_macro_at(d, 3) == -1 && commac_macro_at(d, 4) == -1 &&
          commac_macro_at(d, 0) == -1) {
        del_commac(registry, d->who);
        continue;
      }

      if (typeof_obj(database, d->who) == OBJECT_TYPE_PLAYER)
        continue;
      del_commac(registry, d->who);
    }
  }
}

struct Commac *create_new_commac(void) {
  struct Commac *c;
  int i;

  c = checked_storage_allocate(sizeof(*c));

  c->who = -1;
  c->numchannels = 0;
  c->maxchannels = 0;
  c->alias = nullptr;
  c->channels = nullptr;

  c->curmac = -1;
  for (i = 0; i < 5; i++)
    commac_macro_set(c, (size_t)i, -1);

  c->next = nullptr;
  return c;
}

struct Commac *get_commac(ChannelRegistry *registry, DbRef which) {
  struct Commac *c;

  if (which < 0)
    return nullptr;

  c = channel_registry_bucket_at(registry,
                                 (size_t)(which % COMMAC_BUCKET_COUNT));

  while (c && (c->who != which))
    c = c->next;

  if (!c) {
    c = create_new_commac();
    c->who = which;
    add_commac(registry, c);
  }
  return c;
}

void add_commac(ChannelRegistry *registry, struct Commac *c) {
  if (c->who < 0)
    return;

  const size_t BUCKET = (size_t)(c->who % COMMAC_BUCKET_COUNT);

  c->next = channel_registry_bucket_at(registry, BUCKET);
  channel_registry_bucket_set(registry, BUCKET, c);
}

void del_commac(ChannelRegistry *registry, DbRef who) {
  struct Commac *c;
  struct Commac *last;

  if (who < 0)
    return;

  const size_t BUCKET = (size_t)(who % COMMAC_BUCKET_COUNT);

  c = channel_registry_bucket_at(registry, BUCKET);

  if (c == nullptr)
    return;

  if (c->who == who) {
    channel_registry_bucket_set(registry, BUCKET, c->next);
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

void destroy_commac(struct Commac *c) {
  int i;

  free(c->alias);
  for (i = 0; i < c->numchannels; i++)
    free(commac_channel_at(c, (size_t)i));
  free((void *)c->channels);
  free(c);
}

bool commac_reserve_aliases(struct Commac *commac, size_t capacity) {
  if (capacity <= (size_t)commac->maxchannels)
    return true;
  if (capacity > INT_MAX || capacity > SIZE_MAX / COMMAC_ALIAS_SIZE ||
      capacity > SIZE_MAX / sizeof(*commac->channels))
    return false;

  char *aliases =
      checked_storage_try_allocate_array(capacity, COMMAC_ALIAS_SIZE);
  char **channels =
      (char **)checked_storage_try_allocate_array(capacity, sizeof(*channels));
  if (aliases == nullptr || channels == nullptr) {
    free(aliases);
    free((void *)channels);
    return false;
  }

  const size_t COUNT = (size_t)commac->numchannels;
  if (COUNT > 0) {
    memcpy(aliases, commac->alias, COUNT * COMMAC_ALIAS_SIZE);
    memcpy((void *)channels, (const void *)commac->channels,
           COUNT * sizeof(*channels));
  }
  free(commac->alias);
  free((void *)commac->channels);
  commac->alias = aliases;
  commac->channels = channels;
  commac->maxchannels = (int)capacity;
  return true;
}

void sort_com_aliases(struct Commac *c) {
  int i;
  int cont;
  char buffer[10];
  char *s;

  cont = 1;
  while (cont) {
    cont = 0;
    for (i = 0; i < c->numchannels - 1; i++) {
      char *left_alias = commac_alias_at(c, (size_t)i);
      char *right_alias = commac_alias_at(c, (size_t)i + 1);

      if (strcasecmp(left_alias, right_alias) > 0) {
        (void)string_copy_bounded(buffer, sizeof(buffer), left_alias);
        (void)string_copy_bounded(left_alias, COMMAC_ALIAS_SIZE, right_alias);
        (void)string_copy_bounded(right_alias, COMMAC_ALIAS_SIZE, buffer);
        s = commac_channel_at(c, (size_t)i);
        *commac_channel_slot(c, (size_t)i) =
            commac_channel_at(c, (size_t)i + 1);
        *commac_channel_slot(c, (size_t)i + 1) = s;
        cont = 1;
      }
    }
  }
}
