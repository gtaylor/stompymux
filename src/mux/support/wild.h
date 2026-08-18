/** @file
 * Wildcard matching interface.
 */
#pragma once

/** Executes wild. @param[in] tstr Tstr. @param[in] dstr Dstr. @param[in,out]
 * args Argument list. @param[in] nargs Nargs. */

bool wild(const char *tstr, const char *dstr, char *args[], int nargs);
/** Executes wild match. @param[in] tstr Tstr. @param[in] dstr Dstr. */

bool wild_match(const char *tstr, const char *dstr);
/** Executes quick wild. @param[in] tstr Tstr. @param[in] dstr Dstr. */

bool quick_wild(const char *tstr, const char *dstr);
