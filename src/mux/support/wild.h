/* wild.h - Wildcard matching interface. */

#pragma once

bool wild(const char *tstr, const char *dstr, char *args[], int nargs);
bool wild_match(const char *tstr, const char *dstr);
bool quick_wild(const char *tstr, const char *dstr);
