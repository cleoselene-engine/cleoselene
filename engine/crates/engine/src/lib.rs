use bytes::{BufMut, Bytes, BytesMut};
use mlua::{Lua, Function, LuaSerdeExt, StdLib, LuaOptions, UserData, AnyUserData};
use std::sync::{Arc, Mutex};
use serde_json::Value;

mod spatial_db;
use spatial_db::SpatialDb;
mod physics;
use physics::PhysicsWorld;
mod graph_nav;
use graph_nav::Graph;

// OpCodes
const OP_CLEAR: u8 = 0x01;
const OP_SET_COLOR: u8 = 0x02;
const OP_FILL_RECT: u8 = 0x03;
const OP_DRAW_LINE: u8 = 0x04;
const OP_DRAW_TEXT: u8 = 0x05;
const OP_LOAD_SOUND: u8 = 0x06;
const OP_PLAY_SOUND: u8 = 0x07;
const OP_STOP_SOUND: u8 = 0x08;
const OP_SET_VOLUME: u8 = 0x09;
const OP_LOAD_IMAGE: u8 = 0x0A;
const OP_DRAW_IMAGE: u8 = 0x0B;

#[derive(Clone, Copy, PartialEq, Debug)]
enum GameMode {
    Update,
    Draw,
}

// Wrapper for SpatialDb to be exposed as UserData
#[derive(Clone)]
struct SpatialDbWrapper(Arc<Mutex<SpatialDb>>);

impl UserData for SpatialDbWrapper {
    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("add_circle", |_, this, (x, y, r, tag): (f32, f32, f32, String)| {
            let mut db = this.0.lock().unwrap();
            Ok(db.add_circle(x, y, r, &tag))
        });

        methods.add_method("add_segment", |_, this, (x1, y1, x2, y2, tag): (f32, f32, f32, f32, String)| {
            let mut db = this.0.lock().unwrap();
            Ok(db.add_segment(x1, y1, x2, y2, &tag))
        });

        methods.add_method("update", |_, this, (id, x, y): (u64, f32, f32)| {
            let mut db = this.0.lock().unwrap();
            db.update_position(id, x, y);
            Ok(())
        });

        methods.add_method("get_position", |_, this, id: u64| {
            let db = this.0.lock().unwrap();
            let pos = db.get_position(id);
            match pos {
                Some((x, y)) => Ok((Some(x), Some(y))),
                None => Ok((None, None))
            }
        });

        methods.add_method("remove", |_, this, id: u64| {
            let mut db = this.0.lock().unwrap();
            db.remove(id);
            Ok(())
        });

        methods.add_method("query_range", |_, this, (x, y, r, tag_filter): (f32, f32, f32, Option<String>)| {
            let db = this.0.lock().unwrap();
            let ids = db.query_range(x, y, r, tag_filter.as_deref());
            Ok(ids)
        });

        methods.add_method("query_rect", |_, this, (min_x, min_y, max_x, max_y, tag_filter): (f32, f32, f32, f32, Option<String>)| {
            let db = this.0.lock().unwrap();
            let ids = db.query_rect(min_x, min_y, max_x, max_y, tag_filter.as_deref());
            Ok(ids)
        });

        methods.add_method("cast_ray", |_, this, (x, y, angle, dist, tag_filter): (f32, f32, f32, f32, Option<String>)| {
            let db = this.0.lock().unwrap();
            let res = db.cast_ray(x, y, angle, dist, tag_filter.as_deref());
            match res {
                Some((id, dist_fac, hx, hy)) => Ok((Some(id), Some(dist_fac), Some(hx), Some(hy))),
                None => Ok((None, None, None, None))
            }
        });
    }
}

// Wrapper for PhysicsWorld
#[derive(Clone)]
struct PhysicsWrapper(Arc<Mutex<PhysicsWorld>>);

impl UserData for PhysicsWrapper {
    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("add_body", |_, this, (id, props): (u64, mlua::Table)| {
            let mass: f32 = props.get("mass").unwrap_or(1.0);
            let restitution: f32 = props.get("restitution").unwrap_or(0.5);
            let drag: f32 = props.get("drag").unwrap_or(0.0);
            
            let mut phys = this.0.lock().unwrap();
            phys.add_body(id, mass, restitution, drag);
            Ok(())
        });

        methods.add_method("set_gravity", |_, this, (x, y): (f32, f32)| {
            let mut phys = this.0.lock().unwrap();
            phys.set_gravity(x, y);
            Ok(())
        });

        methods.add_method("set_velocity", |_, this, (id, vx, vy): (u64, f32, f32)| {
            let mut phys = this.0.lock().unwrap();
            phys.set_velocity(id, vx, vy);
            Ok(())
        });
        
        methods.add_method("get_velocity", |_, this, id: u64| {
            let phys = this.0.lock().unwrap();
            let v = phys.get_velocity(id);
            match v {
                Some((vx, vy)) => Ok((Some(vx), Some(vy))),
                None => Ok((None, None))
            }
        });

        methods.add_method("step", |_, this, dt: f32| {
            let mut phys = this.0.lock().unwrap();
            phys.step(dt);
            Ok(())
        });

        methods.add_method("get_collision_events", |_, this, ()| {
            let mut phys = this.0.lock().unwrap();
            let events = phys.get_collision_events();
            let lua_events: Vec<Vec<u64>> = events.into_iter().map(|(a, b)| vec![a, b]).collect();
            Ok(lua_events)
        });
    }
}

// Wrapper for Graph
#[derive(Clone)]
struct GraphWrapper(Arc<Mutex<Graph>>);

impl UserData for GraphWrapper {
    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("add_node", |_, this, (id, x, y): (u64, f32, f32)| {
            let mut g = this.0.lock().unwrap();
            g.add_node(id, x, y);
            Ok(())
        });

        methods.add_method("add_edge", |_, this, (u, v): (u64, u64)| {
            let mut g = this.0.lock().unwrap();
            g.add_edge(u, v);
            Ok(())
        });

        methods.add_method("find_path", |_, this, (start, goal): (u64, u64)| {
            let g = this.0.lock().unwrap();
            let path = g.find_path(start, goal);
            Ok(path)
        });
    }
}

#[derive(Clone)]
pub struct CommandBuffer {
    data: Arc<Mutex<BytesMut>>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(BytesMut::with_capacity(1024))),
        }
    }

    pub fn clear(&self) {
        let mut data = self.data.lock().unwrap();
        data.clear();
    }

    pub fn get_bytes(&self) -> Bytes {
        let data = self.data.lock().unwrap();
        Bytes::copy_from_slice(&data)
    }

    // --- Primitive Writers ---

    fn cmd_clear_screen(&self, r: u8, g: u8, b: u8) {
        let mut data = self.data.lock().unwrap();
        data.put_u8(OP_CLEAR);
        data.put_u8(r);
        data.put_u8(g);
        data.put_u8(b);
    }

    fn cmd_set_color(&self, r: u8, g: u8, b: u8, a: u8) {
        let mut data = self.data.lock().unwrap();
        data.put_u8(OP_SET_COLOR);
        data.put_u8(r);
        data.put_u8(g);
        data.put_u8(b);
        data.put_u8(a);
    }

    fn cmd_fill_rect(&self, x: f32, y: f32, w: f32, h: f32) {
        let mut data = self.data.lock().unwrap();
        data.put_u8(OP_FILL_RECT);
        data.put_f32_le(x);
        data.put_f32_le(y);
        data.put_f32_le(w);
        data.put_f32_le(h);
    }

    fn cmd_draw_line(&self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32) {
        let mut data = self.data.lock().unwrap();
        data.put_u8(OP_DRAW_LINE);
        data.put_f32_le(x1);
        data.put_f32_le(y1);
        data.put_f32_le(x2);
        data.put_f32_le(y2);
        data.put_f32_le(width);
    }

    fn cmd_draw_text(&self, text: &str, x: f32, y: f32) {
        let mut data = self.data.lock().unwrap();
        data.put_u8(OP_DRAW_TEXT);
        data.put_f32_le(x);
        data.put_f32_le(y);
        let bytes = text.as_bytes();
        data.put_u16_le(bytes.len() as u16);
        data.put_slice(bytes);
    }

    fn cmd_load_sound(&self, name: &str, url: &str) {
        let mut data = self.data.lock().unwrap();
        data.put_u8(OP_LOAD_SOUND);
        
        let name_bytes = name.as_bytes();
        data.put_u16_le(name_bytes.len() as u16);
        data.put_slice(name_bytes);

        let url_bytes = url.as_bytes();
        data.put_u16_le(url_bytes.len() as u16);
        data.put_slice(url_bytes);
    }

    fn cmd_play_sound(&self, name: &str, loop_sound: bool, volume: f32) {
        let mut data = self.data.lock().unwrap();
        data.put_u8(OP_PLAY_SOUND);
        
        let name_bytes = name.as_bytes();
        data.put_u16_le(name_bytes.len() as u16);
        data.put_slice(name_bytes);
        
        data.put_u8(if loop_sound { 1 } else { 0 });
        data.put_f32_le(volume);
    }

    fn cmd_stop_sound(&self, name: &str) {
        let mut data = self.data.lock().unwrap();
        data.put_u8(OP_STOP_SOUND);
        let name_bytes = name.as_bytes();
        data.put_u16_le(name_bytes.len() as u16);
        data.put_slice(name_bytes);
    }

    fn cmd_set_volume(&self, name: &str, volume: f32) {
        let mut data = self.data.lock().unwrap();
        data.put_u8(OP_SET_VOLUME);
        let name_bytes = name.as_bytes();
        data.put_u16_le(name_bytes.len() as u16);
        data.put_slice(name_bytes);
        data.put_f32_le(volume);
    }

    fn cmd_load_image(&self, name: &str, url: &str) {
        let mut data = self.data.lock().unwrap();
        data.put_u8(OP_LOAD_IMAGE);
        
        let name_bytes = name.as_bytes();
        data.put_u16_le(name_bytes.len() as u16);
        data.put_slice(name_bytes);

        let url_bytes = url.as_bytes();
        data.put_u16_le(url_bytes.len() as u16);
        data.put_slice(url_bytes);
    }

    fn cmd_draw_image(&self, name: &str, x: f32, y: f32, w: Option<f32>, h: Option<f32>, sx: Option<f32>, sy: Option<f32>, sw: Option<f32>, sh: Option<f32>, r: Option<f32>, ox: Option<f32>, oy: Option<f32>) {
        let mut data = self.data.lock().unwrap();
        data.put_u8(OP_DRAW_IMAGE);
        
        let name_bytes = name.as_bytes();
        data.put_u16_le(name_bytes.len() as u16);
        data.put_slice(name_bytes);
        
        // Dest Rect
        data.put_f32_le(x);
        data.put_f32_le(y);
        data.put_f32_le(w.unwrap_or(-1.0)); // -1.0 means use original size
        data.put_f32_le(h.unwrap_or(-1.0));

        // Source Rect (Sprite Sheet support)
        data.put_f32_le(sx.unwrap_or(0.0));
        data.put_f32_le(sy.unwrap_or(0.0));
        data.put_f32_le(sw.unwrap_or(-1.0)); // -1.0 means full width
        data.put_f32_le(sh.unwrap_or(-1.0));
        
        // Rotation & Origin (Optional)
        data.put_f32_le(r.unwrap_or(0.0));
        data.put_f32_le(ox.unwrap_or(0.0)); // Origin X relative to Dest X (0.0 = top-left, 0.5 = center of w)
        data.put_f32_le(oy.unwrap_or(0.0));
    }

    pub fn append(&self, other: &CommandBuffer) {
        let mut data = self.data.lock().unwrap();
        let other_data = other.data.lock().unwrap();
        data.extend_from_slice(&other_data);
    }
}

pub struct GameState {
    lua: Lua,
    command_buffer: CommandBuffer,
    event_buffer: CommandBuffer,
    current_mode: Arc<Mutex<GameMode>>,
}

impl GameState {
    pub fn new(script_content: &str, script_path: Option<&std::path::Path>) -> anyhow::Result<Self> {
        // SANDBOX SECURITY:
        // 1. Only load safe standard libraries. NO IO, NO OS, NO DEBUG.
        let libs = StdLib::MATH | StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::COROUTINE | StdLib::PACKAGE;
        let lua = Lua::new_with(libs, LuaOptions::default())?;
        
        // 2. Set Memory Limit (128 MB) to prevent RAM exhaustion DoS
        lua.set_memory_limit(128 * 1024 * 1024)?;

        // 3. Configure package.path to allow requiring local modules
        // We set it to "./?.lua" so scripts can require files relative to the working directory (which is set to the game dir by the server)
        {
            let globals = lua.globals();
            let package: mlua::Table = globals.get("package")?;
            
            let mut path_str = "./?.lua".to_string();
            if let Some(p) = script_path {
                if let Some(parent) = p.parent() {
                    if let Some(parent_str) = parent.to_str() {
                        path_str.push_str(";");
                        path_str.push_str(parent_str);
                        path_str.push_str("/?.lua");
                    }
                }
            }
            package.set("path", path_str)?;
        }

        let command_buffer = CommandBuffer::new();
        let event_buffer = CommandBuffer::new();
        let current_mode = Arc::new(Mutex::new(GameMode::Update));
        
        // Expose API to Lua
        {
            let globals = lua.globals();
            let api = lua.create_table()?;
            
            let buf_clone = command_buffer.clone();
            api.set("clear_screen", lua.create_function(move |_, (r, g, b): (u8, u8, u8)| {
                buf_clone.cmd_clear_screen(r, g, b);
                Ok(())
            })?)?;

            let buf_clone = command_buffer.clone();
            api.set("set_color", lua.create_function(move |_, (r, g, b, a): (u8, u8, u8, Option<u8>)| {
                buf_clone.cmd_set_color(r, g, b, a.unwrap_or(255));
                Ok(())
            })?)?;

            let buf_clone = command_buffer.clone();
            api.set("fill_rect", lua.create_function(move |_, (x, y, w, h): (f32, f32, f32, f32)| {
                buf_clone.cmd_fill_rect(x, y, w, h);
                Ok(())
            })?)?;

            let buf_clone = command_buffer.clone();
            api.set("draw_line", lua.create_function(move |_, (x1, y1, x2, y2, w): (f32, f32, f32, f32, Option<f32>)| {
                buf_clone.cmd_draw_line(x1, y1, x2, y2, w.unwrap_or(1.0));
                Ok(())
            })?)?;

            let buf_clone = command_buffer.clone();
            api.set("draw_text", lua.create_function(move |_, (text, x, y): (String, f32, f32)| {
                buf_clone.cmd_draw_text(&text, x, y);
                Ok(())
            })?)?;

            let buf_clone = command_buffer.clone();
            api.set("load_sound", lua.create_function(move |_, (name, url): (String, String)| {
                buf_clone.cmd_load_sound(&name, &url);
                Ok(())
            })?)?;

            // Context-Aware Play Sound
            let event_buf = event_buffer.clone();
            let cmd_buf = command_buffer.clone();
            let mode_ref = current_mode.clone();
            
            api.set("play_sound", lua.create_function(move |_, (name, loop_val, volume): (String, Option<bool>, Option<f32>)| {
                let mode = *mode_ref.lock().unwrap();
                let vol = volume.unwrap_or(1.0);
                let lp = loop_val.unwrap_or(false);
                
                match mode {
                    GameMode::Update => event_buf.cmd_play_sound(&name, lp, vol),
                    GameMode::Draw => cmd_buf.cmd_play_sound(&name, lp, vol),
                }
                Ok(())
            })?)?;

            let event_buf = event_buffer.clone();
            let cmd_buf = command_buffer.clone();
            let mode_ref = current_mode.clone();
            api.set("stop_sound", lua.create_function(move |_, name: String| {
                let mode = *mode_ref.lock().unwrap();
                match mode {
                    GameMode::Update => event_buf.cmd_stop_sound(&name),
                    GameMode::Draw => cmd_buf.cmd_stop_sound(&name),
                }
                Ok(())
            })?)?;

            let event_buf = event_buffer.clone();
            let cmd_buf = command_buffer.clone();
            let mode_ref = current_mode.clone();
            api.set("set_volume", lua.create_function(move |_, (name, vol): (String, f32)| {
                let mode = *mode_ref.lock().unwrap();
                match mode {
                    GameMode::Update => event_buf.cmd_set_volume(&name, vol),
                    GameMode::Draw => cmd_buf.cmd_set_volume(&name, vol),
                }
                Ok(())
            })?)?;

            let buf_clone = command_buffer.clone();
            api.set("load_image", lua.create_function(move |_, (name, url): (String, String)| {
                buf_clone.cmd_load_image(&name, &url);
                Ok(())
            })?)?;

            let buf_clone = command_buffer.clone();
            // api.draw_image(name, x, y, [w, h, sx, sy, sw, sh, r, ox, oy])
            api.set("draw_image", lua.create_function(move |_, (name, x, y, w, h, sx, sy, sw, sh, r, ox, oy): (String, f32, f32, Option<f32>, Option<f32>, Option<f32>, Option<f32>, Option<f32>, Option<f32>, Option<f32>, Option<f32>, Option<f32>)| {
                buf_clone.cmd_draw_image(&name, x, y, w, h, sx, sy, sw, sh, r, ox, oy);
                Ok(())
            })?)?;

            api.set("new_spatial_db", lua.create_function(move |_, cell_size: f32| {
                let db = SpatialDb::new(cell_size);
                Ok(SpatialDbWrapper(Arc::new(Mutex::new(db))))
            })?)?;

            api.set("new_physics_world", lua.create_function(move |_, userdata: AnyUserData| {
                let db_wrapper = userdata.borrow::<SpatialDbWrapper>()?;
                let phys = PhysicsWorld::new(db_wrapper.0.clone());
                Ok(PhysicsWrapper(Arc::new(Mutex::new(phys))))
            })?)?;

            api.set("new_graph", lua.create_function(move |_, ()| {
                let graph = Graph::new();
                Ok(GraphWrapper(Arc::new(Mutex::new(graph))))
            })?)?;

            globals.set("api", api)?;

            // Load the game script
            lua.load(script_content).exec()?;

            // Call init if exists
            if let Ok(init) = globals.get::<_, Function>("init") {
                init.call::<_, ()>(())?;
            }
        }

        Ok(Self {
            lua,
            command_buffer,
            event_buffer,
            current_mode,
        })
    }

    pub fn begin_frame(&self) {
        self.event_buffer.clear();
    }

    pub fn update(&self, dt: f32) -> anyhow::Result<()> {
        *self.current_mode.lock().unwrap() = GameMode::Update;
        let globals = self.lua.globals();
        if let Ok(update) = globals.get::<_, Function>("update") {
            update.call::<_, ()>(dt)?;
        }
        Ok(())
    }

    // Now accepts session_id so Lua knows WHO to draw for
    pub fn draw(&self, session_id: &str) -> anyhow::Result<Bytes> {
        *self.current_mode.lock().unwrap() = GameMode::Draw;
        
        // Clear previous buffer
        self.command_buffer.clear();
        
        // Include events from update (sounds)
        self.command_buffer.append(&self.event_buffer);

        let globals = self.lua.globals();
        if let Ok(draw) = globals.get::<_, Function>("draw") {
            draw.call::<_, ()>(session_id)?;
        }
        
        Ok(self.command_buffer.get_bytes())
    }
    
    pub fn handle_input(&self, session_id: &str, input_code: u8, active: bool) -> anyhow::Result<()> {
         let globals = self.lua.globals();
         if let Ok(on_input) = globals.get::<_, Function>("on_input") {
             on_input.call::<_, ()>((session_id, input_code, active))?;
         }
         Ok(())
    }

    pub fn on_connect(&self, session_id: &str) -> anyhow::Result<Bytes> {
        self.command_buffer.clear();
        let globals = self.lua.globals();
        if let Ok(cb) = globals.get::<_, Function>("on_connect") {
            cb.call::<_, ()>(session_id)?;
        }
        Ok(self.command_buffer.get_bytes())
    }

    pub fn on_disconnect(&self, session_id: &str) -> anyhow::Result<()> {
        let globals = self.lua.globals();
        if let Ok(cb) = globals.get::<_, Function>("on_disconnect") {
            cb.call::<_, ()>(session_id)?;
        }
        Ok(())
    }

    // --- State Persistence for Hot Reload ---

    pub fn snapshot_state(&self) -> anyhow::Result<String> {
        let globals = self.lua.globals();
        
        // Get as generic Lua Value first
        let players_lua: mlua::Value = globals.get("players")?;
        let asteroids_lua: mlua::Value = globals.get("asteroids")?;
        let bullets_lua: mlua::Value = globals.get("bullets")?;

        // Convert to Serde Value
        let players: Value = self.lua.from_value(players_lua)?;
        let asteroids: Value = self.lua.from_value(asteroids_lua)?;
        let bullets: Value = self.lua.from_value(bullets_lua)?;

        let state = serde_json::json!({
            "players": players,
            "asteroids": asteroids,
            "bullets": bullets
        });

        Ok(state.to_string())
    }

    pub fn restore_state(&self, json_state: &str) -> anyhow::Result<()> {
        let globals = self.lua.globals();
        let state: Value = serde_json::from_str(json_state)?;

        if let Some(obj) = state.as_object() {
            if let Some(p) = obj.get("players") {
                let lua_val = self.lua.to_value(p)?;
                globals.set("players", lua_val)?;
            }
            if let Some(a) = obj.get("asteroids") {
                let lua_val = self.lua.to_value(a)?;
                globals.set("asteroids", lua_val)?;
            }
            if let Some(b) = obj.get("bullets") {
                let lua_val = self.lua.to_value(b)?;
                globals.set("bullets", lua_val)?;
            }
        }
        Ok(())
    }

    pub fn eval(&self, code: &str) -> String {
        match self.lua.load(code).eval::<mlua::Value>() {
            Ok(v) => format!("{:?}", v),
            Err(e) => format!("Error: {}", e),
        }
    }
}