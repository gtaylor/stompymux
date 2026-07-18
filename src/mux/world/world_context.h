/* world_context.h - Explicit dependencies for world and object operations. */

#pragma once

typedef struct AccessControlStore AccessControlStore;
typedef struct GameDatabase GameDatabase;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct WorldIndexes WorldIndexes;

typedef struct WorldContext WorldContext;
struct WorldContext {
  GameDatabase *database;
  ServerConfiguration *configuration;
  WorldIndexes *indexes;
  AccessControlStore *access_control;
  DescriptorRegistry *descriptors;
};

static inline void
world_context_initialize(WorldContext *world, GameDatabase *database,
                         ServerConfiguration *configuration,
                         WorldIndexes *indexes, AccessControlStore *access_control,
                         DescriptorRegistry *descriptors) {
  *world = (WorldContext){
      .database = database,
      .configuration = configuration,
      .indexes = indexes,
      .access_control = access_control,
      .descriptors = descriptors,
  };
}
