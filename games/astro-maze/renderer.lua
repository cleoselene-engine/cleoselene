-- --- Rendering System ---
local Config = require("config")
local State = require("state")
local Utils = require("utils")

local M = {}
local sqrt = math.sqrt
local min, max, floor = math.min, math.max, math.floor
local cos, sin = math.cos, math.sin
local pi = math.pi
local pi2 = pi * 2
local ipairs = ipairs

-- --- Coordinate Transforms ---

local function get_transform_funcs(me)
    local half_world_w, half_world_h = Config.SCREEN_W/2, Config.SCREEN_H/2
    local cx_offset, cy_offset = Config.VIEW_W/2, Config.VIEW_H/2
    local me_x, me_y = me.x, me.y
    local sw, sh = Config.SCREEN_W, Config.SCREEN_H

    local function tx(x) 
        local dx = x - me_x
        if dx > half_world_w then dx = dx - sw
        elseif dx < -half_world_w then dx = dx + sw end
        return cx_offset + dx
    end
    
    local function ty(y) 
        local dy = y - me_y
        if dy > half_world_h then dy = dy - sh
        elseif dy < -half_world_h then dy = dy + sh end
        return cy_offset + dy
    end
    
    return tx, ty
end

-- --- Drawing Helpers ---

local function draw_symbol(cx, cy, type)
    local c = Config.ITEMS[type]
    if not c then return end
    api.set_color(c.r, c.g, c.b)
    
    if c.symbol == "dot" then api.fill_rect(cx-2, cy-2, 4, 4)
    elseif c.symbol == "arrow" then api.draw_line(cx-4, cy+4, cx, cy-4, 1); api.draw_line(cx, cy-4, cx+4, cy+4, 1)
    elseif c.symbol == "circle" then api.draw_line(cx-3, cy-3, cx+3, cy-3, 1); api.draw_line(cx+3, cy-3, cx+3, cy+3, 1); api.draw_line(cx+3, cy+3, cx-3, cy+3, 1); api.draw_line(cx-3, cy+3, cx-3, cy-3, 1)
    elseif c.symbol == "M" then api.draw_line(cx-4, cy+4, cx-4, cy-4, 1); api.draw_line(cx-4, cy-4, cx, cy, 1); api.draw_line(cx, cy, cx+4, cy-4, 1); api.draw_line(cx+4, cy-4, cx+4, cy+4, 1)
    elseif c.symbol == "plus" then api.draw_line(cx-4, cy, cx+4, cy, 2); api.draw_line(cx, cy-4, cx, cy+4, 2)
    elseif c.symbol == "bolt" then api.draw_line(cx+2, cy-4, cx-2, cy, 1); api.draw_line(cx-2, cy, cx+2, cy, 1); api.draw_line(cx+2, cy, cx-2, cy+4, 1)
    end
end

local function draw_player(p, tx, ty)
    local sx, sy = 0, 0
    if p.shake_timer and p.shake_timer > 0 then 
        sx, sy = (math.random()-0.5)*6, (math.random()-0.5)*6 
    end
    
    local dpx, dpy = p.x+sx, p.y+sy
    local vis = true
    
    if p.damage_timer and p.damage_timer > 0 and floor(p.damage_timer*30)%2 == 0 then vis = false end
    
    if vis then
        local cr, cg, cb = p.color.r, p.color.g, p.color.b
        if p.hp < 30 and p.low_health_timer and p.low_health_timer > 1.8 then cr, cg, cb = 255, 0, 0 end
        api.set_color(cr, cg, cb)
        
        local r = p.angle * (pi/180)
        local c, s = cos(r), sin(r)
        -- Increased size for visibility
        local scale = 1.2
        local xA, yA = 14*c*scale, 14*s*scale
        local xB, yB = (-10*c+7*s)*scale, (-10*s-7*c)*scale
        local xC, yC = (-10*c-7*s)*scale, (-10*s+7*c)*scale
        
        -- Thicker lines (2px)
        api.draw_line(tx(dpx+xA), ty(dpy+yA), tx(dpx+xB), ty(dpy+yB), 2)
        api.draw_line(tx(dpx+xB), ty(dpy+yB), tx(dpx+xC), ty(dpy+yC), 2)
        api.draw_line(tx(dpx+xC), ty(dpy+yC), tx(dpx+xA), ty(dpy+yA), 2)
        
        if p.blink_timer and p.blink_timer > 0 then 
            api.set_color(255, 255, 255, 100)
            api.draw_line(tx(dpx+xA), ty(dpy+yA), tx(dpx+xB), ty(dpy+yB), 2)
            api.draw_line(tx(dpx+xB), ty(dpy+yB), tx(dpx+xC), ty(dpy+yC), 2)
            api.draw_line(tx(dpx+xC), ty(dpy+yC), tx(dpx+xA), ty(dpy+yA), 2) 
        end
    end
    
    -- Keys
    for i=1,4 do 
        if p.keys[i] then 
            api.set_color(Config.COLORS[i][1], Config.COLORS[i][2], Config.COLORS[i][3])
            api.fill_rect(tx(dpx)-15+i*6, ty(dpy)+20, 4, 4) 
        end 
    end
end

-- --- Components ---

local function draw_grid(me, tx, ty)
    local grid_sz = 250
    api.set_color(25, 25, 40)
    
    local start_x = floor((me.x - Config.VIEW_W/2)/grid_sz) * grid_sz
    local end_x = start_x + Config.VIEW_W + grid_sz
    for cx = start_x, end_x, grid_sz do
        local nx = cx % Config.SCREEN_W
        local sx = tx(nx)
        api.draw_line(sx, 0, sx, Config.VIEW_H, 1)
    end
    
    local start_y = floor((me.y - Config.VIEW_H/2)/grid_sz) * grid_sz
    local end_y = start_y + Config.VIEW_H + grid_sz
    for cy = start_y, end_y, grid_sz do
        local ny = cy % Config.SCREEN_H
        local sy = ty(ny)
        api.draw_line(0, sy, Config.VIEW_W, sy, 1)
    end
end

local function draw_shadows(cx, cy, segments)
    local angles = {}
    local function add_ang(a) table.insert(angles, a); table.insert(angles, a - 0.0001); table.insert(angles, a + 0.0001) end
    
    for _, s in ipairs(segments) do
        add_ang(math.atan2(s.y1 - cy, s.x1 - cx))
        add_ang(math.atan2(s.y2 - cy, s.x2 - cx))
    end
    
    add_ang(math.atan2(0 - cy, 0 - cx))
    add_ang(math.atan2(0 - cy, Config.VIEW_W - cx))
    add_ang(math.atan2(Config.VIEW_H - cy, Config.VIEW_W - cx))
    add_ang(math.atan2(Config.VIEW_H - cy, 0 - cx))
    
    table.sort(angles)
    
    -- Raycast
    local points = {}
    for _, a in ipairs(angles) do
        local s, c = sin(a), cos(a)
        local min_d = 2000
        local bx, by = cx + c*min_d, cy + s*min_d
        
        for _, seg in ipairs(segments) do
            local dx = seg.x2 - seg.x1
            local dy = seg.y2 - seg.y1
            local det = c * dy - s * dx
            if det ~= 0 then
                local u = ((seg.x1 - cx) * dy - (seg.y1 - cy) * dx) / det
                local v = ((seg.x1 - cx) * s - (seg.y1 - cy) * c) / det
                if u > 0 and u < min_d and v >= 0 and v <= 1 then
                    min_d = u
                    bx, by = cx + c*u, cy + s*u
                end
            end
        end
        table.insert(points, {x=bx, y=by, ang=a})
    end
    
    -- Draw Polygons (Shadows)
    api.set_color(0, 0, 0, 255)
    local max_dist = 3000
    for i=1, #points do
        local p1 = points[i]
        local p2 = points[(i % #points) + 1]
        local ang_diff = p2.ang - p1.ang
        if ang_diff < -pi then ang_diff = ang_diff + pi2 end
        
        if ang_diff > 0 then
             local ex1 = cx + cos(p1.ang) * max_dist
             local ey1 = cy + sin(p1.ang) * max_dist
             local ex2 = cx + cos(p2.ang) * max_dist
             local ey2 = cy + sin(p2.ang) * max_dist
             api.fill_poly({p1.x, p1.y, p2.x, p2.y, ex2, ey2, ex1, ey1})
        end
    end
end

local function draw_hud(me)
    local hp_x, hp_y = 50, 20
    draw_symbol(hp_x - 12, hp_y + 5, "health")
    api.set_color(40, 40, 40, 255)
    api.fill_rect(hp_x, hp_y, 100, 10)
    api.set_color(255, 50, 50, 255)
    api.fill_rect(hp_x, hp_y, max(0, me.hp), 10)
    
    local en_x, en_y = 50, 35
    draw_symbol(en_x - 12, en_y + 3, me.active_ability)
    api.set_color(40, 40, 40, 255)
    api.fill_rect(en_x, en_y, 100, 6)
    
    local en_w = 100
    local fill_en = min(en_w, (me.last_shot_timer / 2.0) * en_w)
    if me.last_shot_timer >= 2.0 then api.set_color(50, 255, 255, 255) else api.set_color(100, 100, 100, 255) end
    api.fill_rect(en_x, en_y, fill_en, 6)
end

local function process_audio(me)
    if me.thruster_on then
        if not me.is_playing_propulsion then 
            api.play_sound("propulsion", true, 0.6)
            me.is_playing_propulsion = true 
        end
        local v_len = sqrt(me.vx^2 + me.vy^2)
        local vol = 0.6
        if v_len > 1 then 
            local rad = me.angle * (pi/180)
            local dx, dy = cos(rad), sin(rad)
            local align = (me.vx * dx + me.vy * dy) / v_len
            vol = max(0.0, min(1.0, 0.6 - 0.4 * align)) 
        end
        api.set_volume("propulsion", vol)
    else 
        if me.is_playing_propulsion then 
            api.stop_sound("propulsion")
            me.is_playing_propulsion = false 
        end 
    end

    for _, snd in ipairs(State.frame_sounds) do
        local d = sqrt(Utils.dist_sq(me.x, me.y, snd.x, snd.y))
        local vol = 1.0 - (d / 1000)
        if vol > 0.001 then 
            if vol > 1.0 then vol = 1.0 end
            api.play_sound(snd.name, false, vol) 
        end
    end
    State.frame_sounds = {}
end

-- --- Main Draw Function ---

function M.draw(session_id)
    if not State.db then return end
    local me = State.players[session_id]
    if not me then return end
    
    api.clear_screen(8, 8, 12)
    
    process_audio(me)
    
    local tx, ty = get_transform_funcs(me)
    local cam_x, cam_y = me.x - Config.VIEW_W/2, me.y - Config.VIEW_H/2
    local pad = 100
    
    -- 1. Background
    draw_grid(me, tx, ty)
    
    -- 2. Calculate Visible Set
    local queries = {}
    table.insert(queries, {l=cam_x - pad, t=cam_y - pad, r=cam_x + Config.VIEW_W + pad, b=cam_y + Config.VIEW_H + pad})
    
    -- Wrapping Logic
    local wx, wy = nil, nil
    if cam_x < 0 then wx = {l=Config.SCREEN_W + cam_x - pad, r=Config.SCREEN_W + pad}
    elseif cam_x + Config.VIEW_W > Config.SCREEN_W then wx = {l=-pad, r=(cam_x + Config.VIEW_W - Config.SCREEN_W) + pad} end
    if wx then table.insert(queries, {l=wx.l, t=cam_y - pad, r=wx.r, b=cam_y + Config.VIEW_H + pad}) end
    
    if cam_y < 0 then wy = {t=Config.SCREEN_H + cam_y - pad, b=Config.SCREEN_H + pad}
    elseif cam_y + Config.VIEW_H > Config.SCREEN_H then wy = {t=-pad, b=(cam_y + Config.VIEW_H - Config.SCREEN_H) + pad} end
    if wy then table.insert(queries, {l=cam_x - pad, t=wy.t, r=cam_x + Config.VIEW_W + pad, b=wy.b}) end
    
    if wx and wy then table.insert(queries, {l=wx.l, t=wy.t, r=wx.r, b=wy.b}) end

    -- 3. Draw Entities & Collect Segments
    local segments = {}
    local drawn_ids = {}
    
    for _, q in ipairs(queries) do
        local visible_ids = State.db:query_rect(q.l, q.t, q.r, q.b, nil)
        for _, id in ipairs(visible_ids) do
            if not drawn_ids[id] then
                drawn_ids[id] = true
                local obj = State.entity_map[id]
                if obj then
                    -- Collect for shadows
                    if obj.type == "wall" or (obj.type == "door" and not obj.open) then
                        table.insert(segments, {x1=tx(obj.x1), y1=ty(obj.y1), x2=tx(obj.x2), y2=ty(obj.y2)})
                    end
                    
                    -- Draw Entity (except local player)
                    if obj ~= me then
                        if obj.type == "wall" or obj.type == "door" then
                            if not obj.open then
                                if obj.type == "door" then api.set_color(Config.COLORS[obj.color_id][1], Config.COLORS[obj.color_id][2], Config.COLORS[obj.color_id][3])
                                else api.set_color(120, 120, 150) end
                                api.draw_line(tx(obj.x1), ty(obj.y1), tx(obj.x2), ty(obj.y2), 1)
                            end
                        elseif obj.active and not obj.inputs then -- Enemy
                            local ex, ey = obj.x, obj.y
                            if obj.shake_timer and obj.shake_timer > 0 then
                                ex = ex + (math.random()-0.5)*6; ey = ey + (math.random()-0.5)*6
                            end
                            if obj.owner_p then api.set_color(obj.color.r, obj.color.g, obj.color.b)
                            else api.set_color(255, 0, 0) end
                            
                            local visual_r = 15
                            local pts = (obj.points or 5) * 2
                            local inner_r, outer_r = visual_r * 0.4, visual_r
                            local lx, ly
                            for i=0, pts do
                                local a = (i/pts)*pi2+(obj.spin or 0)
                                local r = (i%2==0) and outer_r or inner_r
                                local px, py = ex + cos(a)*r, ey + sin(a)*r
                                if i > 0 then api.draw_line(tx(lx), ty(ly), tx(px), ty(py), 2) end
                                lx, ly = px, py
                            end
                        elseif obj.inputs then -- Other Player
                            draw_player(obj, tx, ty)
                        end
                    end
                end
            end
        end
    end
    
    -- Items, keys, particles, projectiles
    -- (Simplified Loop from previous code, consolidated)
    -- Items
    for _, it in ipairs(State.items) do if not it.taken then
        local c = Config.ITEMS[it.type]
        api.set_color(c.r, c.g, c.b)
        local bx, by = tx(it.x)-10, ty(it.y)-10
        api.draw_line(bx, by, bx+20, by, 1); api.draw_line(bx+20, by, bx+20, by+20, 1)
        api.draw_line(bx+20, by+20, bx, by+20, 1); api.draw_line(bx, by+20, bx, by, 1)
        draw_symbol(tx(it.x), ty(it.y), it.type)
    end end
    -- Keys
    for _, k in ipairs(State.keys) do if not k.taken then
        api.set_color(Config.COLORS[k.color_id][1], Config.COLORS[k.color_id][2], Config.COLORS[k.color_id][3])
        api.fill_rect(tx(k.x)-6, ty(k.y)-6, 12, 12)
    end end
    -- Asteroids
    api.set_color(80, 80, 80)
    for _, a in ipairs(State.asteroids) do
        local sx, sy = tx(a.x), ty(a.y)
        if sx>-100 and sx<Config.VIEW_W+100 and sy>-100 and sy<Config.VIEW_H+100 then
            api.fill_rect(sx-a.radius, sy-a.radius, a.radius*2, a.radius*2)
        end
    end
    -- Projectiles
    for _, s in ipairs(State.shots) do
        local p = 1.0 - (s.life / 0.5)
        api.set_color(255, 255, 0, max(0, min(255, floor(255 * (1.0 - p)))))
        api.draw_line(tx(s.x1), ty(s.y1), tx(s.x2), ty(s.y2), 1 + p * 8)
    end
    -- Shards
    for _, s in ipairs(State.shards) do
        local a = floor(255*(s.life/s.max_life))
        if s.color then api.set_color(s.color.r, s.color.g, s.color.b, a) else api.set_color(255, 100, 0, a) end
        local c, sn = cos(s.angle), sin(s.angle)
        local x1, y1 = s.x1*c - s.y1*sn + s.cx, s.x1*sn + s.y1*c + s.cy
        local x2, y2 = s.x2*c - s.y2*sn + s.cx, s.x2*sn + s.y2*c + s.cy
        api.draw_line(tx(x1), ty(y1), tx(x2), ty(y2), 2)
    end
    -- Particles
    for _, pt in ipairs(State.particles) do
        local p = 1.0 - (pt.life/pt.max_life)
        if pt.type == "ship_echo" then
            local scale = 1.0 + p * 2.0
            local alpha = max(0, min(255, floor(255 * (1.0 - p))))
            api.set_color(pt.color.r, pt.color.g, pt.color.b, alpha)
            local rad, c, s = pt.angle*(pi/180), cos(pt.angle*(pi/180)), sin(pt.angle*(pi/180))
            local xA, yA = 14*c*scale, 14*s*scale
            local xB, yB = (-10*c+7*s)*scale, (-10*s-7*c)*scale
            local xC, yC = (-10*c-7*s)*scale, (-10*s+7*c)*scale
            api.draw_line(tx(pt.x+xA), ty(pt.y+yA), tx(pt.x+xB), ty(pt.y+yB), 2)
            api.draw_line(tx(pt.x+xB), ty(pt.y+yB), tx(pt.x+xC), ty(pt.y+yC), 2)
            api.draw_line(tx(pt.x+xC), ty(pt.y+yC), tx(pt.x+xA), ty(pt.y+yA), 2)
        else
            local g = max(0, min(255, floor(255*(1.0-p*0.6))))
            local a = max(0, min(255, floor(200*(1.0-p))))
            api.set_color(255, g, 0, a)
            local rad, c, s = pt.angle*(pi/180), cos(pt.angle*(pi/180)), sin(pt.angle*(pi/180))
            local px, py = -s, c
            local w = 12 * (pt.size_factor or 1.0)
            api.draw_line(tx(pt.x-px*w), ty(pt.y-py*w), tx(pt.x+px*w), ty(pt.y+py*w), 2)
        end
    end
    -- Bombs
    for _, b in ipairs(State.bombs) do
        local r = b.timer/b.max_timer
        local c, g, bl = 255, floor(100*r), floor(100*r)
        if r < 0.3 and (floor(State.global_time*20)%2 == 0) then c, g, bl = 255, 255, 255 end
        api.set_color(c, g, bl)
        local bx, by = tx(b.x), ty(b.y)
        for _, rad in ipairs({b.radius, b.radius*0.6}) do
            local lx, ly
            for i=0, 8 do
                local a = (i/8)*pi2
                local px, py = bx + cos(a)*rad, by + sin(a)*rad
                if i > 0 then api.draw_line(lx, ly, px, py, 1) end
                lx, ly = px, py
            end
        end
        api.fill_rect(bx-1, by-1, 2, 2)
        local da = floor(20 + (1.0-r)*60)
        if r < 0.3 and (floor(State.global_time*15)%2 == 0) then da = 100 end
        api.set_color(255, 0, 0, da)
        local lx, ly
        for i=0, 16 do
            local a = (i/16)*pi2
            local px, py = bx + cos(a)*200, by + sin(a)*200
            if i > 0 then api.draw_line(lx, ly, px, py, 1) end
            lx, ly = px, py
        end
    end

    -- 4. Shadows (Fog of War)
    draw_shadows(Config.VIEW_W/2, Config.VIEW_H/2, segments)
    
    -- 5. Player (Top Layer)
    draw_player(me, tx, ty)
    
    -- 6. HUD
    draw_hud(me)
end

return M