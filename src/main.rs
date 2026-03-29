use crossterm::event::{KeyEventKind, KeyModifiers};
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode},
    execute,
    style::Print,
    terminal,
};
use rand::Rng;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::io::{stdout, Write};

fn transliterate_key(code: KeyCode) -> KeyCode {
    if let KeyCode::Char(c) = code {
        let mapped = match c {
            'й' | 'Й' => 'q',
            'ц' | 'Ц' => 'w',
            'у' | 'У' => 'e',
            'к' | 'К' => 'r',
            'е' | 'Е' => 't',
            'н' | 'Н' => 'y',
            'г' | 'Г' => 'u',
            'ш' | 'Ш' => 'i',
            'щ' | 'Щ' => 'o',
            'з' | 'З' => 'p',
            'ф' | 'Ф' => 'a',
            'ы' | 'Ы' => 's',
            'в' | 'В' => 'd',
            'а' | 'А' => 'f',
            'п' | 'П' => 'g',
            'р' | 'Р' => 'h',
            'о' | 'О' => 'j',
            'л' | 'Л' => 'k',
            'д' | 'Д' => 'l',
            'я' | 'Я' => 'z',
            'ч' | 'Ч' => 'x',
            'с' | 'С' => 'c',
            'м' | 'М' => 'v',
            'и' | 'И' => 'b',
            'т' | 'Т' => 'n',
            'ь' | 'Ь' => 'm',
            'б' | 'Б' => ',',
            'ю' | 'Ю' => '.',
            _ => c,
        };
        KeyCode::Char(mapped)
    } else {
        code
    }
}

const MAP_W: usize = 200;
const MAP_H: usize = 200;
const VIEW_W: usize = 80;
const VIEW_H: usize = 24;
const TICK_RATE: u64 = 50;
const MOVE_COOLDOWN_MS: u64 = 80;
const GATHER_RADIUS: i32 = 1;

#[derive(Clone, PartialEq)]
enum Tile {
    Empty,
    IronOre,
    CopperOre,
    CoalOre,
    Belt {
        direction: Direction,
        items: VecDeque<Item>,
    },
    Furnace {
        progress: f32,
        recipe: Option<Recipe>,
    },
    Inserter {
        state: InserterState,
    },
    Chest {
        items: VecDeque<Item>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Direction {
    N,
    E,
    S,
    W,
}

impl Direction {
    fn delta(&self) -> (i32, i32) {
        match self {
            Direction::N => (0, -1),
            Direction::E => (1, 0),
            Direction::S => (0, 1),
            Direction::W => (-1, 0),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum ItemType {
    IronOre,
    CopperOre,
    Coal,
    IronPlate,
    CopperPlate,
}

#[derive(Clone, PartialEq)]
struct Item {
    kind: ItemType,
}

#[derive(Clone, Copy, PartialEq)]
enum InserterState {
    Idle,
    Reaching,
    Grabbing,
    Retreating,
}

#[derive(Clone, PartialEq)]
struct Recipe {
    input: ItemType,
    output: ItemType,
    time: f32,
}

const RECIPES: &[Recipe] = &[Recipe {
    input: ItemType::IronOre,
    output: ItemType::IronPlate,
    time: 3.0,
}];

fn make_recipe(kind: ItemType) -> Option<Recipe> {
    RECIPES.iter().find(|r| r.input == kind).cloned()
}

fn tile_sprite(tile: &Tile) -> &'static [&'static str] {
    match tile {
        Tile::Empty => &[],
        Tile::IronOre => &["<i>", "/i\\"],
        Tile::CopperOre => &["/~\\", "\\~/"],
        Tile::CoalOre => &["(o)", "o(o)"],
        Tile::Belt { direction, .. } => match direction {
            Direction::N | Direction::S => &[" | ", " V "],
            Direction::E | Direction::W => &["==>", "==="],
        },
        Tile::Furnace { progress, .. } => {
            if *progress > 0.0 {
                &["[F]", "/#\\"]
            } else {
                &["[F]", "/.\\"]
            }
        }
        Tile::Inserter { state } => match state {
            InserterState::Idle => &[" | ", " O "],
            InserterState::Reaching => &[" /|", " O "],
            InserterState::Grabbing => &["/|=", " O "],
            InserterState::Retreating => &["\\| ", " O "],
        },
        Tile::Chest { .. } => &["[C]", "/+\\"],
    }
}

fn stamp_sprite(buffer: &mut Vec<Vec<char>>, sprite: &[&str], sx: i32, sy: i32) {
    for (row_i, line) in sprite.iter().enumerate() {
        let ry = sy + row_i as i32;
        if ry < 0 || ry >= VIEW_H as i32 {
            continue;
        }
        for (col_i, ch) in line.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let rx = sx + col_i as i32;
            if rx < 0 || rx >= VIEW_W as i32 {
                continue;
            }
            buffer[ry as usize][rx as usize] = ch;
        }
    }
}

fn stamp_string(buffer: &mut Vec<Vec<char>>, s: &str, sx: i32, sy: i32) {
    for (i, ch) in s.chars().enumerate() {
        let rx = sx + i as i32;
        if rx < 0 || rx >= VIEW_W as i32 {
            continue;
        }
        if sy >= 0 && sy < VIEW_H as i32 {
            buffer[sy as usize][rx as usize] = ch;
        }
    }
}

struct Game {
    map: Vec<Vec<Tile>>,
    player_x: i32,
    player_y: i32,
    build_mode: BuildMode,
    tick: u64,
    inventory: VecDeque<Item>,
    craft_open: bool,
    crafting: Option<(Recipe, f32)>,
}

#[derive(Clone)]
enum BuildMode {
    None,
    Belt(Direction),
    Furnace,
    Inserter,
    Destroy,
}

impl Game {
    fn new() -> Self {
        let mut map = vec![vec![Tile::Empty; MAP_W]; MAP_H];
        let mut rng = rand::thread_rng();
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let roll = rng.gen_range(0..1000);
                map[y][x] = match roll {
                    r if r < 10 => Tile::IronOre,
                    r if r < 18 => Tile::CopperOre,
                    r if r < 24 => Tile::CoalOre,
                    _ => Tile::Empty,
                };
            }
        }
        Self {
            map,
            player_x: MAP_W as i32 / 2,
            player_y: MAP_H as i32 / 2,
            build_mode: BuildMode::None,
            tick: 0,
            inventory: VecDeque::new(),
            craft_open: false,
            crafting: None,
        }
    }

    fn tick(&mut self) {
        self.tick += 1;

        for y in 0..MAP_H {
            for x in 0..MAP_W {
                match &mut self.map[y][x] {
                    Tile::Belt { items, direction } => {
                        if items.len() > 1 {
                            continue;
                        }
                        if items.pop_front().is_some() {
                            let (dx, dy) = direction.delta();
                            let nx = x as i32 + dx;
                            let ny = y as i32 + dy;
                            if nx >= 0 && ny >= 0 && nx < MAP_W as i32 && ny < MAP_H as i32 {
                                if let Tile::Belt {
                                    items: ref mut next,
                                    ..
                                } = self.map[ny as usize][nx as usize]
                                {
                                    if next.len() < 8 {
                                        next.push_back(Item {
                                            kind: ItemType::IronOre,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Tile::Furnace { progress, recipe } => {
                        if recipe.is_none() {
                            if let Some(rec) = make_recipe(ItemType::IronOre) {
                                *recipe = Some(rec);
                            }
                        }
                        if let Some(_rec) = recipe {
                            *progress += 1.0 / 3.0 * 0.1;
                            if *progress >= 1.0 {
                                *progress = 0.0;
                            }
                        }
                    }
                    Tile::Inserter { state } => {
                        *state = match state {
                            InserterState::Idle => InserterState::Idle,
                            InserterState::Reaching => InserterState::Grabbing,
                            InserterState::Grabbing => InserterState::Retreating,
                            InserterState::Retreating => InserterState::Idle,
                        };
                    }
                    _ => {}
                }
            }
        }

        if let Some((recipe, progress)) = &mut self.crafting {
            *progress += 1.0 / recipe.time * 0.1;
            if *progress >= 1.0 {
                self.inventory.push_back(Item {
                    kind: recipe.output.clone(),
                });
                self.crafting = None;
            }
        }
    }

    fn handle_input(&mut self, key: KeyCode, mods: KeyModifiers) {
        let key = transliterate_key(key);

        if key == KeyCode::Char('c') && !mods.contains(KeyModifiers::SHIFT) {
            self.craft_open = !self.craft_open;
            return;
        }

        if self.craft_open {
            match key {
                KeyCode::Char('1') => self.try_start_craft(ItemType::IronOre),
                KeyCode::Char('2') => self.try_start_craft(ItemType::CopperOre),
                KeyCode::Char('3') => self.try_start_craft(ItemType::Coal),
                KeyCode::Esc => self.craft_open = false,
                _ => {}
            }
            return;
        }

        match key {
            KeyCode::Char('1') => self.build_mode = BuildMode::Belt(Direction::E),
            KeyCode::Char('2') => self.build_mode = BuildMode::Furnace,
            KeyCode::Char('3') => self.build_mode = BuildMode::Inserter,
            KeyCode::Char('4') => self.build_mode = BuildMode::Destroy,
            KeyCode::Char(' ') => self.try_place(),
            KeyCode::Char('e') => self.try_interact(),
            KeyCode::Char('r') => self.build_mode = BuildMode::None,
            KeyCode::Char('o') => self.try_rotate(),
            _ => {}
        }
    }

    fn try_start_craft(&mut self, input: ItemType) {
        if self.crafting.is_some() {
            return;
        }
        let count = self.inventory.iter().filter(|i| i.kind == input).count();
        if count > 0 {
            self.inventory.retain(|i| i.kind != input);
            if let Some(recipe) = make_recipe(input) {
                self.crafting = Some((recipe, 0.0));
            }
        }
    }

    fn try_place(&mut self) {
        let x = self.player_x;
        let y = self.player_y;
        if x < 0 || y < 0 || x >= MAP_W as i32 || y >= MAP_H as i32 {
            return;
        }
        let tile = &self.map[y as usize][x as usize];
        if !matches!(tile, Tile::Empty) && !matches!(self.build_mode, BuildMode::Destroy) {
            return;
        }

        match &self.build_mode {
            BuildMode::Belt(dir) => {
                self.map[y as usize][x as usize] = Tile::Belt {
                    direction: *dir,
                    items: VecDeque::new(),
                };
            }
            BuildMode::Furnace => {
                self.map[y as usize][x as usize] = Tile::Furnace {
                    progress: 0.0,
                    recipe: None,
                };
            }
            BuildMode::Inserter => {
                self.map[y as usize][x as usize] = Tile::Inserter {
                    state: InserterState::Idle,
                };
            }
            BuildMode::Destroy => {
                self.map[y as usize][x as usize] = Tile::Empty;
            }
            BuildMode::None => {}
        }
    }

    fn try_interact(&mut self) {
        let mut gathered = false;

        for dy in -GATHER_RADIUS..=GATHER_RADIUS {
            for dx in -GATHER_RADIUS..=GATHER_RADIUS {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let x = self.player_x + dx;
                let y = self.player_y + dy;
                if x < 0 || y < 0 || x >= MAP_W as i32 || y >= MAP_H as i32 {
                    continue;
                }

                match &self.map[y as usize][x as usize] {
                    Tile::IronOre | Tile::CopperOre | Tile::CoalOre => {
                        let kind = match self.map[y as usize][x as usize] {
                            Tile::IronOre => ItemType::IronOre,
                            Tile::CopperOre => ItemType::CopperOre,
                            Tile::CoalOre => ItemType::Coal,
                            _ => continue,
                        };
                        self.map[y as usize][x as usize] = Tile::Empty;
                        self.inventory.push_back(Item { kind });
                        gathered = true;
                        break;
                    }
                    _ => {}
                }
            }
            if gathered {
                break;
            }
        }

        if !gathered {
            let x = self.player_x;
            let y = self.player_y;
            if x >= 0 && y >= 0 && x < MAP_W as i32 && y < MAP_H as i32 {
                match &self.map[y as usize][x as usize] {
                    Tile::IronOre | Tile::CopperOre | Tile::CoalOre => {
                        let kind = match self.map[y as usize][x as usize] {
                            Tile::IronOre => ItemType::IronOre,
                            Tile::CopperOre => ItemType::CopperOre,
                            Tile::CoalOre => ItemType::Coal,
                            _ => return,
                        };
                        self.map[y as usize][x as usize] = Tile::Empty;
                        self.inventory.push_back(Item { kind });
                    }
                    _ => {}
                }
            }
        }
    }

    fn try_rotate(&mut self) {
        if let BuildMode::Belt(dir) = &mut self.build_mode {
            *dir = match dir {
                Direction::N => Direction::E,
                Direction::E => Direction::S,
                Direction::S => Direction::W,
                Direction::W => Direction::N,
            };
        }
    }

    fn render(&self, menu_x: i32, menu_width: i32, menu_height: i32) -> Vec<Vec<char>> {
        let cam_x = self.player_x - VIEW_W as i32 / 2;
        let cam_y = self.player_y - VIEW_H as i32 / 2;
        let mut buffer = vec![vec![' '; VIEW_W]; VIEW_H];

        for vy in 0..VIEW_H {
            for vx in 0..VIEW_W {
                let mx = cam_x + vx as i32;
                let my = cam_y + vy as i32;
                if mx < 0 || my < 0 || mx >= MAP_W as i32 || my >= MAP_H as i32 {
                    continue;
                }
                let tile = &self.map[my as usize][mx as usize];
                let sprite = tile_sprite(tile);
                if !sprite.is_empty() {
                    stamp_sprite(&mut buffer, sprite, vx as i32, vy as i32);
                }
            }
        }

        let cx = VIEW_W as i32 / 2 - 1;
        let cy = VIEW_H as i32 / 2 - 1;
        stamp_sprite(&mut buffer, &[" O ", "/|\\", "/ \\"], cx, cy);

        if self.craft_open {
            self.render_craft_menu_to_buffer(&mut buffer, menu_x, menu_width, menu_height);
        }

        buffer
    }

    fn render_craft_menu_to_buffer(
        &self,
        buffer: &mut Vec<Vec<char>>,
        menu_x: i32,
        _menu_width: i32,
        menu_height: i32,
    ) {
        let lines = self.get_craft_menu_lines();
        for (i, line) in lines.iter().enumerate() {
            let y = 1 + i as i32;
            if y >= menu_height {
                break;
            }
            stamp_string(buffer, line, menu_x, y);
        }
    }

    fn get_craft_menu_lines(&self) -> Vec<String> {
        let iron_count = self
            .inventory
            .iter()
            .filter(|i| i.kind == ItemType::IronOre)
            .count();
        let copper_count = self
            .inventory
            .iter()
            .filter(|i| i.kind == ItemType::CopperOre)
            .count();
        let coal_count = self
            .inventory
            .iter()
            .filter(|i| i.kind == ItemType::Coal)
            .count();
        let plate_count = self
            .inventory
            .iter()
            .filter(|i| i.kind == ItemType::IronPlate)
            .count();

        let mut lines = Vec::new();
        lines.push("╔════════════════╗".to_string());
        lines.push("║   CRAFT MENU   ║".to_string());
        lines.push("╠════════════════╣".to_string());
        lines.push("║[1]Iron->Plate  ║".to_string());
        lines.push("║[2]Copper->Plate║".to_string());
        lines.push("║[3]Coal (soon)  ║".to_string());
        lines.push("╠════════════════╣".to_string());
        lines.push(format!("║Iron:{:>2}        ║", iron_count));
        lines.push(format!("║Copper:{:>1}       ║", copper_count));
        lines.push(format!("║Coal:{:>2}        ║", coal_count));
        lines.push(format!("║Plate:{:>1}        ║", plate_count));
        lines.push("╠════════════════╣".to_string());

        if let Some((_recipe, progress)) = &self.crafting {
            lines.push(format!("║{:.0}% complete   ║", progress * 100.0));
        } else {
            lines.push("║[E] to gather   ║".to_string());
        }
        lines.push("╚════════════════╝".to_string());
        lines
    }
}

fn main() {
    let mut out = stdout();
    execute!(out, terminal::EnterAlternateScreen).unwrap();
    terminal::enable_raw_mode().unwrap();

    let mut game = Game::new();
    let mut last_tick = std::time::Instant::now();
    let mut last_move = std::time::Instant::now();
    let mut pressed_keys: HashSet<KeyCode> = HashSet::new();
    let mut prev_diag = false;

    let menu_width = 16;
    let menu_x = VIEW_W as i32 - menu_width - 1;
    let menu_height = 20;

    loop {
        while let Ok(true) = event::poll(std::time::Duration::from_millis(10)) {
            if let Ok(Event::Key(key)) = event::read() {
                match key.kind {
                    KeyEventKind::Press => {
                        pressed_keys.insert(key.code);
                        if key.code == KeyCode::Char('q') {
                            break;
                        }
                        game.handle_input(key.code, key.modifiers);
                    }
                    KeyEventKind::Release => {
                        pressed_keys.remove(&key.code);
                    }
                    _ => {}
                }
            }
        }

        if pressed_keys.contains(&KeyCode::Char('q')) {
            break;
        }

        let now = std::time::Instant::now();
        if now - last_move >= std::time::Duration::from_millis(MOVE_COOLDOWN_MS) {
            let mut dx: i32 = 0;
            let mut dy: i32 = 0;

            if pressed_keys.contains(&KeyCode::Char('w')) || pressed_keys.contains(&KeyCode::Up) {
                dy -= 1;
            }
            if pressed_keys.contains(&KeyCode::Char('s')) || pressed_keys.contains(&KeyCode::Down) {
                dy += 1;
            }
            if pressed_keys.contains(&KeyCode::Char('a')) || pressed_keys.contains(&KeyCode::Left) {
                dx -= 1;
            }
            if pressed_keys.contains(&KeyCode::Char('d')) || pressed_keys.contains(&KeyCode::Right)
            {
                dx += 1;
            }

            let is_diag = dx != 0 && dy != 0;
            if is_diag {
                dx = dx.signum();
                dy = dy.signum();
            }

            if dx != 0 || dy != 0 {
                if !is_diag || !prev_diag {
                    game.player_x = (game.player_x + dx).clamp(0, MAP_W as i32 - 1);
                    game.player_y = (game.player_y + dy).clamp(0, MAP_H as i32 - 1);
                }
            }
            prev_diag = is_diag;
            last_move = now;
        }

        let buffer = game.render(menu_x, menu_width, menu_height);

        for (vy, row) in buffer.iter().enumerate() {
            let line: String = row.iter().collect();
            let padded_line = format!("{:<width$}", line, width = VIEW_W);
            execute!(out, MoveTo(0, vy as u16), Print(&padded_line)).unwrap();
        }

        let mode_str: String = match &game.build_mode {
            BuildMode::None => "1=лента 2=печь 3=инсертер 4=разрушить R=отмена".to_string(),
            BuildMode::Belt(d) => format!("Лента: {:?}", d),
            BuildMode::Furnace => "Печь".to_string(),
            BuildMode::Inserter => "Инсертер".to_string(),
            BuildMode::Destroy => "Разрушить".to_string(),
        };

        let status = format!(
            "Tick:{:4} Pos:({},{}) | {} | SPACE=place E=gather C=craft Q=quit",
            game.tick, game.player_x, game.player_y, mode_str
        );
        let padded_status = format!("{:<width$}", status, width = VIEW_W);
        execute!(out, MoveTo(0, VIEW_H as u16), Print(&padded_status)).unwrap();

        out.flush().unwrap();

        if std::time::Instant::now() - last_tick >= std::time::Duration::from_millis(TICK_RATE) {
            game.tick();
            last_tick = std::time::Instant::now();
        }
    }

    terminal::disable_raw_mode().unwrap();
    execute!(out, terminal::LeaveAlternateScreen).unwrap();

    let iron = game
        .inventory
        .iter()
        .filter(|i| i.kind == ItemType::IronOre)
        .count();
    let copper = game
        .inventory
        .iter()
        .filter(|i| i.kind == ItemType::CopperOre)
        .count();
    let plates = game
        .inventory
        .iter()
        .filter(|i| i.kind == ItemType::IronPlate)
        .count();
    println!("Done! Iron:{} Copper:{} Plates:{}", iron, copper, plates);
}
