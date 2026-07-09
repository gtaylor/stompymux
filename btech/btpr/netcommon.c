#include "mudconf.h"
#include "externs.h"
#include "interface.h"

#include <stdio.h>
#include <string.h>

/*
 * Fake notify_printf().  This is supposed to printf-ize a message and queue it
 * on all a player's outgoing network buffers.
 */
void
notify_printf(dbref player, const char *format, ...)
{
	char buffer[LBUF_SIZE];
	va_list ap;

	va_start(ap, format);
	vsnprintf(buffer, LBUF_SIZE, format, ap);
	va_end(ap);

	raw_notify(player, buffer);
}

extern void btshim_raw_notify(dbref, const char *);

void
raw_notify(dbref player, const char *msg)
{
	btshim_raw_notify(player, msg);
}
