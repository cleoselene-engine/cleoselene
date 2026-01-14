# Cleoselene Manual

A Multiplayer-First Server-Rendered Game Engine with Lua Scripting.

## CLI Usage

| Flag | Description |
| :--- | :--- |
| `-h, --help` | Print this help manual and exit. |
| `-V, --version` | Print engine version and exit. |
| `--port <PORT>` | Port to start the server on (default: 3425). |
| `--debug` | Enable the remote debug endpoint at `/debug`. |
| `--test` | Run the script in headless test mode (init + 1 update) and exit. |

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
end

-- Called for EACH connected client to generate their frame
function draw(session_id)
    api.clear_screen(20, 20, 30)
    api.set_color(255, 255, 255)
    api.draw_text("Session: " .. session_id, 10, 10)
end

-- Network Events
function on_connect(session_id) end
function on_disconnect(session_id) end
function on_input(session_id, key_code, is_down) end
```

## API Reference

### Display & Coordinates

The engine uses a fixed virtual coordinate system of **800x600**. The output is automatically scaled to fit the client screen.

### Graphics & Sound

| Method | Description |
| :--- | :--- |
| `api.clear_screen(r, g, b)` | Clears the frame with a background color. |
| `api.set_color(r, g, b, [a])` | Sets the current drawing color. |
| `api.fill_rect(x, y, w, h)` | Draws a filled rectangle. |
| `api.draw_line(x1, y1, x2, y2, [width])` | Draws a line. |
| `api.draw_text(text, x, y)` | Draws text at position. |
| `api.load_image(name, url)` | Preloads an image/sprite from a URL or local path. |
| `api.draw_image(name, x, y, [w, h, sx, sy, sw, sh, r, ox, oy])` | Draws a (sub)image with optional scaling, rotation, and origin. |
| `api.load_sound(name, url)` | Preloads a sound from a URL/path. |
| `api.play_sound(name, [loop])` | Plays a loaded sound. |
| `api.stop_sound(name)` | Stops a sound. |
| `api.set_volume(name, volume)` | Sets volume (0.0 to 1.0). |

### Spatial DB (Geometry & Physics)

#### Creation
```lua
local db = api.new_spatial_db(cell_size)
local phys = api.new_physics_world(db)
```

#### Object Management
| Method | Description | Returns |
| :--- | :--- | :--- |
| `db:add_circle(x, y, radius, tag)` | Registers a circular entity. | `id` |
| `db:add_segment(x1, y1, x2, y2, tag)` | Registers a line segment. | `id` |
| `db:remove(id)` | Removes an entity. | `nil` |
| `db:update(id, x, y)` | Teleports an entity. | `nil` |
| `db:get_position(id)` | Returns current `x, y`. | `x, y` |

#### Physics integration
| Method | Description |
| :--- | :--- |
| `phys:add_body(id, props)` | Adds physics: `{mass, restitution, drag}`. |
| `phys:set_velocity(id, vx, vy)` | Sets body velocity. |
| `phys:get_velocity(id)` | Returns `vx, vy`. |
| `phys:set_gravity(x, y)` | Sets global gravity. |
| `phys:step(dt)` | Advances simulation and updates DB. |
| `phys:get_collision_events()` | Returns list of collisions: `{{idA, idB}, ...}`. |

#### Queries (Sensors)
| Method | Description |
| :--- | :--- |
| `db:query_range(x, y, r, [tag])` | Finds entity IDs within radius `r`. |
| `db:query_rect(x1, y1, x2, y2, [tag])` | Finds entity IDs within AABB. |
| `db:cast_ray(x, y, angle, dist, [tag])` | Returns `id, frac, hit_x, hit_y`. |

## Debugging

Start engine with `--debug`.

### Debug Endpoint (`/debug`)

Send POST requests with Lua code to `http://localhost:3425/debug`.

```bash
# Example: Inspect player state
curl -X POST -d "local State = require('state'); return State.players" http://localhost:3425/debug
```

## Testing

Start engine with `--test`.

```bash
cleoselene my_game.lua --test
```

Headless mode: runs `init()` and one `update(0.1)` cycle, then exits with code 0 (success) or 1 (error).
