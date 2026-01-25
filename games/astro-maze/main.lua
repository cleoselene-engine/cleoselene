-- Organic Voronoi-like Maze (Modular Version)
local Config = require("config")
local State = require("state")
local Utils = require("utils")
local Entities = require("entities")
local MapGen = require("map_gen")
local Abilities = require("abilities")
local Renderer = require("renderer")

local min, max, floor = math.min, math.max, math.floor
local sqrt = math.sqrt
local cos, sin = math.cos, math.sin
local pi = math.pi
local atan2 = math.atan2
local random = math.random
local insert = table.insert
local remove = table.remove
local pairs = pairs
local ipairs = ipairs

-- --- Main Game Logic ---

function on_connect(id)
    if State.players[id] then return end
    
    -- DEBUG: Spawn at Top-Left Corner to test Wrap
    local sp = {x=100, y=100} 
    -- local sp = Entities.get_smart_spawn() -- Original
    
    api.play_sound("start")
    
    local p = {
        id=id, 
        x=sp.x, y=sp.y, 
        angle=0, vx=0, vy=0, radius=12, 
        keys={}, inputs={}, 
        color={r=random(100,255), g=random(100,255), b=random(100,255)}, 
        last_shot_timer=2, hp=100, active_ability="laser"
    }
    State.players[id] = p
    Entities.register(p, "circle", p.radius, "player")
    
    local snds = {"laser","die","key","enemy-hit","enemy-noise","enemy-die","wall","door","laser-ready","laser-miss","health-low","start","propulsion","health-up", "minion", "dash", "bomb-bip", "bomb-explosion"}
    for _,s in ipairs(snds) do 
        api.load_sound(s, "/assets/".. (s=="propulsion" and "player-propulsion" or s) .. (s=="laser" and "-shot" or "") .. ".wav") 
    end
end

function on_disconnect(id) 
    if State.players[id] then 
        Entities.unregister(State.players[id])
        State.players[id] = nil 
    end 
end

function on_input(id, code, is_down)
    if State.players[id] then
        local p = State.players[id]
        p.inputs[code] = is_down
        if code == 90 and is_down then p.try_ability = true end
        if code == 38 and is_down then p.thruster_on = not p.thruster_on end
    end
end

-- Wrap Position Helper
local function wrap_pos(obj)
    local wrapped = false
    if obj.x < 0 then obj.x = obj.x + Config.SCREEN_W; wrapped = true
    elseif obj.x > Config.SCREEN_W then obj.x = obj.x - Config.SCREEN_W; wrapped = true end
    
    if obj.y < 0 then obj.y = obj.y + Config.SCREEN_H; wrapped = true
    elseif obj.y > Config.SCREEN_H then obj.y = obj.y - Config.SCREEN_H; wrapped = true end
    return wrapped
end

function update(dt)
    if not State.db or not State.phys then MapGen.generate() end
    
    State.global_time = State.global_time + dt
    State.frame_sounds = {}
    State.phys:step(dt)
    State.phys:step(dt)
    
    -- Process Respawn Queue
    for i=#State.respawn_queue, 1, -1 do
        local r = State.respawn_queue[i]
        r.timer = r.timer - dt
        if r.timer <= 0 then
            local sp = Entities.get_safe_spawn_point()
            if sp then
                Entities.spawn_enemy_at(sp.x, sp.y)
            end
            remove(State.respawn_queue, i)
        end
    end

    -- Process Item Respawn Queue
    for i=#State.item_respawn_queue, 1, -1 do
        local r = State.item_respawn_queue[i]
        r.timer = r.timer - dt
        if r.timer <= 0 then
            Entities.spawn_random_item_at(r.x, r.y)
            remove(State.item_respawn_queue, i)
        end
    end
    
    -- Sync Physics Positions & Wrap
    for id, p in pairs(State.players) do 
        wrap_pos(p)
        if p.phys_id then State.db:update(p.phys_id, p.x, p.y) end 
    end
    for _, e in ipairs(State.enemies) do 
        if e.active then
            wrap_pos(e)
            if e.phys_id then State.db:update(e.phys_id, e.x, e.y) end 
        end
    end
    for _, b in ipairs(State.bombs) do 
        wrap_pos(b)
        if b.phys_id then State.db:update(b.phys_id, b.x, b.y) end 
    end
    for _, s in ipairs(State.shots) do
        -- Manually wrap shots (they don't use physics engine usually, just rays, but we store coords)
        -- Actually shots are raycasts, so wrapping them is weird.
        -- For this simple engine, shots are instant lines.
        -- If we want shots to wrap, we'd need to cast two rays.
        -- Leaving shots unwrapped for now (they just stop at edge of world).
    end

    for id, p in pairs(State.players) do
        -- Ability Trigger
        if p.try_ability then
            p.try_ability = false
            if p.last_shot_timer >= 2.0 then
                p.last_shot_timer = 0
                if p.active_ability == "laser" then Abilities.fire_laser(p)
                elseif p.active_ability == "dash" then Abilities.activate_dash(p)
                elseif p.active_ability == "bomb" then Abilities.spawn_bomb(p)
                elseif p.active_ability == "minion" then Abilities.spawn_minion(p)
                end
            else
                Utils.play_sound_at("laser-miss", p.x, p.y); p.last_shot_timer = 0; p.shake_timer = 0.2
            end
        end

        local prev_ready = (p.last_shot_timer >= 2.0)
        p.last_shot_timer = min(2.0, p.last_shot_timer + dt)
        if not prev_ready and p.last_shot_timer >= 2.0 then
            insert(State.particles, {
                type = "ship_echo",
                x = p.x, y = p.y,
                angle = p.angle,
                color = p.color,
                life = 0.6, max_life = 0.6,
                size_factor = 1.0
            })
            Utils.play_sound_at("laser-ready", p.x, p.y) -- Ensure feedback sound
        end

        if p.damage_timer and p.damage_timer > 0 then p.damage_timer = p.damage_timer - dt end
        if p.shake_timer and p.shake_timer > 0 then p.shake_timer = p.shake_timer - dt end
        if p.blink_timer and p.blink_timer > 0 then p.blink_timer = p.blink_timer - dt end
        
        -- Low Health Logic
        if p.hp < 30 then
            p.low_health_timer = (p.low_health_timer or 0) - dt
            if p.low_health_timer <= 0 then
                Utils.play_sound_at("health-low", p.x, p.y)
                p.low_health_timer = 2.0
            end
        else
            p.low_health_timer = 0
        end

        if p.inputs[37] then p.angle = p.angle - 220 * dt end
        if p.inputs[39] then p.angle = p.angle + 220 * dt end
        p.x = p.x + p.vx * dt
        p.y = p.y + p.vy * dt
        p.vx = p.vx * 0.96
        p.vy = p.vy * 0.96
        
        -- Physics Safety Cap
        local max_v = 800
        if p.vx > max_v then p.vx = max_v elseif p.vx < -max_v then p.vx = -max_v end
        if p.vy > max_v then p.vy = max_v elseif p.vy < -max_v then p.vy = -max_v end
        
        -- NaN Protection
        if p.x ~= p.x then p.x = 100; p.vx = 0 end
        if p.y ~= p.y then p.y = 100; p.vy = 0 end
        
        if p.phys_id then State.db:update(p.phys_id, p.x, p.y) end
        
        -- Dash Logic
        if p.dash_timer and p.dash_timer > 0 then 
            p.dash_timer = p.dash_timer - dt
            -- Reentry FX
            if p.is_dashing then
                local v_rad = atan2(p.vy, p.vx)
                local v_deg = v_rad * (180/pi)
                local rad = p.angle * (pi/180)
                local offsets = {0, -0.6, 0.6} 
                for _, off in ipairs(offsets) do
                    local spawn_rad = rad + off
                    local dc, ds = cos(spawn_rad), sin(spawn_rad)
                    local dist = (off == 0) and 18 or 12
                    insert(State.particles, {x=p.x+dc*dist, y=p.y+ds*dist, angle=v_deg-90, life=0.2, max_life=0.2, size_factor=(off==0) and 1.5 or 1.0})
                end
            end
            if p.dash_timer <= 0 then p.is_dashing = false end
        end

        -- Thruster Logic
        if p.thruster_on then
            local rad = p.angle * (pi/180)
            p.vx = p.vx + cos(rad) * 450 * dt
            p.vy = p.vy + sin(rad) * 450 * dt
            p.thruster_timer = (p.thruster_timer or 0) + dt
            if p.thruster_timer > 0.05 then 
                p.thruster_timer = 0
                local c,s = cos(rad), sin(rad)
                local v_len = sqrt(p.vx^2+p.vy^2)
                local align = 0
                if v_len > 1 then align = (p.vx*c+p.vy*s)/v_len end
                local fac = 0.4+(1.0-align)*0.8
                insert(State.particles, {x=p.x-c*12, y=p.y-s*12, angle=p.angle, life=0.5, max_life=0.5, size_factor=fac}) 
            end
        end

        -- Wall Collision
        p.wall_hit_cooldown = (p.wall_hit_cooldown or 0) - dt
        local walls_near = State.db:query_range(p.x, p.y, p.radius+20, "wall")
        for _, wid in ipairs(walls_near) do
            local w = State.entity_map[wid]
            if w and not w.open then
                 local cx, cy = Utils.closest_point_on_segment(p.x, p.y, w.x1, w.y1, w.x2, w.y2)
                 local dx, dy = p.x-cx, p.y-cy
                 local d_sq = dx*dx + dy*dy
                 if d_sq < (p.radius+2)^2 then 
                    if w.type == "door" and p.keys[w.color_id] then 
                        w.open = true
                        Entities.unregister(w)
                        Utils.play_sound_at("door", w.x1, w.y1)
                    else 
                        local died = false
                        if p.wall_hit_cooldown <= 0 then 
                            Utils.play_sound_at("wall", p.x, p.y)
                            p.wall_hit_cooldown = 0.3
                            p.hp = p.hp - 2
                            p.damage_timer = 0.3
                            if p.hp <= 0 then Entities.respawn_player(p); died = true end 
                        end
                        if not died then 
                            local d = sqrt(d_sq)
                            local nx, ny = 1, 0
                            if d > 0 then nx, ny = dx/d, dy/d end
                            p.x = p.x + nx*(p.radius+2-d)
                            p.y = p.y + ny*(p.radius+2-d)
                            local dot = p.vx*nx + p.vy*ny
                            p.vx = p.vx - 1.5*dot*nx
                            p.vy = p.vy - 1.5*dot*ny
                        end 
                    end 
                 end
            end
        end

        -- Items Pickup
        for _, it in ipairs(State.items) do
            if not it.taken and Utils.dist_sq(p.x, p.y, it.x, it.y) < 600 then
                Entities.pickup_item(it)
                if it.type == "health" then p.hp = min(100, p.hp + 50)
                elseif it.type == "energy" then p.last_shot_timer = 2.0
                else p.active_ability = it.type end
            end
        end
        for _, k in ipairs(State.keys) do 
            if not k.taken and Utils.dist_sq(p.x, p.y, k.x, k.y) < 600 then 
                k.taken = true
                p.keys[k.color_id] = true
                Utils.play_sound_at("key", k.x, k.y) 
            end 
        end
        
        -- Enemy Collision
        p.enemy_hit_cooldown = (p.enemy_hit_cooldown or 0) - dt
        local ens = State.db:query_range(p.x, p.y, p.radius+20, "enemy")
        -- Quick hack for wrapped enemies (not perfect but covers 90% of cases)
        if p.x < 100 then 
            local extra = State.db:query_range(p.x + Config.SCREEN_W, p.y, p.radius+20, "enemy")
            for _, id in ipairs(extra) do insert(ens, id) end
        elseif p.x > Config.SCREEN_W - 100 then
            local extra = State.db:query_range(p.x - Config.SCREEN_W, p.y, p.radius+20, "enemy")
            for _, id in ipairs(extra) do insert(ens, id) end
        end

        for _, eid in ipairs(ens) do
             local e = State.entity_map[eid]
             if e and e.active then
                 local d2 = Utils.dist_sq(p.x, p.y, e.x, e.y)
                 local r_sum = p.radius + e.radius
                 
                 -- Separation logic for owner
                 if e.owner_p == p and e.waiting_separation then
                    if d2 > (r_sum + 5)^2 then
                        e.waiting_separation = nil
                    else
                        goto next_enemy -- Still inside owner after spawn, skip collision
                    end
                 end

                 if d2 < r_sum^2 then 
                     -- Only apply damage/destruction if NOT the owner
                     if e.owner_p ~= p then
                         if p.is_dashing then 
                             Entities.kill_enemy(e)
                             goto next_enemy
                         else
                             -- Normal Collision Damage
                             if p.enemy_hit_cooldown <= 0 then 
                                 Utils.play_sound_at("enemy-hit", p.x, p.y)
                                 p.enemy_hit_cooldown = 0.5
                                 p.hp = p.hp - 25
                                 p.damage_timer = 0.3
                                 if p.hp <= 0 then Entities.respawn_player(p) end 
                                 
                                 if e.owner_p then -- Hostile Minion dies on impact
                                     Entities.kill_enemy(e)
                                     goto next_enemy
                                 end
                             end
                         end
                     end

                     -- Physics Bounce (Applies to everyone, including owner)
                     local d = sqrt(d2)
                     if d > 0 then 
                         -- Use get_vector for wrapped direction
                         local dx, dy = Utils.get_vector(e.x, e.y, p.x, p.y)
                         local nx, ny = dx/d, dy/d
                         
                         p.x = p.x + nx * (r_sum - d)
                         p.vx = p.vx + nx * 100
                         p.vy = p.vy + ny * 100
                     end
                 end
             end
             ::next_enemy::
        end

        -- PvP Collision
        for id2, p2 in pairs(State.players) do if id ~= id2 then 
            local d2 = Utils.dist_sq(p.x, p.y, p2.x, p2.y)
            if d2 < (p.radius + p2.radius)^2 then
                if p.is_dashing and not p2.is_dashing then
                    if p2.enemy_hit_cooldown <= 0 then 
                        Utils.play_sound_at("enemy-hit", p2.x, p2.y)
                        p2.hp = p2.hp - 50
                        p2.enemy_hit_cooldown = 0.5
                        if p2.hp <= 0 then Entities.respawn_player(p2) end 
                    end
                elseif not p.is_dashing and not p2.is_dashing then
                    local died = false
                    if p.enemy_hit_cooldown <= 0 then 
                        Utils.play_sound_at("enemy-hit", p.x, p.y)
                        p.enemy_hit_cooldown = 0.5
                        p.hp = p.hp - 15
                        if p.hp <= 0 then Entities.respawn_player(p); died = true end 
                    end
                    if not died then 
                        local d = sqrt(d2)
                        if d > 0 then 
                            local nx, ny = Utils.get_vector(p2.x, p2.y, p.x, p.y)
                            if nx == 0 and ny == 0 then nx=1 end
                            local l = sqrt(nx*nx+ny*ny)
                            if l > 0 then
                                nx, ny = nx/l, ny/l
                                p.x = p.x + nx * (p.radius + p2.radius - d)
                            end
                        end 
                    end 
                end
            end
        end end
    end
    
    -- Minion vs Enemy Collision Loop
    for _, e in ipairs(State.enemies) do
        if e.active and e.owner_p then -- Is Minion
             local targets = State.db:query_range(e.x, e.y, e.radius + 15, "enemy")
             for _, tid in ipairs(targets) do
                 local t = State.entity_map[tid]
                 if t and t.active and t ~= e and t.owner_p ~= e.owner_p then -- Hostile
                     local d2 = Utils.dist_sq(e.x, e.y, t.x, t.y)
                     if d2 < (e.radius + t.radius)^2 then
                         -- Mutual Destruction
                         Entities.kill_enemy(e)
                         Entities.kill_enemy(t)
                     end
                 end
             end
        end
    end
    
    -- Bomb Loop
    for i=#State.bombs, 1, -1 do
        local b = State.bombs[i]
        
        -- Beep Logic
        b.beep_timer = (b.beep_timer or 0) - dt
        local beep_interval = 0.5
        if b.timer < 1.0 then beep_interval = 0.15 end
        
        if b.beep_timer <= 0 then
            Utils.play_sound_at("bomb-bip", b.x, b.y)
            b.beep_timer = beep_interval
        end
        
        b.timer = b.timer - dt
        b.x = b.x + b.vx * dt
        b.y = b.y + b.vy * dt
        b.vx = b.vx * 0.99
        b.vy = b.vy * 0.99
        
        local walls = State.db:query_range(b.x, b.y, b.radius+10, "wall")
        for _, wid in ipairs(walls) do 
            local w = State.entity_map[wid]
            if w and not w.open then 
                local cx, cy = Utils.closest_point_on_segment(b.x, b.y, w.x1, w.y1, w.x2, w.y2)
                local dx,dy = b.x-cx, b.y-cy
                local d = sqrt(dx*dx+dy*dy)
                if d < b.radius then 
                    local nx,ny = dx/d, dy/d
                    b.x, b.y = b.x + nx*(b.radius-d), b.y + ny*(b.radius-d)
                    local dot = b.vx*nx + b.vy*ny
                    b.vx = b.vx - 1.8*dot*nx
                    b.vy = b.vy - 1.8*dot*ny
                    Utils.play_sound_at("wall", b.x, b.y) 
                end 
            end 
        end
        
        if b.timer <= 0 then
            Utils.play_sound_at("bomb-explosion", b.x, b.y)
            Entities.spawn_explosion(b.x, b.y, 0, 0, {r=255, g=255, b=255})
            
            local t = State.db:query_range(b.x, b.y, 200, nil)
            for _, tid in ipairs(t) do 
                local obj = State.entity_map[tid]
                if obj then 
                    if obj.inputs then 
                        obj.hp = obj.hp - 60
                        if obj.hp <= 0 then Entities.respawn_player(obj) end 
                    elseif obj.active then 
                        Entities.kill_enemy(obj)
                    end 
                end 
            end
            Entities.unregister(b)
            remove(State.bombs, i)
        end
    end
    
    -- Chasers
    local chasers = {}
    for id, p in pairs(State.players) do chasers[id] = {} end
    for i, e in ipairs(State.enemies) do 
        e.target_p = nil
        if e.active then
            local cp, md, cid = nil, 1440000, nil
            for id, p in pairs(State.players) do 
                if e.owner_p ~= p then 
                    local d = Utils.dist_sq(e.x, e.y, p.x, p.y)
                    if d < md then md = d; cp = p; cid = id end 
                end 
            end
            if cp then insert(chasers[cid], {enemy = e, dist = md}) end
        end 
    end
    for id, list in pairs(chasers) do 
        table.sort(list, function(a,b) return a.dist < b.dist end)
        for i=1, min(#list, 4) do list[i].enemy.target_p = State.players[id] end 
    end

    -- Enemy AI
    for _, e in ipairs(State.enemies) do
        if e.active then
            local target = e.target_p
            local ax, ay = 0, 0

            -- 1. Check targeted status (Evasion)
            local need_evasion = false
            for _, p in pairs(State.players) do
                local dx, dy = Utils.get_vector(p.x, p.y, e.x, e.y) -- Vector TO Enemy FROM Player
                local dist_sq_val = dx*dx + dy*dy
                if dist_sq_val < 640000 then -- 800^2
                    local v_p_sq = p.vx^2 + p.vy^2
                    if v_p_sq < 10000 then
                        local prad = p.angle * (pi/180)
                        local pdx, pdy = cos(prad), sin(prad)
                        local dist = sqrt(dist_sq_val)
                        if dist > 0 then
                            local dot = (dx/dist)*pdx + (dy/dist)*pdy
                            if dot > 0.98 and Utils.has_line_of_sight(e.x, e.y, p.x, p.y) then 
                                need_evasion = true
                                break 
                            end
                        end
                    end
                end
            end

            if need_evasion then
                if not e.evade_dir then e.evade_dir = (random() > 0.5) and 1 or -1 end
                e.chase_cooldown = 0.8
            else
                e.evade_dir = nil
            end

            e.chase_cooldown = (e.chase_cooldown or 0) - dt

            -- 2. Calculate Movement Force
            if target and e.chase_cooldown <= 0 then
                if Utils.has_line_of_sight(e.x, e.y, target.x, target.y) then
                    local dx, dy = Utils.get_vector(e.x, e.y, target.x, target.y)
                    local d = sqrt(dx*dx + dy*dy)
                    if d > 0 then ax, ay = (dx/d)*250, (dy/d)*250 end
                else
                    e.path_timer = (e.path_timer or 0) - dt
                    if e.path_timer <= 0 or not e.path then
                        local n1, n2 = Utils.get_closest_node(e.x, e.y), Utils.get_closest_node(target.x, target.y)
                        if n1 and n2 then
                            e.path = State.nav:find_path(n1, n2)
                            e.path_idx = 2
                            e.path_timer = 2.0
                        end
                    end
                    if e.path and e.path[e.path_idx] then
                        local n = State.nodes_list[e.path[e.path_idx]]
                        if n then
                            local dx, dy = n.x - e.x, n.y - e.y
                            local d = sqrt(dx*dx + dy*dy)
                            if d < 40 then e.path_idx = e.path_idx + 1 end
                            if d > 0 then ax, ay = (dx/d)*250, (dy/d)*250 end
                        end
                    end
                end
            elseif need_evasion and target and e.evade_dir then
                local dx, dy = Utils.get_vector(e.x, e.y, target.x, target.y)
                local pdx, pdy = -dy, dx
                local l = sqrt(pdx*pdx + pdy*pdy)
                if l > 0 then
                    ax, ay = (pdx/l) * 400 * e.evade_dir, (pdy/l) * 400 * e.evade_dir
                end
            end

            -- 3. Update Physics State
            e.spin = e.spin + e.spin_speed * dt
            
            -- Noise & Shake Logic
            e.noise_timer = (e.noise_timer or 0) - dt
            if e.noise_timer <= 0 then
                e.noise_timer = 5.0 + random() * 15.0
                e.shake_timer = 0.5
                e.spin_speed = (e.spin_speed or 2) * -1
                Utils.play_sound_at("enemy-noise", e.x, e.y)
            end
            if e.shake_timer and e.shake_timer > 0 then e.shake_timer = e.shake_timer - dt end

            -- Apply Friction & Acceleration
            e.vx = e.vx * 0.95 + ax * dt
            e.vy = e.vy * 0.95 + ay * dt

            -- Cap Velocity
            local v_max = 250
            local v_sq = e.vx*e.vx + e.vy*e.vy
            if v_sq > v_max*v_max then
                local v = sqrt(v_sq)
                e.vx, e.vy = (e.vx/v) * v_max, (e.vy/v) * v_max
            end

            -- Move
            e.x = e.x + e.vx * dt
            e.y = e.y + e.vy * dt

            -- Wall Avoidance/Resolution
            local walls = State.db:query_range(e.x, e.y, 60, "wall")
            for _, wid in ipairs(walls) do
                local w = State.entity_map[wid]
                if w and not w.open then
                    local cx, cy = Utils.closest_point_on_segment(e.x, e.y, w.x1, w.y1, w.x2, w.y2)
                    local dx, dy = e.x - cx, e.y - cy
                    local d_sq = dx*dx + dy*dy
                    -- Resolution
                    if d_sq < e.radius^2 then
                        local d = sqrt(d_sq)
                        if d > 0 then
                            local nx, ny = dx/d, dy/d
                            e.x, e.y = e.x + nx * (e.radius - d), e.y + ny * (e.radius - d)
                        end
                    end
                    -- Avoidance force
                    local d = sqrt(d_sq)
                    if d < 40 and d > 0 then
                        local f = (40 - d) / 40
                        e.vx = e.vx + (dx/d) * 800 * f * dt
                        e.vy = e.vy + (dy/d) * 800 * f * dt
                    end
                end
            end
        end
    end
    
    -- Separation
    for i=1, #State.enemies do 
        local e1 = State.enemies[i]
        if e1.active then 
            for j=i+1, #State.enemies do 
                local e2 = State.enemies[j]
                if e2.active then 
                    local dx, dy = e1.x-e2.x, e1.y-e2.y
                    local d2 = dx*dx+dy*dy
                    if d2 < 900 then 
                        local d = sqrt(d2)
                        local nx, ny, s = dx/d, dy/d, (30-d)*0.5
                        e1.x = e1.x + nx*s
                        e1.y = e1.y + ny*s
                        e2.x = e2.x - nx*s
                        e2.y = e2.y - ny*s 
                    end 
                end 
            end 
        end 
    end

    -- Cleanup
    for i=#State.shards, 1, -1 do 
        local s = State.shards[i]
        s.life = s.life - dt
        if s.life <= 0 then remove(State.shards, i) 
        else 
            s.cx = s.cx + s.vx * dt
            s.cy = s.cy + s.vy * dt
            s.angle = s.angle + s.spin * dt 
        end 
    end
    
    for i=#State.particles, 1, -1 do 
        State.particles[i].life = State.particles[i].life - dt
        if State.particles[i].life <= 0 then remove(State.particles, i) end 
    end
    
    for i=#State.shots, 1, -1 do 
        State.shots[i].life = State.shots[i].life - dt
        if State.shots[i].life <= 0 then remove(State.shots, i) end 
    end
    
    for _, a in ipairs(State.asteroids) do 
        if a.phys_id then 
            local nx, ny = State.db:get_position(a.phys_id)
            if nx then 
                a.x, a.y = nx, ny
                if wrap_pos(a) then
                    State.db:update(a.phys_id, a.x, a.y) 
                elseif a.x < 0 then a.x = Config.SCREEN_W; State.db:update(a.phys_id, a.x, a.y) 
                elseif a.x > Config.SCREEN_W then a.x = 0; State.db:update(a.phys_id, a.x, a.y) end
            end 
        end 
    end
end

function draw(id)
    Renderer.draw(id)
end