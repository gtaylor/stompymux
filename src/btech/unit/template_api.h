/* Declares the BattleTech template API. */

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
typedef struct TemplateTokenRequest {
  char *input;
  char *output;
  size_t output_capacity;
  bool pipe_delimited;
} TemplateTokenRequest;

typedef struct TemplateBitSet {
  const char *const *descriptions;
  size_t count;
  int bits;
} TemplateBitSet;

typedef struct TemplateBitStringRequest {
  const TemplateBitSet *sets;
  size_t set_count;
  char delimiter;
  char *buffer;
} TemplateBitStringRequest;

typedef struct PartNameRequest {
  const ServerConfiguration *configuration;
  int part;
  int brand;
  bool short_name;
  char *buffer;
} PartNameRequest;

char *template_token_parse(const TemplateTokenRequest *request);
char *template_bit_string_build(const TemplateBitStringRequest *request);
char *my_shortform(const char *source, char buffer[static BTECH_TEXT_CAPACITY]);
char *part_figure_out_shname(int i, char buffer[static BTECH_TEXT_CAPACITY]);
char *part_name_format(const PartNameRequest *request);
void dump_locations(FILE *fp, Mech *mech, const char *const locdesc[],
                    size_t location_count);
float generic_computer_multiplier(Mech *mech);
int generic_radio_type(int i, int is_clan);
float generic_radio_multiplier(Mech *mech);
void computer_conversion(Mech *mech);
void try_to_find_name(const char *mechref, Mech *mech);
int default_fuel_by_type(Mech *mech);
typedef struct TemplateSaveRequest {
  DbRef player;
  Mech *mech;
  const char *reference;
  const char *filename;
} TemplateSaveRequest;
int template_save(const TemplateSaveRequest *request);
typedef struct TemplateDescriptionRead {
  FILE *file;
  char *line;
  char *buffer;
} TemplateDescriptionRead;

char *template_description_read(const TemplateDescriptionRead *request);
int find_section(char *cmd, int type, int mtype);
long build_bit_vector(const char *const list[], size_t count, char *line);
long build_bit_vector_with_delim(const char *const list[], size_t count,
                                 char *line);
long build_bit_vector_no_err(const char *const list[], size_t count,
                             char *line);
int check_specials_list(const char *const special_list[], size_t count,
                        const char *const special_list2[], size_t count2,
                        char *line);
int weapon_i_from_string(char *data);
int ammo_i_from_string(char *data);
void update_specials(Mech *mech);
int update_oweight(Mech *mech, int value);
int mech_calculated_weight(Mech *mech);
int load_template(DbRef player, Mech *mech, char *filename);
void dump_mech_special_objects(BtechContext *context, DbRef player);
void dump_weapons(BtechContext *context, DbRef player);
char *techlist_func(Mech *mech, char *buffer);
char *payloadlist_func(Mech *mech, char *buffer);
char *partlist_func(Mech *mech, char *buffer);
