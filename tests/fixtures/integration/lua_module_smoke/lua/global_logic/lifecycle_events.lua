return {
  events = {
    on_server_first_startup = function(ctx)
      assert(ctx.scope == "global")
      assert(ctx.object == nil)
    end,
    on_server_startup = function(ctx)
      assert(ctx.scope == "global")
      assert(ctx.object == nil)
    end,
    on_player_connect = function(ctx)
      assert(ctx.scope == "global")
      assert(type(ctx.descriptor) == "number")
      assert(type(ctx.reconnect) == "boolean")
    end,
    on_player_disconnect = function(ctx)
      assert(ctx.scope == "global")
      assert(type(ctx.descriptor) == "number")
      assert(type(ctx.reason) == "string")
    end,
  },
}
