/* world_context.h - Explicit dependencies for world and object operations. */

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
