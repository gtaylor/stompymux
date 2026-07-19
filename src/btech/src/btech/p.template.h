
/*
   p.template.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:33:05 CET 1999 from template.c */

#include "mux/server/platform.h"

#pragma once

enum { BTECH_TEXT_CAPACITY = 8192 };

/* template.c */
int count_special_items(void);
int compare_array(char *list[], char *command);
char *one_arg(char *argument, char *first_arg);
char *one_arg_delim(char *argument, char *first_arg);
char *build_bit_string(char *bitdescs[], int data,
                       char buffer[static BTECH_TEXT_CAPACITY]);
char *build_bit_string2(char *bitdescs[], char *bitdescs2[], int data,
                        int data2, char buffer[static BTECH_TEXT_CAPACITY]);
char *build_bit_string_delimited2(char *bitdescs[], char *bitdescs2[], int data,
                                  int data2,
                                  char buffer[static BTECH_TEXT_CAPACITY]);
char *build_bit_string3(char *bitdescs[], char *bitdescs2[], char *bitdescs3[],
                        int data, int data2, int data3,
                        char buffer[static BTECH_TEXT_CAPACITY]);
char *my_shortform(const char *source, char buffer[static BTECH_TEXT_CAPACITY]);
char *part_figure_out_shname(int i, char buffer[static BTECH_TEXT_CAPACITY]);
char *part_figure_out_name(const ServerConfiguration *configuration, int i,
                           int brand, char buffer[static BTECH_TEXT_CAPACITY]);
char *part_figure_out_sname(const ServerConfiguration *configuration, int i,
                            int brand, char buffer[static BTECH_TEXT_CAPACITY]);
void dump_locations(FILE *fp, MECH *mech, const char *locdesc[]);
float generic_computer_multiplier(MECH *mech);
int generic_radio_type(int i, int isClan);
float generic_radio_multiplier(MECH *mech);
void computer_conversion(MECH *mech);
void try_to_find_name(char *mechref, MECH *mech);
int DefaultFuelByType(MECH *mech);
int save_template(DbRef player, MECH *mech, char *reference, char *filename);
char *read_desc(FILE *fp, char *data, char buffer[static BTECH_TEXT_CAPACITY]);
int find_section(char *cmd, int type, int mtype);
long BuildBitVector(char **list, char *line);
long BuildBitVectorWithDelim(char **list, char *line);
long BuildBitVectorNoErr(char **list, char *line);
int CheckSpecialsList(char **specials, char **specials2, char *line);
int WeaponIFromString(char *data);
int AmmoIFromString(char *data);
void update_specials(MECH *mech);
int update_oweight(MECH *mech, int value);
int get_weight(MECH *mech);
int load_template(DbRef player, MECH *mech, char *filename);
void DumpMechSpecialObjects(BtechContext *context, DbRef player);
void DumpWeapons(BtechContext *context, DbRef player);
char *techlist_func(MECH *mech, char *buffer);
char *payloadlist_func(MECH *mech, char *buffer);
char *partlist_func(MECH *mech, char *buffer);
