local Config = require("config")
local State = require("state")
local M = {}

local sqrt = math.sqrt
local floor = math.floor
local abs = math.abs
local ipairs = ipairs

-- Toroidal Distance
function M.dist_sq(x1, y1, x2, y2)
    local dx = abs(x1 - x2)
    local dy = abs(y1 - y2)
    
    if dx > Config.SCREEN_W / 2 then dx = Config.SCREEN_W - dx end
    if dy > Config.SCREEN_H / 2 then dy = Config.SCREEN_H - dy end
    
    return dx*dx + dy*dy
end

-- Get Shortest Vector (Toroidal) - from x1,y1 TO x2,y2
function M.get_vector(x1, y1, x2, y2)
    local dx = x2 - x1
    local dy = y2 - y1
    local hw, hh = Config.SCREEN_W/2, Config.SCREEN_H/2
    
    if dx > hw then dx = dx - Config.SCREEN_W
    elseif dx < -hw then dx = dx + Config.SCREEN_W end
    
    if dy > hh then dy = dy - Config.SCREEN_H
    elseif dy < -hh then dy = dy + Config.SCREEN_H end
    
    return dx, dy
end

function M.closest_point_on_segment(px, py, x1, y1, x2, y2)
    local dx, dy = x2-x1, y2-y1
    if dx==0 and dy==0 then return x1, y1 end
    local t = ((px-x1)*dx + (py-y1)*dy) / (dx*dx + dy*dy)
    if t < 0 then t = 0 elseif t > 1 then t = 1 end
    return x1 + t*dx, y1 + t*dy
end

function M.play_sound_at(name, x, y)
    table.insert(State.frame_sounds, {name=name, x=x, y=y})
end

-- Raycast Helper using Engine DB
function M.cast_ray(px, py, angle, shooter_id)
    if not State.db then return px, py, nil end
    local rad = angle * (math.pi/180)
    
    local id, frac, hx, hy = State.db:cast_ray(px, py, angle, 2000, nil)
    
    if id then
        local obj = State.entity_map[id]
        if obj == State.players[shooter_id] then
             -- Self hit ignored? Logic simplified in original
        end
        return hx, hy, obj
    end
    
    return px + math.cos(rad)*2000, py + math.sin(rad)*2000, nil
end

function M.has_line_of_sight(x1, y1, x2, y2)
    local dx, dy = x2-x1, y2-y1
    local dist = sqrt(dx*dx + dy*dy)
    if dist == 0 then return true end
    local angle = math.atan2(dy, dx) * (180/math.pi)
    
    local id, hit_frac = State.db:cast_ray(x1, y1, angle, dist, "wall")
    if id then
         if hit_frac < 0.98 then return false end
    end
    return true
end

function M.get_closest_node(x, y)
    local Config = require("config")
    local k = floor(x/Config.BASE_SIZE) + floor(y/Config.BASE_SIZE) * Config.GRID_STRIDE
    local c = State.grid_nodes[k]
    if not c then return nil end
    local bn, md = nil, 9e9
    for _, nid in ipairs(c) do
        local n = State.nodes_list[nid]
        if n then
            local d = (x-n.x)^2 + (y-n.y)^2
            if d < md then md = d; bn = nid end
        end
    end
    return bn
end

return M
