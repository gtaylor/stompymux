/* name_table.h - Name-to-value table entry type. */

#pragma once
typedef struct NameTable {
  const char *name;
  int minlen;
  int perm;
  int flag;
} NameTable;
