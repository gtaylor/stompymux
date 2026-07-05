
/*
 * $Id: mechfile.c,v 1.1 2005/06/13 20:50:49 murrayma Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Created: Tue Oct 29 23:09:59 1996 fingon
 * Last modified: Fri Feb  7 19:37:02 1997 fingon
 *
 */

#include <stdio.h>
#include <string.h>
#include <stdlib.h>

#undef FILES_COMPRESSED_BY_DEFAULT

FILE *my_open_file(char *name, char *mode, int *openway)
{
	FILE *f;
	char *buf;
	char *buf2;
	size_t buflen;
	size_t buf2len;

	if(!strcmp(mode, "w")) {
#ifdef FILES_COMPRESSED_BY_DEFAULT

/*       dup2(2, 1); */
		buflen = strlen("nice gzip -c > ") + strlen(name) + strlen(".gz") + 1;
		buf = malloc(buflen);
		if(!buf)
			return NULL;
		snprintf(buf, buflen, "nice gzip -c > %s.gz", name);
		if(!(f = popen(buf, mode))) {
			free(buf);
			return NULL;
		}
		free(buf);
		*openway = 1;
		return f;
#else
		if(!(f = fopen(name, mode)))
			return NULL;
		*openway = 0;
		return f;
#endif
	}
	if((f = fopen(name, mode))) {
		*openway = 0;
			return f;
	}
	buflen = strlen(name) + strlen(".gz") + 1;
	buf = malloc(buflen);
	if(!buf)
		return NULL;
	snprintf(buf, buflen, "%s.gz", name);
	if((f = fopen(buf, mode)))
		fclose(f);
	else {
		free(buf);
		return NULL;
	}
	buf2len = strlen("nice gzip -dc < ") + strlen(buf) + 1;
	buf2 = malloc(buf2len);
	if(!buf2) {
		free(buf);
		return NULL;
	}
	snprintf(buf2, buf2len, "nice gzip -dc < %s", buf);
	free(buf);

/*   dup2(2, 1); */
	if((f = popen(buf2, mode))) {
		free(buf2);
		*openway = 1;
		return f;
	}
	free(buf2);
	return NULL;
}

void my_close_file(FILE * f, int *openway)
{
	if(!f)
		return;
	if(*openway) {
		pclose(f);

/*       close(1); */
	} else
		fclose(f);
}
