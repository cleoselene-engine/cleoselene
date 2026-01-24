local Config = require("config")
local State = require("state")
local Entities = require("entities")

local M = {}
local random = math.random
local insert = table.insert
local remove = table.remove

function M.generate()
    print("Generating Organic Mesh... Grid: " .. Config.GRID_W .. "x" .. Config.GRID_H .. " Scale: " .. Config.BASE_SIZE)
    
    -- Reset State
    State.walls = {}
    State.keys = {}
    State.asteroids = {}
    State.spawn_points = {}
    State.enemies = {}
    State.shots = {}
    State.items = {}
    State.nav_graph = {}
    State.nodes_list = {}
    State.grid_nodes = {}
    State.shards = {}
    State.bombs = {}
    State.particles = {}
    State.entity_map = {}
    
    -- Re-init Engine Systems
    State.db = api.new_spatial_db(Config.BASE_SIZE)
    State.phys = api.new_physics_world(State.db)
    State.nav = api.new_graph()
    
    local verts = {}
    for y = 0, Config.GRID_H do 
        verts[y] = {}
        for x = 0, Config.GRID_W do
            local jx, jy = (random()-0.5)*Config.BASE_SIZE*Config.JITTER, (random()-0.5)*Config.BASE_SIZE*Config.JITTER
            if x==0 or x==Config.GRID_W then jx=0 end
            if y==0 or y==Config.GRID_H then jy=0 end
            verts[y][x] = { x = x * Config.BASE_SIZE + jx, y = y * Config.BASE_SIZE + jy }
        end 
    end

    local edges = {}
    local function add_edge(u, v, p1, p2) 
        insert(edges, {n1=u, n2=v, x1=p1.x, y1=p1.y, x2=p2.x, y2=p2.y, type="wall"}) 
    end
    
    local idx = 1
    for y = 0, Config.GRID_H - 1 do for x = 0, Config.GRID_W - 1 do
        local vTL, vTR, vBL, vBR = verts[y][x], verts[y][x+1], verts[y+1][x], verts[y+1][x+1]
        local t1, t2 = idx, idx+1
        idx = idx + 2
        
        local cx1, cy1 = (vTL.x+vTR.x+vBL.x)/3, (vTL.y+vTR.y+vBL.y)/3
        local cx2, cy2 = (vTR.x+vBL.x+vBR.x)/3, (vTR.y+vBL.y+vBR.y)/3
        
        State.nodes_list[t1] = {id=t1, set=t1, x=cx1, y=cy1}
        State.nodes_list[t2] = {id=t2, set=t2, x=cx2, y=cy2}
        
        insert(State.spawn_points, State.nodes_list[t1])
        insert(State.spawn_points, State.nodes_list[t2])
        
        State.grid_nodes[x + y * Config.GRID_STRIDE] = {t1, t2}
        
        add_edge(t1, t2, vTR, vBL)
        if x>0 then add_edge(t1, t2-2, vTL, vBL) end
        if y>0 then add_edge(t1, t2-(Config.GRID_W*2), vTL, vTR) end
    end end
    
    -- Shuffle Edges
    for i=#edges, 2, -1 do local j=random(i); edges[i], edges[j] = edges[j], edges[i] end
    
    -- Kruskal's Algorithm (MST)
    local function find(i) 
        if State.nodes_list[i].set==i then return i end
        State.nodes_list[i].set = find(State.nodes_list[i].set)
        return State.nodes_list[i].set 
    end
    
    local mst = {}
    for _, e in ipairs(edges) do
        local r1, r2 = find(e.n1), find(e.n2)
        if r1 ~= r2 then
            State.nodes_list[r2].set = r1
            insert(mst, e)
            if not State.nav_graph[e.n1] then State.nav_graph[e.n1]={} end
            if not State.nav_graph[e.n2] then State.nav_graph[e.n2]={} end
            insert(State.nav_graph[e.n1], e.n2)
            insert(State.nav_graph[e.n2], e.n1)
        else
            insert(State.walls, e)
            Entities.register(e, "segment", e.x2, e.y2, "wall")
        end
    end
    
    -- Border Walls
    for x=0, Config.GRID_W-1 do
        local w1={x1=verts[0][x].x, y1=verts[0][x].y, x2=verts[0][x+1].x, y2=verts[0][x+1].y, type="wall"}
        insert(State.walls,w1); Entities.register(w1,"segment",w1.x2,w1.y2,"wall")
        
        local w2={x1=verts[Config.GRID_H][x].x, y1=verts[Config.GRID_H][x].y, x2=verts[Config.GRID_H][x+1].x, y2=verts[Config.GRID_H][x+1].y, type="wall"}
        insert(State.walls,w2); Entities.register(w2,"segment",w2.x2,w2.y2,"wall")
    end
    for y=0, Config.GRID_H-1 do
        local w1={x1=verts[y][0].x, y1=verts[y][0].y, x2=verts[y+1][0].x, y2=verts[y+1][0].y, type="wall"}
        insert(State.walls,w1); Entities.register(w1,"segment",w1.x2,w1.y2,"wall")
        
        local w2={x1=verts[y][Config.GRID_W].x, y1=verts[y][Config.GRID_W].y, x2=verts[y+1][Config.GRID_W].x, y2=verts[y+1][Config.GRID_W].y, type="wall"}
        insert(State.walls,w2); Entities.register(w2,"segment",w2.x2,w2.y2,"wall")
    end

    -- Doors & Keys
    for i=1, 40 do 
        if #mst > 0 then
            local e = remove(mst, random(#mst))
            local cid = random(#Config.COLORS)
            local d = {x1=e.x1, y1=e.y1, x2=e.x2, y2=e.y2, type="door", color_id=cid, open=false}
            Entities.register(d, "segment", d.x2, d.y2, "wall")
            insert(State.keys, {x=random(100, Config.SCREEN_W-100), y=random(100, Config.SCREEN_H-100), color_id=cid, taken=false})
            if State.nodes_list[e.n1] then 
                local itype = Config.ITEM_KEYS[random(#Config.ITEM_KEYS)]
                insert(State.items, {x=State.nodes_list[e.n1].x, y=State.nodes_list[e.n1].y, type=itype, taken=false, natural=true}) 
            end
        end 
    end
    
    -- Asteroids
    for i=1, 30 do 
        local a = {x=random(Config.SCREEN_W), y=random(Config.SCREEN_H), vx=(random()-0.5)*80, vy=(random()-0.5)*80, radius=15+random(20)}
        insert(State.asteroids, a)
        local id = Entities.register(a, "circle", a.radius, "asteroid")
        State.phys:add_body(id, {mass=1.0, restitution=0.8})
        State.phys:set_velocity(id, a.vx, a.vy)
    end
    
    -- Enemies
    for i, sp in ipairs(State.spawn_points) do 
        if i % 20 == 0 then 
            Entities.spawn_enemy_at(sp.x, sp.y)
        end 
    end

    -- Nav Graph to Engine
    for id, n in pairs(State.nodes_list) do State.nav:add_node(id, n.x, n.y) end
    for from, neighbors in pairs(State.nav_graph) do 
        for _, to in ipairs(neighbors) do State.nav:add_edge(from, to) end 
    end
    
    print("Map Gen Complete.")
end

return M
