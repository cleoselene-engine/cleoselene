local M = {}

M.GRID_W = 60
M.GRID_H = 60
M.BASE_SIZE = 500
M.JITTER = 0.7
M.GRID_STRIDE = 10000

M.SCREEN_W = M.GRID_W * M.BASE_SIZE
M.SCREEN_H = M.GRID_H * M.BASE_SIZE
M.VIEW_W = 800
M.VIEW_H = 600

M.COLORS = {
    {255, 50, 50},   -- Red
    {50, 255, 50},   -- Green
    {80, 150, 255},  -- Blue
    {255, 200, 50}   -- Yellow
}

M.ITEMS = {
    laser = {r=0, g=255, b=255, symbol="dot"},
    dash = {r=255, g=255, b=0, symbol="arrow"},
    bomb = {r=255, g=50, b=0, symbol="circle"},
    minion = {r=255, g=0, b=255, symbol="M"},
    health = {r=50, g=255, b=50, symbol="plus"},
    energy = {r=80, g=100, b=255, symbol="bolt"}
}

M.ITEM_KEYS = {"laser", "dash", "bomb", "minion", "health", "energy"}

return M
