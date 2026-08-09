/* Declares the BattleTech unit partnames API. */

#pragma once

#include "mux/server/platform.h"

#include <stddef.h>

typedef struct ServerConfiguration ServerConfiguration;
typedef struct BtechContext BtechContext;
typedef struct PartNameEntry PartNameEntry;

/* mech.partnames.c */
void list_phashstats(DbRef player);
void initialize_partname_tables(BtechContext *context);
void destroy_partname_tables(BtechContext *context);
const char *get_parts_short_name(BtechContext *context, int i, int b);
const char *get_parts_long_name(BtechContext *context, int i, int b);
const char *get_parts_vlong_name(BtechContext *context, int i, int b);
int find_matching_vlong_part(BtechContext *context, const char *wc, int *ind,
                             int *id, int *brand);
int find_matching_long_part(BtechContext *context, const char *wc, int *i,
                            int *id, int *brand);
int find_matching_short_part(BtechContext *context, const char *wc, int *ind,
                             int *id, int *brand);
size_t part_name_count(const BtechContext *context);
const PartNameEntry *part_name_at(const BtechContext *context, size_t index);
void ListForms(DbRef player, void *data, char *buffer);
const char *partname_func(BtechContext *context, int index, int size);
