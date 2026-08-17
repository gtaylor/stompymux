#pragma once

typedef struct MuxEvent MuxEvent;

void mux_event_tickmech_removesection(MuxEvent *event);
void mux_event_tickmech_removegun(MuxEvent *event);
void mux_event_tickmech_removepart(MuxEvent *event);
void mux_event_tickmech_scrap_failure(MuxEvent *event);
void mux_event_tickmech_repairarmor(MuxEvent *event);
void mux_event_tickmech_repairinternal(MuxEvent *event);
void mux_event_tickmech_reattach(MuxEvent *event);
void mux_event_tickmech_replacesuit(MuxEvent *event);
void mux_event_tickmech_replacegun(MuxEvent *event);
void mux_event_tickmech_repairgun(MuxEvent *event);
void mux_event_tickmech_repairenhcrit(MuxEvent *event);
void mux_event_tickmech_repairpart(MuxEvent *event);
void mux_event_tickmech_reload(MuxEvent *event);
void mux_event_tickmech_mountbomb(MuxEvent *event);
void mux_event_tickmech_umountbomb(MuxEvent *event);
void mux_event_tickmech_reseal(MuxEvent *event);
