local Config = require("config")
local State = require("state")

local M = {}

local floor = math.floor

function M.init()
    local path = "/assets/assets/"
    -- BG
    api.load_image("background", "/assets/assets/bg_hills.png")
    
    -- P1
    api.load_image("p1_idle", "/assets/assets/p1_idle.png")
    api.load_image("p1_run", path .. "p1_run.png")
    api.load_image("p1_attack", path .. "p1_attack.png")
    api.load_image("p1_hit", path .. "p1_hit.png")
    api.load_image("p1_death", path .. "p1_death.png")
    api.load_image("p1_jump", path .. "p1_jump.png")
    api.load_image("p1_fall", path .. "p1_fall.png")
    
    -- P2
    api.load_image("p2_idle", path .. "p2_idle.png")
    api.load_image("p2_run", path .. "p2_run.png")
    api.load_image("p2_attack", path .. "p2_attack.png")
    api.load_image("p2_hit", path .. "p2_hit.png")
    api.load_image("p2_death", path .. "p2_death.png")
    api.load_image("p2_jump", path .. "p2_jump.png")
    api.load_image("p2_fall", path .. "p2_fall.png")
end

local function get_anim_data(p)
    local char_cfg = Config.CHARS[p.type] or Config.CHARS.dragon
    local anim_cfg = char_cfg.anims[p.state] or char_cfg.anims.IDLE
    
    local prefix = (p.type == "dragon") and "p1_" or "p2_"
    local suffix = "idle"
    
    if p.state == "IDLE" then suffix = "idle"
    elseif p.state == "WALK" then suffix = "run"
    elseif p.state == "DODGE" then suffix = "run" -- Reuse Run
    elseif p.state == "JUMP" then
        if p.vy > 0 then suffix = "fall"; anim_cfg = char_cfg.anims.JUMP -- Fall uses Jump frames usually or specific
        else suffix = "jump" end
    elseif p.state == "PUNCH" or p.state == "KICK" then suffix = "attack"
    elseif p.state == "HIT" then suffix = "hit"
    elseif p.state == "DEAD" then suffix = "death"
    end
    
    return prefix .. suffix, anim_cfg.frames, anim_cfg.loop, anim_cfg.speed, char_cfg
end

function M.draw(id)
    api.clear_screen(0, 0, 0)

    local p = State.players[id]
    if not p then return end
    
    local room = p.room
    if not room then
        api.set_color(255, 255, 255)
        api.draw_text("Waiting for opponent...", 300, 300)
        return
    end

    -- Draw BG
    api.draw_image("background", 0, 0, Config.SCREEN_W, Config.SCREEN_H)
    
    -- Draw Players
    local players = {room.p1, room.p2}
    for _, fighter in ipairs(players) do
        if fighter then
            local img, max_frames, loop, speed, char_cfg = get_anim_data(fighter)
            
            local f_idx = floor(fighter.anim_time * speed)
            if loop then
                f_idx = f_idx % max_frames
            else
                if f_idx >= max_frames then f_idx = max_frames - 1 end
            end
            
            local src_size = char_cfg.src_size
            local dest_size = char_cfg.draw_size
            
            local cx = fighter.x + Config.PLAYER_SIZE.w / 2
            local cy = fighter.y + Config.PLAYER_SIZE.h
            
            local dx = cx - (dest_size / 2)
            local dy = cy - dest_size + 80
            
            local final_w = dest_size
            local final_x = dx
            
            local native = char_cfg.native_facing or 1
            local should_flip = (fighter.facing ~= native)
            
            if should_flip then
                final_w = -dest_size
                final_x = dx + dest_size
            end

            -- DODGE GHOSTING EFFECT
            if fighter.state == "DODGE" then
                for i = 1, 3 do
                    local alpha = 150 - (i * 40)
                    local ghost_offset = i * fighter.facing * -20
                    api.set_color(255, 255, 255, alpha)
                    api.draw_image(
                        img, 
                        final_x + ghost_offset, 
                        dy, 
                        final_w, 
                        dest_size, 
                        f_idx * src_size, 0, src_size, src_size
                    )
                end
            end
            
            api.set_color(255, 255, 255, 255)
            api.draw_image(
                img, 
                final_x, 
                dy, 
                final_w, 
                dest_size, 
                f_idx * src_size, 0, src_size, src_size
            )
            
            -- UI: HP Bar
            local bar_x = (fighter == room.p1) and 50 or 450
            local color = (fighter == room.p1) and {50, 255, 50} or {255, 50, 50}
            
            api.set_color(100, 100, 100)
            api.fill_rect(bar_x, 50, 300, 20)
            
            api.set_color(color[1], color[2], color[3])
            api.fill_rect(bar_x, 50, 300 * (fighter.hp / Config.MAX_HP), 20)
            
            -- Stamina Bar
            api.set_color(60, 60, 60)
            api.fill_rect(bar_x, 75, 200, 8)
            api.set_color(255, 255, 0)
            api.fill_rect(bar_x, 75, 200 * (fighter.stamina / Config.MAX_STAMINA), 8)
            
            api.set_color(0, 0, 0)
            api.draw_text(fighter.type, bar_x, 40)
        end
    end
end

return M