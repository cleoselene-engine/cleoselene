local State = require("state")
local Entities = require("entities")
local Config = require("config")
local Main = require("main")

-- Mocks
_G.api = {
    load_image = function() end,
    clear_screen = function() end,
    draw_image = function() end,
    set_color = function() end,
    fill_rect = function() end,
    draw_text = function() end
}

-- Helpers
local function assert_eq(a, b, msg)
    if a ~= b then
        error(string.format("FAIL: %s (Expected %s, Got %s)", msg, tostring(b), tostring(a)))
    else
        print("PASS: " .. msg)
    end
end

-- Tests
local function Test_Dodge()
    print("\n--- Test_Dodge ---")
    State.players = {}
    State.rooms = {}
    
    -- Init Player
    local p = Entities.new_player("p1", "dragon")
    State.players["p1"] = p
    
    -- Create Room (needed for update loop)
    local room = { id=1, p1=p, p2=nil, state="playing" }
    State.rooms[1] = room
    p.room = room
    
    -- Set Initial State
    p.stamina = Config.MAX_STAMINA
    p.x = 100
    p.inputs[39] = true -- Right Arrow Held
    p.inputs[88] = true -- X Key (Kick) Pressed
    p.prev_inputs[88] = false -- Just pressed logic
    
    -- Run Update
    update(0.1)
    
    -- Check State
    assert_eq(p.state, "DODGE", "Player should be in DODGE state")
    
    -- Check Velocity
    -- Note: update() applies friction at the end, so vx won't be exactly DODGE_SPEED (1200)
    -- It will be 1200 * FRICTION (if friction applied) or 1200 if skipped
    -- My fix disabled friction for DODGE.
    assert_eq(p.vx, Config.DODGE_SPEED, "Velocity should match dodge speed")
    
    -- Check Position Change
    local expected_x = 100 + Config.DODGE_SPEED * 0.1
    assert_eq(p.x, expected_x, "Position should update based on velocity")
end

-- Run
Test_Dodge()
print("\nALL TESTS PASSED")
