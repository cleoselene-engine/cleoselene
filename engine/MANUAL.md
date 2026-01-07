# Cleoselene Manual

A Multiplayer-First Server-Rendered Game Engine with Lua Scripting.

## Game Structure

A minimal game script (`main.lua`) must implement these callbacks:

```lua
-- Called once when the server starts
function init()
    -- Initialize physics, load assets, setup state
    db = api.new_spatial_db(250)
    phys = api.new_physics_world(db)
    api.load_sound("jump", "assets/jump.wav")
end

-- Called every server frame (typically 30 TPS)
function update(dt)
    -- Advance physics simulation
    phys:step(dt)
    
    -- Handle collisions
    local events = phys:get_collision_events()
    for _, pair in ipairs(events) do
        -- Handle game logic (damage, score)
    end
end

-- Called for EACH connected client to generate their frame
function draw(session_id)
    api.clear_screen(20, 20, 30)
    
    -- Draw player-specific view (camera, HUD)
    local p = players[session_id]
    if p then
        api.set_color(255, 255, 255)
        api.draw_text("HP: " .. p.hp, 10, 10)
        -- Use db:query_rect to draw visible entities
    end
end

-- Network Events
function on_connect(session_id)
    print("Player joined: " .. session_id)
    -- Spawn player entity
end

function on_disconnect(session_id)
    print("Player left: " .. session_id)
    -- Despawn entity
end

function on_input(session_id, key_code, is_down)
    -- Handle input (key_code is JS key code)
    -- 37=Left, 38=Up, 39=Right, 40=Down, 32=Space, 90=Z
    if players[session_id] then
        players[session_id].inputs[key_code] = is_down
    end
end
```

## API Reference

### Display & Coordinates

The engine uses a fixed virtual coordinate system of **800x600**. All drawing commands (`api.fill_rect`, `api.draw_line`, etc.) use these coordinates. The engine automatically scales the output to fit the user's screen while maintaining the logical resolution and aspect ratio.

### Graphics & Sound

| Method | Description |
| :--- | :--- |
| `api.clear_screen(r, g, b)` | Clears the frame with a background color. |
| `api.set_color(r, g, b, [a])` | Sets the current drawing color. |
| `api.fill_rect(x, y, w, h)` | Draws a filled rectangle. |
| `api.draw_line(x1, y1, x2, y2, [width])` | Draws a line. |
| `api.draw_text(text, x, y)` | Draws text at position. |
| `api.load_sound(name, url)` | Preloads a sound from a URL/path (relative to script). |
| `api.play_sound(name, [loop])` | Plays a loaded sound. |
| `api.stop_sound(name)` | Stops a sound. |
| `api.set_volume(name, volume)` | Sets volume (0.0 to 1.0). |

### Spatial DB (Geometry)

The engine provides a high-performance Spatial Hash Grid for broadphase queries.

#### Creation
```lua
local db = api.new_spatial_db(cell_size) -- e.g., 250
```

#### Object Management
| Method | Description | Returns |
| :--- | :--- | :--- |
| `db:add_circle(x, y, radius, tag)` | Registers a circular entity. | `id` (int) |
| `db:add_segment(x1, y1, x2, y2, tag)` | Registers a line segment (wall). | `id` (int) |
| `db:remove(id)` | Removes an entity from the DB. | `nil` |
| `db:update(id, x, y)` | Manually updates position (teleport). | `nil` |
| `db:get_position(id)` | Returns `x, y` of the entity. | `x, y` |

#### Queries (Sensors)
| Method | Description | Returns |
| :--- | :--- | :--- |
| `db:query_range(x, y, r, [tag])` | Finds entity IDs within radius `r`. | `{id1, id2...}` |
| `db:query_rect(x1, y1, x2, y2, [tag])` | Finds entity IDs within AABB (Culling). | `{id1, id2...}` |
| `db:cast_ray(x, y, angle, dist, [tag])` | Casts a ray. | `id, frac, hit_x, hit_y` or `nil` |

### Physics Engine (Simulation)

Handles rigid body dynamics, integration, and collision resolution.

#### Creation
```lua
local phys = api.new_physics_world(db)
```

#### Body Management
| Method | Description |
| :--- | :--- |
| `phys:add_body(id, props)` | Adds physics to an entity. Props: `{mass=1.0, restitution=0.5, drag=0.0}`. |
| `phys:set_velocity(id, vx, vy)` | Sets velocity. |
| `phys:get_velocity(id)` | Returns `vx, vy`. |
| `phys:set_gravity(x, y)` | Sets global gravity vector. |
| `phys:step(dt)` | Advances simulation. Resolves collisions and updates `db`. |
| `phys:get_collision_events()` | Returns list of collisions since last step: `{{idA, idB}, ...}`. |

### Graph Navigation (Pathfinding)

Native A* implementation on a custom graph.

| Method | Description |
| :--- | :--- |
| `nav = api.new_graph()` | Creates a new navigation graph. |
| `nav:add_node(id, x, y)` | Adds a node to the graph. |
| `nav:add_edge(u, v)` | Adds an edge (connection) between nodes. |
| `nav:find_path(start, end)` | Returns a list of node IDs forming the shortest path. |

### HTTP & JSON

Make HTTP requests and handle JSON data.

| Method | Description |
| :--- | :--- |
| `api.http_get(url)` | Makes a GET request. Returns `(body, error)`. |
| `api.http_post(url, body)` | Makes a POST request with JSON body. Returns `(body, error)`. |
| `api.json_decode(str)` | Parses JSON string to Lua table. Returns `(table, error)`. |
| `api.json_encode(table)` | Encodes Lua table to JSON string. Returns `(string, error)`. |

#### Example: LLM API Call
```lua
local response, err = api.http_post("http://localhost:8888/v1/chat/completions", {
    model = "gpt-3.5-turbo",
    messages = {
        { role = "user", content = "Hello!" }
    },
    max_tokens = 100
})

if err then
    print("Error: " .. err)
else
    local data, parse_err = api.json_decode(response)
    if data and data.choices and data.choices[1] then
        print("Response: " .. data.choices[1].message.content)
    end
end
```