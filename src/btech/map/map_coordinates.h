/* Defines shared map-coordinate value objects. */

#pragma once

typedef struct MapHexPosition {
  int x;
  int y;
} MapHexPosition;

typedef struct MapRealPosition {
  float x;
  float y;
} MapRealPosition;

typedef struct MapRealSegment {
  MapRealPosition start;
  MapRealPosition end;
} MapRealSegment;

typedef struct MapSpatialPosition {
  float x;
  float y;
  float z;
} MapSpatialPosition;

typedef struct MapSpatialSegment {
  MapSpatialPosition start;
  MapSpatialPosition end;
} MapSpatialSegment;

typedef struct MapPolarVector {
  float magnitude;
  int bearing;
} MapPolarVector;

typedef struct MapProjection {
  MapRealPosition origin;
  int bearing;
  float range;
} MapProjection;
