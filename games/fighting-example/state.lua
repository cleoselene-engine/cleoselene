local M = {}

M.players = {}       -- map[id] -> player
M.rooms = {}         -- map[room_id] -> room_table
M.waiting_player = nil
M.room_counter = 1

return M