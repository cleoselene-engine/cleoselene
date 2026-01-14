local Config = require("config")
local M = {}

function M.new_player(id, type)
    return {
        id = id,
        type = type, -- "dragon" or "tiger"
        x = 0,
        y = Config.GROUND_Y - Config.PLAYER_SIZE.h,
        vx = 0,
        vy = 0,
        facing = 1, -- 1 or -1
        hp = Config.MAX_HP,
        stamina = Config.MAX_STAMINA,
        state = "IDLE", 
        timer = 0, -- State lock timer
        anim_time = 0, -- Animation accumulator
        has_hit = false,
        inputs = {},
        prev_inputs = {}
    }
end

return M