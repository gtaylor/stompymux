
/*
   p.template.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:33:05 CET 1999 from template.c */

#include "mux/server/platform.h"

#include <stddef.h>
#include <stdio.h>

#pragma once

typedef struct BtechContext BtechContext;
typedef struct Mech Mech;
typedef struct ServerConfiguration ServerConfiguration;

enum { BTECH_TEXT_CAPACITY = 8192 };

size_t primary_technology_name_count(void);
const char *primary_technology_name(size_t index);
size_t secondary_technology_name_count(void);
const char *secondary_technology_name(size_t index);
size_t infantry_technology_name_count(void);
const char *infantry_technology_name(size_t index);
char *template_unit_class_name(size_t index);
char *template_movement_type_name(size_t index);
size_t template_load_command_count(void);
size_t template_section_configuration_count(void);
size_t template_unit_class_count(void);
size_t template_movement_type_count(void);
size_t template_critical_fire_mode_count(void);
size_t template_critical_ammo_mode_count(void);

/* template.c */
int count_special_items(void);
int compare_array(char *const list[], size_t count, const char *command);
int compare_const_array(const char *const list[], size_t count,
                        const char *command);
char *one_arg(char *argument, char *first_arg, size_t first_arg_capacity);
char *one_arg_delim(char *argument, char *first_arg, size_t first_arg_capacity);
char *build_bit_string(const char *const bitdescs[], size_t count, int data,
                       char buffer[static BTECH_TEXT_CAPACITY]);
char *build_bit_string2(const char *const bitdescs[], size_t count,
                        const char *const bitdescs2[], size_t count2, int data,
                        int data2, char buffer[static BTECH_TEXT_CAPACITY]);
char *build_bit_string_delimited2(const char *const bitdescs[], size_t count,
                                  const char *const bitdescs2[], size_t count2,
                                  int data, int data2,
                                  char buffer[static BTECH_TEXT_CAPACITY]);
char *build_bit_string3(const char *const bitdescs[], size_t count,
                        const char *const bitdescs2[], size_t count2,
                        const char *const bitdescs3[], size_t count3, int data,
                        int data2, int data3,
                        char buffer[static BTECH_TEXT_CAPACITY]);
char *my_shortform(const char *source, char buffer[static BTECH_TEXT_CAPACITY]);
char *part_figure_out_shname(int i, char buffer[static BTECH_TEXT_CAPACITY]);
char *part_figure_out_name(const ServerConfiguration *configuration, int i,
                           int brand, char buffer[static BTECH_TEXT_CAPACITY]);
char *part_figure_out_sname(const ServerConfiguration *configuration, int i,
                            int brand, char buffer[static BTECH_TEXT_CAPACITY]);
void dump_locations(FILE *fp, Mech *mech, const char *const locdesc[],
                    size_t location_count);
float generic_computer_multiplier(Mech *mech);
int generic_radio_type(int i, int isClan);
float generic_radio_multiplier(Mech *mech);
void computer_conversion(Mech *mech);
void try_to_find_name(char *mechref, Mech *mech);
int DefaultFuelByType(Mech *mech);
int save_template(DbRef player, Mech *mech, char *reference, char *filename);
char *read_desc(FILE *fp, char *data, char buffer[static BTECH_TEXT_CAPACITY]);
int find_section(char *cmd, int type, int mtype);
long BuildBitVector(const char *const list[], size_t count, char *line);
long BuildBitVectorWithDelim(const char *const list[], size_t count,
                             char *line);
long BuildBitVectorNoErr(const char *const list[], size_t count, char *line);
int CheckSpecialsList(const char *const specials[], size_t count,
                      const char *const specials2[], size_t count2, char *line);
int WeaponIFromString(char *data);
int AmmoIFromString(char *data);
void update_specials(Mech *mech);
int update_oweight(Mech *mech, int value);
int mech_calculated_weight(Mech *mech);
int load_template(DbRef player, Mech *mech, char *filename);
void DumpMechSpecialObjects(BtechContext *context, DbRef player);
void DumpWeapons(BtechContext *context, DbRef player);
char *techlist_func(Mech *mech, char *buffer);
char *payloadlist_func(Mech *mech, char *buffer);
char *partlist_func(Mech *mech, char *buffer);
