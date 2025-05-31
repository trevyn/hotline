// Example demonstrating the serialization system for hot-reloading

use hotline::{dict, Message, Object, Value, Serialize, Deserialize};

// Example: A particle object with velocity
struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    color: String,
}

impl Object for Particle {
    fn receive(&mut self, msg: &Message) -> Value {
        match msg.selector.as_str() {
            "update" => {
                self.x += self.vx;
                self.y += self.vy;
                Value::Nil
            }
            "x" => Value::Float(self.x),
            "y" => Value::Float(self.y),
            _ => Value::Nil,
        }
    }
    
    fn serialize(&self) -> Value {
        dict! {
            "x" => self.x.serialize(),
            "y" => self.y.serialize(),
            "vx" => self.vx.serialize(),
            "vy" => self.vy.serialize(),
            "color" => self.color.serialize()
        }
    }
    
    fn deserialize(&mut self, state: &Value) {
        if let Value::Dict(props) = state {
            if let Some(x) = props.get("x").and_then(f64::deserialize) {
                self.x = x;
            }
            if let Some(y) = props.get("y").and_then(f64::deserialize) {
                self.y = y;
            }
            if let Some(vx) = props.get("vx").and_then(f64::deserialize) {
                self.vx = vx;
            }
            if let Some(vy) = props.get("vy").and_then(f64::deserialize) {
                self.vy = vy;
            }
            if let Some(color) = props.get("color").and_then(String::deserialize) {
                self.color = color;
            }
        }
    }
}

// Example: Complex nested state
struct GameWorld {
    player_pos: (f64, f64),
    enemies: Vec<(f64, f64)>,
    score: i64,
    level_name: String,
}

impl Object for GameWorld {
    fn receive(&mut self, msg: &Message) -> Value {
        match msg.selector.as_str() {
            "addEnemy:y:" => {
                if let (Some(Value::Float(x)), Some(Value::Float(y))) = 
                    (msg.args.get(0), msg.args.get(1)) {
                    self.enemies.push((*x, *y));
                }
                Value::Nil
            }
            "score" => Value::Int(self.score),
            _ => Value::Nil,
        }
    }
    
    fn serialize(&self) -> Value {
        // Serialize complex nested state
        let enemies_array = self.enemies.iter()
            .map(|(x, y)| dict! {
                "x" => x.serialize(),
                "y" => y.serialize()
            })
            .collect::<Vec<_>>();
            
        dict! {
            "player_x" => self.player_pos.0.serialize(),
            "player_y" => self.player_pos.1.serialize(),
            "enemies" => Value::Array(enemies_array),
            "score" => self.score.serialize(),
            "level_name" => self.level_name.serialize()
        }
    }
    
    fn deserialize(&mut self, state: &Value) {
        if let Value::Dict(props) = state {
            // Restore player position
            if let (Some(x), Some(y)) = (
                props.get("player_x").and_then(f64::deserialize),
                props.get("player_y").and_then(f64::deserialize)
            ) {
                self.player_pos = (x, y);
            }
            
            // Restore enemies
            if let Some(Value::Array(enemies)) = props.get("enemies") {
                self.enemies.clear();
                for enemy in enemies {
                    if let Value::Dict(enemy_props) = enemy {
                        if let (Some(x), Some(y)) = (
                            enemy_props.get("x").and_then(f64::deserialize),
                            enemy_props.get("y").and_then(f64::deserialize)
                        ) {
                            self.enemies.push((x, y));
                        }
                    }
                }
            }
            
            // Restore other fields
            if let Some(score) = props.get("score").and_then(i64::deserialize) {
                self.score = score;
            }
            if let Some(name) = props.get("level_name").and_then(String::deserialize) {
                self.level_name = name;
            }
        }
    }
}

// Runtime usage example:
fn hot_reload_example() {
    let mut runtime = runtime::Runtime::new();
    
    // Create some objects
    let particle = runtime.create("Particle").unwrap();
    let world = runtime.create("GameWorld").unwrap();
    
    // Do some work...
    runtime.send1(particle, "update", Value::Nil);
    runtime.send(world, "addEnemy:y:", vec![Value::Float(100.0), Value::Float(200.0)]);
    
    // Hot reload - state is automatically preserved through serialization
    runtime.hot_reload("./target/release/libmy_objects.dylib").unwrap();
    
    // Objects continue working with their state intact
    let score = runtime.send0(world, "score");
    println!("Score after hot reload: {:?}", score);
}