/* Declares the BattleTech unit partnames API. */

#pragma once

#include "btech_text_result.h"
#include "mux/server/platform.h"

#include <stdbool.h>
#include <stddef.h>

typedef struct ServerConfiguration ServerConfiguration;
typedef struct BtechContext BtechContext;
typedef struct PartNameEntry PartNameEntry;

typedef struct PartReference {
  int id;
  int brand;
} PartReference;

typedef enum PartMatchKind : int {
  PART_MATCH_SHORT,
  PART_MATCH_LONG,
  PART_MATCH_VERY_LONG,
} PartMatchKind;

typedef struct PartMatchRequest {
  BtechContext *context;
  const char *pattern;
  PartMatchKind kind;
  int cursor;
} PartMatchRequest;

typedef struct PartMatchResult {
  bool found;
  int cursor;
  PartReference part;
} PartMatchResult;

typedef struct PartNameLookupRequest {
  BtechContext *context;
  const char *name;
} PartNameLookupRequest;

/* mech.partnames.c */
void list_phashstats(DbRef player);
void initialize_partname_tables(BtechContext *context);
void destroy_partname_tables(BtechContext *context);
const char *get_parts_short_name(BtechContext *context, int i, int b);
const char *get_parts_long_name(BtechContext *context, int i, int b);
const char *get_parts_vlong_name(BtechContext *context, int i, int b);
PartMatchResult part_match_next(const PartMatchRequest *request);
PartMatchResult part_name_lookup(const PartNameLookupRequest *request);
size_t part_name_count(const BtechContext *context);
const PartNameEntry *part_name_at(const BtechContext *context, size_t index);
void list_forms(DbRef player, void *data, char *buffer);
typedef enum PartNameDescriptionFormat : int {
  PART_NAME_DESCRIPTION_SHORT,
  PART_NAME_DESCRIPTION_LONG,
  PART_NAME_DESCRIPTION_VERY_LONG,
} PartNameDescriptionFormat;

typedef struct PartNameDescriptionRequest {
  BtechContext *context;
  int packed_part;
  PartNameDescriptionFormat format;
} PartNameDescriptionRequest;
BtechTextResult partname_func(const PartNameDescriptionRequest *request);
