/** @file
 * Explicit dependencies for world and object operations.
 */
#pragma once

typedef struct AccessControlStore AccessControlStore;
typedef struct GameDatabase GameDatabase;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct StyledTextPalette StyledTextPalette;
typedef struct WorldIndexes WorldIndexes;

typedef struct WorldContext WorldContext;
struct WorldContext {
  /* Every member is borrowed from MuxServer. */
  GameDatabase *database;
  ServerConfiguration *configuration;
  WorldIndexes *indexes;
  AccessControlStore *access_control;
  DescriptorRegistry *descriptors;
  StyledTextPalette *styled_text_palette;
};

/** Initializes world context. @param[out] world World. @param[in] database Game
 * database. @param[in] configuration Server configuration. @param[in] indexes
 * Indexes. @param[in] access_control Access control. @param[in] descriptors
 * Descriptors. @param[in] palette Palette. */

static inline void world_context_initialize(WorldContext *world,
                                            GameDatabase *database,
                                            ServerConfiguration *configuration,
                                            WorldIndexes *indexes,
                                            AccessControlStore *access_control,
                                            DescriptorRegistry *descriptors,
                                            StyledTextPalette *palette) {
  *world = (WorldContext){
      .database = database,
      .configuration = configuration,
      .indexes = indexes,
      .access_control = access_control,
      .descriptors = descriptors,
      .styled_text_palette = palette,
  };
}
