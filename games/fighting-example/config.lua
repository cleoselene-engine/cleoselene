local M = {}

M.SCREEN_W = 800
M.SCREEN_H = 600
M.GROUND_Y = 570
M.GRAVITY = 1500
M.JUMP_FORCE = -650
M.MOVE_SPEED = 300
M.MAX_HP = 100
M.MAX_STAMINA = 100
M.STAMINA_REGEN = 35 -- Recharges full in ~3s
M.ATTACK_COST = 10 -- Low cost to allow combos
M.DODGE_COST = 30 -- Moderate cost, allows ~3 dashes
M.JUMP_COST = 15 -- Low cost for mobility
M.DODGE_SPEED = 800
M.DODGE_DURATION = 0.18
M.FRICTION = 0.85

-- Hitbox (Physics)
M.PLAYER_SIZE = {w=50, h=100}

-- Character Configs
M.CHARS = {
    dragon = {
        native_facing = 1, -- Faces Right by default
        src_size = 180,
        draw_size = 250,
        anims = {
            IDLE = { frames = 11, loop = true, speed = 10 },
            WALK = { frames = 8, loop = true, speed = 12 },
            DODGE = { frames = 8, loop = true, speed = 25 }, -- Reuse Walk, fast
            PUNCH = { frames = 7, loop = false, speed = 15, hit_frame = 4, range = 160, damage = 10 },
            KICK = { frames = 7, loop = false, speed = 15, hit_frame = 4, range = 140, damage = 12 }, 
            HIT = { frames = 4, loop = false, speed = 10 }, 
            DEAD = { frames = 6, loop = false, speed = 8 },
            JUMP = { frames = 2, loop = false, speed = 0 }
        }
    },
    tiger = {
        native_facing = -1, -- Faces Left by default (fix for inverted flip)
        src_size = 162,
        draw_size = 225, 
        anims = {
            IDLE = { frames = 10, loop = true, speed = 10 },
            WALK = { frames = 8, loop = true, speed = 12 },
            DODGE = { frames = 8, loop = true, speed = 25 },
            PUNCH = { frames = 8, loop = false, speed = 15, hit_frame = 5, range = 150, damage = 10 },
            KICK = { frames = 8, loop = false, speed = 15, hit_frame = 5, range = 130, damage = 12 },
            HIT = { frames = 3, loop = false, speed = 10 }, 
            DEAD = { frames = 6, loop = false, speed = 8 },
            JUMP = { frames = 2, loop = false, speed = 0 }
        }
    }
}

return M