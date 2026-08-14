#include "mux/communication/commac.h"

#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int reserve_preserves_entries(void) {
  struct Commac *commac = create_new_commac();
  if (!commac_reserve_aliases(commac, 2))
    return 1;

  memcpy(commac_alias_at(commac, 0), "alpha", 6);
  *commac_channel_slot(commac, 0) = strdup("Public");
  commac->numchannels = 1;

  if (!commac_reserve_aliases(commac, 12) || commac->maxchannels != 12 ||
      strcmp(commac_alias_at(commac, 0), "alpha") != 0 ||
      strcmp(commac_channel_at(commac, 0), "Public") != 0) {
    destroy_commac(commac);
    return 1;
  }

  destroy_commac(commac);
  return 0;
}

static int reserve_rejects_overflow_without_mutation(void) {
  struct Commac *commac = create_new_commac();
  if (!commac_reserve_aliases(commac, 2))
    return 1;

  char *aliases = commac->alias;
  char **channels = commac->channels;
  if (commac_reserve_aliases(commac, SIZE_MAX) || commac->alias != aliases ||
      commac->channels != channels || commac->maxchannels != 2) {
    destroy_commac(commac);
    return 1;
  }

  destroy_commac(commac);
  return 0;
}

static int reserve_rejects_unrepresentable_capacity(void) {
  struct Commac *commac = create_new_commac();
  if (!commac_reserve_aliases(commac, 2))
    return 1;

  char *aliases = commac->alias;
  char **channels = commac->channels;
  if (commac_reserve_aliases(commac, (size_t)INT_MAX + 1) ||
      commac->alias != aliases || commac->channels != channels ||
      commac->maxchannels != 2) {
    destroy_commac(commac);
    return 1;
  }

  destroy_commac(commac);
  return 0;
}

int main(void) {
  int failures = 0;
  if (reserve_preserves_entries()) {
    fprintf(stderr, "reserve did not preserve channel aliases\n");
    ++failures;
  }
  if (reserve_rejects_overflow_without_mutation()) {
    fprintf(stderr, "overflow reserve mutated channel aliases\n");
    ++failures;
  }
  if (reserve_rejects_unrepresentable_capacity()) {
    fprintf(stderr, "unrepresentable reserve mutated channel aliases\n");
    ++failures;
  }
  return failures != 0;
}
