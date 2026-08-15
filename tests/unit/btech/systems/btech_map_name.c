#include "map.h"
#include "map_name_api.h"

#include <stdio.h>
#include <string.h>

static bool maximum_length_name_is_terminated(void) {
  BattleMap map = {0};
  const char *name = "abcdefghijklmnopqrstuvwxyz1234";

  memset(map.mapname, 0xAA, sizeof(map.mapname));
  battle_map_name_set(&map, name);

  return strlen(name) == MAP_NAME_SIZE && strcmp(map.mapname, name) == 0 &&
         map.mapname[MAP_NAME_SIZE] == '\0';
}

int main(void) {
  if (maximum_length_name_is_terminated())
    return 0;

  (void)fprintf(stderr,
                "maximum-length map name was truncated or unterminated\n");
  return 1;
}
