#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "btech/context.h"
#include "equipment_types.h"
#include "map.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/red_black_tree.h"
#include "registry_api.h"
#include "section_types.h"

static int autopilot_hex_offset(int x, int y) { return x * MAPY + y; }

static bool autopilot_hex_bit_is_set(const unsigned char *array, int offset) {
  return array[(unsigned int)offset >> 3] & (1U << (offset & 7));
}

static void autopilot_hex_bit_set(unsigned char *array, int offset,
                                  bool enabled) {
  unsigned int byte = (unsigned int)offset >> 3;
  unsigned char mask = (unsigned char)(1U << (offset & 7));
  if (enabled)
    array[byte] |= mask;
  else
    array[byte] &= (unsigned char)~mask;
}

/* Experimental (highly) path finding system based on the A* 'a-star'
 * system used in many typical games.
 *
 * Dany - 08/2005 */

/*
 * Create an astar node and return a pointer to it
 */
static AutopilotPathNode *auto_create_astar_node(short x, short y,
                                                 short x_parent, short y_parent,
                                                 short g_score, short h_score) {

  AutopilotPathNode *temp;
  temp = malloc(sizeof(AutopilotPathNode));
  if (temp == nullptr)
    return nullptr;

  memset(temp, 0, sizeof(AutopilotPathNode));

  temp->x = x;
  temp->y = y;
  temp->x_parent = x_parent;
  temp->y_parent = y_parent;
  temp->g_score = g_score;
  temp->h_score = h_score;
  temp->f_score = g_score + h_score;
  temp->hexoffset = x * MAPY + y;

  return temp;
}

/*
 * The A* (A-Star) path finding function for the AI
 *
 * Returns 1 if it found a path and 0 if it doesn't
 */
static int astar_compare(void *left_key, void *right_key, void *arg) {
  const long left = (long)left_key;
  const long right = (long)right_key;

  return (left > right) - (left < right);
}
void astar_release(void *key, void *data) { free(data); }
int auto_astar_generate_path(Autopilot *autopilot, Mech *mech, short end_x,
                             short end_y) {
  BattleMap *map =
      btech_context_get_map(autopilot->xcode.context, autopilot->mapindex);
  int found_path = 0;

  /* Our bit arrays */
  unsigned char closed_list_bitfield[(MAPX * MAPY) / 8];
  unsigned char open_list_bitfield[(MAPX * MAPY) / 8];

  float x1, y1, x2, y2;                 /* Floating point vars for real cords */
  short map_x1, map_y1, map_x2, map_y2; /* The actual map 'hexes' */
  int i;
  int child_g_score, child_h_score; /* the score values for the child hexes */
  long hexoffset; /* temp int to pass around as the hexoffset */

  /* Our lists using Hag's RedBlackTree */
  /* Using two RedBlackTree's to store the open_list so we can sort two
   * different ways */
  RedBlackTree open_list_by_score; /* open list sorted by score */
  RedBlackTree open_list_by_xy;    /* open list sorted by hexoffset */
  RedBlackTree closed_list;        /* closed list sorted by hexoffset */

  /* Helper node for the final path */
  DoublyLinkedListNode *astar_path_node;

  /* Our AutopilotPathNode helpers */
  AutopilotPathNode *temp_astar_node;
  AutopilotPathNode *parent_astar_node;

#ifdef DEBUG_ASTAR
  /* Log File */
  FILE *logfile;
  char log_msg[MBUF_SIZE];

  /* Open the logfile */
  logfile = fopen("astar.log", "a");

  /* Write first message */
  snprintf(log_msg, MBUF_SIZE,
           "\nStarting ASTAR Path finding for AI #%d from "
           "%d, %d to %d, %d\n",
           autopilot->mynum, mech_position_x(mech), mech_position_y(mech),
           end_x, end_y);
  fprintf(logfile, "%s", log_msg);
#endif

  /* Zero the bitfields */
  memset(closed_list_bitfield, 0, sizeof(closed_list_bitfield));
  memset(open_list_bitfield, 0, sizeof(open_list_bitfield));

  /* Setup the trees */
  open_list_by_score = red_black_tree_init(astar_compare, nullptr);
  open_list_by_xy = red_black_tree_init(astar_compare, nullptr);
  closed_list = red_black_tree_init(astar_compare, nullptr);

  /* Setup the path */
  /* Destroy any existing path first */
  auto_destroy_astar_path(autopilot);
  autopilot->astar_path = doubly_linked_list_create_list();

  /* Setup the start hex */
  temp_astar_node = auto_create_astar_node(mech_position_x(mech),
                                           mech_position_y(mech), -1, -1, 0, 0);

  if (temp_astar_node == nullptr) {
    /*! \todo {Add code here to break if we can't alloc memory} */

#ifdef DEBUG_ASTAR
    /* Write Log Message */
    snprintf(log_msg, MBUF_SIZE,
             "AI ERROR - Unable to malloc astar node for "
             "hex %d, %d\n",
             mech_position_x(mech), mech_position_y(mech));
    fprintf(logfile, "%s", log_msg);
#endif
  }

  /* Add start hex to open list */
  red_black_tree_insert(open_list_by_score, (void *)temp_astar_node->f_score,
                        temp_astar_node);
  red_black_tree_insert(open_list_by_xy, (void *)temp_astar_node->hexoffset,
                        temp_astar_node);
  autopilot_hex_bit_set(open_list_bitfield, temp_astar_node->hexoffset, true);

#ifdef DEBUG_ASTAR
  /* Log it */
  snprintf(log_msg, MBUF_SIZE, "Added hex %d, %d (%d %d) to open list\n",
           temp_astar_node->x, temp_astar_node->y, temp_astar_node->g_score,
           temp_astar_node->h_score);
  fprintf(logfile, "%s", log_msg);
#endif

  /* Now loop till we find path */
  while (!found_path) {

    /* Check to make sure there is still stuff in the open list
     * if not, means we couldn't find a path so quit */
    if (red_black_tree_size(open_list_by_score) == 0) {
      break;
    }

    /* Get lowest cost node, then remove it from the open list */
    parent_astar_node = (AutopilotPathNode *)red_black_tree_search(
        open_list_by_score, SEARCH_FIRST, nullptr);

    red_black_tree_delete(open_list_by_score,
                          (void *)parent_astar_node->f_score);
    red_black_tree_delete(open_list_by_xy,
                          (void *)parent_astar_node->hexoffset);
    autopilot_hex_bit_set(open_list_bitfield, parent_astar_node->hexoffset,
                          false);

#ifdef DEBUG_ASTAR
    /* Log it */
    snprintf(log_msg, MBUF_SIZE,
             "Removed hex %d, %d (%d %d) from open "
             "list - lowest cost node\n",
             parent_astar_node->x, parent_astar_node->y,
             parent_astar_node->g_score, parent_astar_node->h_score);
    fprintf(logfile, "%s", log_msg);
#endif

    /* Add it to the closed list */
    red_black_tree_insert(closed_list, (void *)parent_astar_node->hexoffset,
                          parent_astar_node);
    autopilot_hex_bit_set(closed_list_bitfield, parent_astar_node->hexoffset,
                          true);

#ifdef DEBUG_ASTAR
    /* Log it */
    snprintf(log_msg, MBUF_SIZE,
             "Added hex %d, %d (%d %d) to closed list"
             " - lowest cost node\n",
             parent_astar_node->x, parent_astar_node->y,
             parent_astar_node->g_score, parent_astar_node->h_score);
    fprintf(logfile, "%s", log_msg);
#endif

    /* Now we check to see if we added the end hex to the closed list.
     * When this happens it means we are done */
    if (autopilot_hex_bit_is_set(closed_list_bitfield,
                                 autopilot_hex_offset(end_x, end_y))) {
      found_path = 1;

#ifdef DEBUG_ASTAR
      fprintf(logfile, "Found path for the AI\n");
#endif

      break;
    }

    /* Update open list */
    /* Loop through the hexes around current hex and see if we can add
     * them to the open list */

    /* Set the parent hex of the new nodes */
    map_x1 = parent_astar_node->x;
    map_y1 = parent_astar_node->y;

    /* Going around clockwise direction */
    for (i = 0; i < 360; i += 60) {

      /* Map coord to Real */
      MapCoordToRealCoord(map_x1, map_y1, &x1, &y1);

      /* Calc new hex */
      FindXY(x1, y1, i, 1.0, &x2, &y2);

      /* Real coord to Map */
      RealCoordToMapCoord(&map_x2, &map_y2, x2, y2);

      /* Make sure the hex is sane */
      if (map_x2 < 0 || map_y2 < 0 || map_x2 >= battle_map_width(map) ||
          map_y2 >= battle_map_height(map))
        continue;

      /* Generate hexoffset for the child node */
      hexoffset = autopilot_hex_offset(map_x2, map_y2);

      /* Check to see if its in the closed list
       * if so just ignore it */
      if (autopilot_hex_bit_is_set(closed_list_bitfield, hexoffset))
        continue;

      /* Check to see if we can enter it */
      if ((mech_class(mech) == CLASS_MECH) &&
          (abs(map_elevation_get(map, map_x1, map_y1) -
               map_elevation_get(map, map_x2, map_y2)) > 2))
        continue;

      if ((mech_class(mech) == CLASS_VEH_GROUND) &&
          (abs(map_elevation_get(map, map_x1, map_y1) -
               map_elevation_get(map, map_x2, map_y2)) > 1))
        continue;

      /* Score the hex */
      /* Right now just assume movement cost from parent to child hex is
       * the same (so 100) no matter which dir we go*/
      /*! \todo {Possibly add in code to make turning less desirable} */
      child_g_score = 100;

      /* Now add the g score from the parent */
      child_g_score += parent_astar_node->g_score;

      /* Next get range */
      /* Using a varient of the Manhattan method since its perfectly
       * logical for us to go diagonally
       *
       * Basicly just going to get the range,
       * and multiply by 100 */
      /*! \todo {Add in something for elevation cost} */

      /* Get the end hex in real coords, using the old variables
       * to store the values */
      MapCoordToRealCoord(end_x, end_y, &x1, &y1);

      /* Re-using the x2 and y2 values we calc'd for the child hex
       * to find the range between the child hex and end hex */
      child_h_score = 100 * FindHexRange(x2, y2, x1, y1);

      /* Lets attempt to avoid hexes that already have our friendlies in it
       * (Stack Check) */
      if (battle_map_mech_count_in_hex(map, map_x2, map_y2, 1,
                                       mech_team(mech)) > 2)
        child_g_score += 150;

      /* Now add in some modifiers for terrain */
      switch (map_terrain_get(map, map_x2, map_y2)) {
      case BATTLE_TERRAIN_LIGHT_FOREST:

        /* Don't bother trying to enter a light forest
         * hex unless we can */
        if ((mech_class(mech) == CLASS_VEH_GROUND) &&
            (mech_movement_type(mech) != MOVE_TRACK))
          continue;

        child_g_score += 50;
        break;
      case BATTLE_TERRAIN_ROUGH:
        child_g_score += 50;
        break;
      case BATTLE_TERRAIN_HEAVY_FOREST:

        /* Don't bother trying to enter a heavy forest
         * hex unless we can */
        if (mech_class(mech) == CLASS_VEH_GROUND)
          continue;

        child_g_score += 100;
        break;
      case BATTLE_TERRAIN_MOUNTAINS:
        child_g_score += 100;
        break;
      case BATTLE_TERRAIN_WATER:

        /* Don't bother trying to enter a water hex
         * unless we can */
        if ((mech_class(mech) == CLASS_VEH_GROUND) &&
            (mech_movement_type(mech) != MOVE_HOVER) &&
            !(mech_technology_flags_secondary(mech) & WATERPROOF_TECH))
          continue;

        /* We really don't want them trying to enter water */
        child_g_score += 200;
        break;
      case BATTLE_TERRAIN_HIGH_WATER:

        /* Don't bother trying to enter a water hex
         * unless we can */
        if ((mech_class(mech) == CLASS_VEH_GROUND) &&
            (mech_movement_type(mech) != MOVE_HOVER) &&
            !(mech_technology_flags_secondary(mech) & WATERPROOF_TECH))
          continue;

        /* We really don't want them trying to enter water */
        child_g_score += 200;
        break;
      default:
        break;
      }

      /* Is it already on the openlist */
      if (autopilot_hex_bit_is_set(open_list_bitfield, hexoffset)) {

        /* Ok need to compare the scores and if necessary recalc
         * and change stuff */

        /* Get the node off the open_list */
        temp_astar_node = (AutopilotPathNode *)red_black_tree_find(
            open_list_by_xy, (void *)hexoffset);

        /* Now compare the 'g_scores' to determine shortest path */
        /* If g_score is lower, this means better path
         * from the current parent node */
        if (child_g_score < temp_astar_node->g_score) {

          /* Remove from open list */
          red_black_tree_delete(open_list_by_score,
                                (void *)temp_astar_node->f_score);
          red_black_tree_delete(open_list_by_xy,
                                (void *)temp_astar_node->hexoffset);
          autopilot_hex_bit_set(open_list_bitfield, temp_astar_node->hexoffset,
                                false);

#ifdef DEBUG_ASTAR
          /* Log it */
          snprintf(log_msg, MBUF_SIZE,
                   "Removed hex %d, %d (%d %d) from "
                   "open list - score recal\n",
                   temp_astar_node->x, temp_astar_node->y,
                   temp_astar_node->g_score, temp_astar_node->h_score);
          fprintf(logfile, "%s", log_msg);
#endif

          /* Recalc score */
          /* H-Score should be the same since the hex doesn't move */
          temp_astar_node->g_score = child_g_score;
          temp_astar_node->f_score =
              temp_astar_node->g_score + temp_astar_node->h_score;

          /* Change parent hex */
          temp_astar_node->x_parent = map_x1;
          temp_astar_node->y_parent = map_y1;

          /* Will re-add the node below */

        } else {

          /* Don't need to do anything so we can skip
           * to the next node */
          continue;
        }

      } else {

        /* Node isn't on the open list so we have to create it */
        temp_astar_node = auto_create_astar_node(map_x2, map_y2, map_x1, map_y1,
                                                 child_g_score, child_h_score);

        if (temp_astar_node == nullptr) {
          /*! \todo {Add code here to break if we can't alloc memory} */

#ifdef DEBUG_ASTAR
          /* Log it */
          snprintf(log_msg, MBUF_SIZE,
                   "AI ERROR - Unable to malloc astar"
                   " node for hex %d, %d\n",
                   map_x2, map_y2);
          fprintf(logfile, "%s", log_msg);
#endif
        }
      }

      /* Now add (or re-add) the node to the open list */

      /* Hack to check to make sure its score is not already on the open
       * list. This slightly skews the results towards nodes found earlier
       * then those found later */
      while (1) {

        if (red_black_tree_exists(open_list_by_score,
                                  (void *)temp_astar_node->f_score)) {
          temp_astar_node->f_score++;

#ifdef DEBUG_ASTAR
          fprintf(logfile,
                  "Adjusting score for hex %d, %d - same"
                  " fscore already exists\n",
                  temp_astar_node->x, temp_astar_node->y);
#endif

        } else {
          break;
        }
      }
      red_black_tree_insert(open_list_by_score,
                            (void *)temp_astar_node->f_score, temp_astar_node);
      red_black_tree_insert(open_list_by_xy, (void *)temp_astar_node->hexoffset,
                            temp_astar_node);
      autopilot_hex_bit_set(open_list_bitfield, temp_astar_node->hexoffset,
                            true);

#ifdef DEBUG_ASTAR
      /* Log it */
      snprintf(log_msg, MBUF_SIZE, "Added hex %d, %d (%d %d) to open list\n",
               temp_astar_node->x, temp_astar_node->y, temp_astar_node->g_score,
               temp_astar_node->h_score);
      fprintf(logfile, "%s", log_msg);
#endif

    } /* End of looking for hexes next to us */

  } /* End of looking for path */

  /* We Done lets go */

  /* Lets first see if we found a path */
  if (found_path) {

#ifdef DEBUG_ASTAR
    /* Log Message */
    fprintf(logfile, "Building Path from closed list for AI\n");
#endif

    /* Found a path so we need to go through the closed list
     * and generate it */

    /* Get the end hex, find its parent hex and work back to
     * start hex while building list */

    /* Get end hex from closed list */
    hexoffset = autopilot_hex_offset(end_x, end_y);
    temp_astar_node = red_black_tree_find(closed_list, (void *)hexoffset);

    /* Add end hex to path list */
    astar_path_node = doubly_linked_list_create_node(temp_astar_node);
    doubly_linked_list_insert_beginning(autopilot->astar_path, astar_path_node);

#ifdef DEBUG_ASTAR
    /* Log it */
    fprintf(logfile, "Added hex %d, %d to path list\n", temp_astar_node->x,
            temp_astar_node->y);
#endif

    /* Remove it from closed list */
    red_black_tree_delete(closed_list, (void *)temp_astar_node->hexoffset);

#ifdef DEBUG_ASTAR
    /* Log it */
    fprintf(logfile, "Removed hex %d, %d from closed list - path list work\n",
            temp_astar_node->x, temp_astar_node->y);
#endif

    /* Check if the end hex is the start hex */
    if (!(temp_astar_node->x == mech_position_x(mech) &&
          temp_astar_node->y == mech_position_y(mech))) {

      /* Its not so lets loop through the closed list
       * building the path */

      /* Loop */
      while (1) {

        /* Get Parent Node Offset */
        hexoffset = autopilot_hex_offset(temp_astar_node->x_parent,
                                         temp_astar_node->y_parent);

        /*! \todo {Possibly add check here incase the node we're
         * looking for some how did not end up on the list} */

        /* Get Parent Node from closed list */
        parent_astar_node = red_black_tree_find(closed_list, (void *)hexoffset);

        /* Check if start hex */
        /* If start hex quit */
        if (parent_astar_node->x == mech_position_x(mech) &&
            parent_astar_node->y == mech_position_y(mech)) {
          break;
        }

        /* Add to path list */
        astar_path_node = doubly_linked_list_create_node(parent_astar_node);
        doubly_linked_list_insert_beginning(autopilot->astar_path,
                                            astar_path_node);

#ifdef DEBUG_ASTAR
        /* Log it */
        fprintf(logfile, "Added hex %d, %d to path list\n",
                parent_astar_node->x, parent_astar_node->y);
#endif

        /* Remove from closed list */
        red_black_tree_delete(closed_list,
                              (void *)parent_astar_node->hexoffset);

#ifdef DEBUG_ASTAR
        /* Log it */
        fprintf(logfile,
                "Removed hex %d, %d from closed list - path list work\n",
                parent_astar_node->x, parent_astar_node->y);
#endif

        /* Make parent new child */
        temp_astar_node = parent_astar_node;

      } /* End of while loop */
    }

    /* Done with the path its cleanup time */
  }

  /* Make sure we destroy all the objects we dont need any more */

#ifdef DEBUG_ASTAR
  /* Log Message */
  fprintf(logfile, "Destorying the AI lists\n");
#endif

  /* Destroy the open lists */
  red_black_tree_release(open_list_by_score, (void *)astar_release, nullptr);
  red_black_tree_destroy(open_list_by_xy);

  /* Destroy the closed list */
  red_black_tree_release(closed_list, (void *)astar_release, nullptr);

#ifdef DEBUG_ASTAR
  /* Close Log file */
  fclose(logfile);
#endif

  /* End */
  if (found_path) {
    return 1;
  } else {
    return 0;
  }
}

/* Function to Smooth out the AI path and remove
 * nodes we don't need */
/* Not even close to being finished yet */
void astar_smooth_path(Autopilot *autopilot) {

  /* Get the n node off the list */

  /* Get the n+1 node off the list */

  /* Get bearing from n to n+1 */

  /* Get n+2 node off list */

  /* Get bearing from n to n+2 node */

  /* Compare bearings */
  /* If in same direction as previous
   * don't need n+1 node */

  /* Keep looping till bearing doesn't match */
  /* Then reset n node to final node and continue */

  return;
}

void auto_destroy_astar_path(Autopilot *autopilot) {

  AutopilotPathNode *temp_astar_node;

  /* Make sure there is a path if not quit */
  if (!(autopilot->astar_path))
    return;

  /* There is a path lets kill it */
  if (doubly_linked_list_size(autopilot->astar_path) > 0) {

    while (doubly_linked_list_size(autopilot->astar_path)) {
      temp_astar_node =
          doubly_linked_list_remove_node_at_pos(autopilot->astar_path, 1);
      free(temp_astar_node);
    }
  }

  /* Finally destroying the path */
  doubly_linked_list_destroy_list(autopilot->astar_path);
  autopilot->astar_path = nullptr;
}
