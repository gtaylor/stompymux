#pragma once

#include <stddef.h>

typedef struct BtechContext BtechContext;

typedef struct BtechPartCostSet {
  const unsigned long long *costs;
  size_t count;
  int first_part;
} BtechPartCostSet;

enum { BTECH_PART_COST_SET_COUNT = 5 };

void btech_part_costs_initialize(BtechContext *context);
void btech_part_costs_destroy(BtechContext *context);
void btech_part_costs_reset(BtechContext *context);
void btech_part_cost_sets(
    const BtechContext *context,
    BtechPartCostSet sets[static BTECH_PART_COST_SET_COUNT]);
unsigned long long btech_part_cost_get(const BtechContext *context, int part);
void btech_part_cost_set(BtechContext *context, int part,
                         unsigned long long cost);
