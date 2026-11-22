rex.action{
  name = "fixture_success", title = "Run fixture command",
  run = function(ctx)
    assert(ctx.session_id and ctx.block_id, "Missing terminal context")
    local p = assert(rex.block.call("com.superlogical.terminal", "process", {block_id=ctx.block_id}))
    assert(p.foreground.cwd == "/tmp", "Wrong pane")
    return "Fixture ran on the selected pane"
  end,
}
rex.action{
  name = "fixture_failure", title = "Fail fixture command",
  run = function() error("FIXTURE_ACTION_FAILED", 0) end,
}
rex.action{
  name = "fixture_reveal", title = "Reveal fixture tab",
  run = function(ctx)
    local tab = assert(rex.session.new_window{
      window_label = "Revealed fixture",
      focus = false,
      layout = rex.layout.block{flavor="com.superlogical.terminal.shell", options={cwd="/tmp"}},
    })
    rex.client.queue("session.select", {session_id=ctx.session_id, window_id=tab.window_id})
    -- The client must use the queued action, not try to infer navigation from this.
    return {window_id="not the destination"}
  end,
}
rex.action{
  name = "fixture_check_focus", title = "Check fixture tab",
  run = function(ctx)
    local info = assert(rex.session.describe_block{block_id=ctx.block_id})
    local view = assert(rex.session.view{})
    for _, tab in ipairs(view.windows) do
      if tab.window_id == info.block.window_id and tab.label == "Revealed fixture" then
        return "Rexling is viewing the requested tab"
      end
    end
    error("Rexling did not move to the requested tab", 0)
  end,
}
