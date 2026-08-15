#include "mux/communication/commac.h"
#include "mux/server/platform.h"

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

static int channel_lookup_finds_sorted_aliases(void) {
  struct Commac *commac = create_new_commac();
  if (!commac_reserve_aliases(commac, 3))
    return 1;

  (void)string_copy_bounded(commac_alias_at(commac, 0), COMMAC_ALIAS_SIZE,
                            "alpha");
  (void)string_copy_bounded(commac_alias_at(commac, 1), COMMAC_ALIAS_SIZE,
                            "mid");
  (void)string_copy_bounded(commac_alias_at(commac, 2), COMMAC_ALIAS_SIZE,
                            "omega");
  *commac_channel_slot(commac, 0) = strdup("Alpha Channel");
  *commac_channel_slot(commac, 1) = strdup("Middle Channel");
  *commac_channel_slot(commac, 2) = strdup("Omega Channel");
  commac->numchannels = 3;

  const bool MATCHES =
      commac_channel_for_alias(commac, "alpha") ==
          commac_channel_at(commac, 0) &&
      commac_channel_for_alias(commac, "MID") == commac_channel_at(commac, 1) &&
      commac_channel_for_alias(commac, "omega") ==
          commac_channel_at(commac, 2) &&
      commac_channel_for_alias(commac, "missing") == nullptr;
  destroy_commac(commac);
  return MATCHES ? 0 : 1;
}

static int channel_lookup_handles_empty_and_null_records(void) {
  struct Commac *commac = create_new_commac();
  const bool HANDLES_EMPTY =
      commac_channel_for_alias(commac, "missing") == nullptr;
  destroy_commac(commac);
  return HANDLES_EMPTY &&
                 commac_channel_for_alias(nullptr, "missing") == nullptr
             ? 0
             : 1;
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
  if (channel_lookup_finds_sorted_aliases()) {
    fprintf(stderr, "channel lookup did not find sorted aliases\n");
    ++failures;
  }
  if (channel_lookup_handles_empty_and_null_records()) {
    fprintf(stderr, "channel lookup did not handle empty records\n");
    ++failures;
  }
  return failures != 0;
}
