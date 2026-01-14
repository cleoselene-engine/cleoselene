local Config = require("config")
local State = require("state")
local Entities = require("entities")
local Renderer = require("renderer")

local min, max, abs, floor = math.min, math.max, math.abs, math.floor

function init()
    Renderer.init()
    print("Dragon Fighters Initialized.")
end

function on_connect(id)
    if State.players[id] then return end
    
    -- Ensure client loads assets
    Renderer.init()
    
    print("Player connected: " .. id)
    local p = Entities.new_player(id, "dragon") 
    State.players[id] = p
    
    -- Matchmaking
    if State.waiting_player then
        local p1 = State.waiting_player
        local p2 = p
        
        if p1.id == p2.id then return end 
        
        State.waiting_player = nil
        start_match(p1, p2)
    else
        State.waiting_player = p
        print("Player " .. id .. " waiting for opponent.")
    end
end

function start_match(p1, p2)
    local rid = State.room_counter
    State.room_counter = State.room_counter + 1
    
    p1.type = "dragon"
    p2.type = "tiger"
    p1.x = 100
    p2.x = 600
    p1.facing = 1
    p2.facing = -1
    p1.state = "IDLE"
    p2.state = "IDLE"
    p1.hp = Config.MAX_HP
    p2.hp = Config.MAX_HP
    
    local room = {
        id = rid,
        p1 = p1,
        p2 = p2,
        state = "playing"
    }
    
    p1.room = room
    p2.room = room
    State.rooms[rid] = room
    print("Room " .. rid .. " created.")
end

function on_disconnect(id)
    local p = State.players[id]
    if p then
        if State.waiting_player == p then
            State.waiting_player = nil
        end
        if p.room then
            -- Notify other player? For now just end room
            local room = p.room
            local other = (room.p1 == p) and room.p2 or room.p1
            if other then
                other.room = nil
                -- Put other back in queue?
                on_disconnect(other.id) -- Kick them for now or re-queue
                on_connect(other.id)
            end
            State.rooms[room.id] = nil
        end
        State.players[id] = nil
    end
end

local function start_attack(p, type)
    if p.stamina < Config.ATTACK_COST then 
        -- PENALTY: Take damage and reset stamina cooldown
        p.hp = max(1, p.hp - 5) -- Don't die from penalty
        p.stamina = 0
        p.state = "HIT"
        p.timer = 0.2
        return 
    end

    local char_cfg = Config.CHARS[p.type] or Config.CHARS.dragon
    local anim = char_cfg.anims[type]
    if not anim then return end
    
    p.stamina = 0 -- Drain full bar
    p.state = type
    p.timer = anim.frames / anim.speed
    p.vx = 0
    p.has_hit = false
    p.anim_time = 0
end

local function start_dodge(p, dir)
    print("Attempting Dodge. Stamina: " .. p.stamina .. " Cost: " .. Config.DODGE_COST)
    if p.stamina < Config.DODGE_COST then 
        print("Dodge failed: Low Stamina")
        return 
    end
    
    print("START DODGE " .. dir .. " Speed: " .. Config.DODGE_SPEED)
    p.stamina = p.stamina - Config.DODGE_COST
    p.state = "DODGE"
    p.timer = Config.DODGE_DURATION
    p.vx = dir * Config.DODGE_SPEED
    p.facing = dir
    p.anim_time = 0
end

function on_input(id, code, is_down)
    -- print("Input: " .. tostring(code) .. " Type: " .. type(code) .. " Down: " .. tostring(is_down))
    local p = State.players[id]
    if p then
        p.inputs[code] = is_down
    end
end

local function update_player(p, dt, opponent)
    if not p then return end
    
    -- Regen Stamina
    if p.state ~= "PUNCH" and p.state ~= "KICK" and p.state ~= "DODGE" then
        p.stamina = min(Config.MAX_STAMINA, p.stamina + Config.STAMINA_REGEN * dt)
    end

    p.anim_time = (p.anim_time or 0) + dt
    local prev_state = p.state
    
    -- State Timer
    if p.timer > 0 then
        p.timer = p.timer - dt
        if p.timer <= 0 then
            p.state = "IDLE" -- Reset to idle after animation
        end
    end
    
    -- HITBOX LOGIC (Frame Perfect)
    if (p.state == "PUNCH" or p.state == "KICK") and not p.has_hit then
        local char_cfg = Config.CHARS[p.type] or Config.CHARS.dragon
        local anim = char_cfg.anims[p.state]
        if anim then
            local frame = floor(p.anim_time * anim.speed)
            local hit_frame = anim.hit_frame or 3
            if frame >= hit_frame then
                p.has_hit = true
                local range = anim.range or 100
                local dmg = anim.damage or 10
                check_hit(p, opponent, dmg, range)
            end
        end
    end
    
    -- Actions (Only if IDLE or MOVING)
    if p.state == "IDLE" or p.state == "WALK" or p.state == "JUMP" then
        -- Movement
        local dx = 0
        if p.inputs[37] then dx = -1 end -- Left
        if p.inputs[39] then dx = 1 end  -- Right
        
        p.vx = dx * Config.MOVE_SPEED
        
        if dx ~= 0 then 
            if p.state == "IDLE" then p.state = "WALK" end
            p.facing = dx -- Face movement direction
        else
             if p.state == "WALK" then p.state = "IDLE" end
        end
        
        -- Jump
        if p.inputs[38] and p.y >= Config.GROUND_Y - Config.PLAYER_SIZE.h - 1 then
            if p.stamina >= Config.JUMP_COST then
                p.vy = Config.JUMP_FORCE
                p.stamina = p.stamina - Config.JUMP_COST
                p.state = "JUMP"
            end
        end
        
        -- Attacks & Dodge
        -- Check Just Pressed
        local just_punch = p.inputs[90] and not p.prev_inputs[90]
        local just_kick = p.inputs[88] and not p.prev_inputs[88]

        if just_punch then
            start_attack(p, "PUNCH")
        elseif just_kick then
            print("X Just Pressed. Calling start_dodge.")
            -- X is now strictly DODGE (Dash)
            if p.inputs[37] then -- Left
                start_dodge(p, -1)
            elseif p.inputs[39] then -- Right
                start_dodge(p, 1)
            else
                -- No direction held: Dash in facing direction
                start_dodge(p, p.facing)
            end
        end
    end
    
    -- Update Prev Inputs
    for k,v in pairs(p.inputs) do p.prev_inputs[k] = v end
    
    if p.state ~= prev_state then p.anim_time = 0 end

    -- Physics
    p.vy = p.vy + Config.GRAVITY * dt
    
    -- Apply Friction (helps stop after dash/hit)
    -- Don't apply friction during DODGE (constant speed dash)
    if p.state ~= "WALK" and p.state ~= "DODGE" then
        p.vx = p.vx * (Config.FRICTION or 0.95)
    end
    
    p.x = p.x + p.vx * dt
    p.y = p.y + p.vy * dt
    
    -- Ground Collision
    if p.y > Config.GROUND_Y - Config.PLAYER_SIZE.h then
        p.y = Config.GROUND_Y - Config.PLAYER_SIZE.h
        p.vy = 0
        if p.state == "JUMP" then p.state = "IDLE" end
    end
    
    -- Screen Bounds
    p.x = max(0, min(Config.SCREEN_W - Config.PLAYER_SIZE.w, p.x))
end

function check_hit(attacker, defender, damage, range)
    -- Simple AABB or Distance check
    if defender.state == "HIT" or defender.state == "DEAD" or defender.state == "DODGE" then return end
    
    -- Direction Check (Must be facing opponent)
    local diff = defender.x - attacker.x
    if (diff * attacker.facing) < 0 then return end
    
    local dx = abs(diff)
    local dy = abs(attacker.y - defender.y)
    
    if dx < range and dy < 50 then
        -- HIT!
        defender.hp = defender.hp - damage
        defender.state = "HIT"
        defender.timer = 0.3
        
        -- Knockback
        defender.vx = attacker.facing * 200
        defender.vy = -200
        
        if defender.hp <= 0 then
            defender.state = "DEAD"
            -- Reset round?
        end
    end
end

function update(dt)
    for _, room in pairs(State.rooms) do
        if room.state == "playing" then
            if room.p1 and room.p2 then
                update_player(room.p1, dt, room.p2)
                update_player(room.p2, dt, room.p1)
                
                if room.p1.hp <= 0 or room.p2.hp <= 0 then
                    -- Reset
                    room.p1.hp = Config.MAX_HP
                    room.p2.hp = Config.MAX_HP
                    room.p1.stamina = Config.MAX_STAMINA
                    room.p2.stamina = Config.MAX_STAMINA
                    room.p1.x = 100
                    room.p2.x = 600
                    room.p1.state = "IDLE"
                    room.p2.state = "IDLE"
                end
            end
        end
    end
end

function draw(id)
    Renderer.draw(id)
end