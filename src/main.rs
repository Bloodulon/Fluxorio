use std::io::{stdout, Write};
use crossterm::event::KeyEventKind;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode},
    execute,
    style::Print,
    terminal,
};
use rand::Rng;

const MAP_W: usize = 200;
const MAP_H: usize = 200;
const VIEW_W: usize = 120;
const VIEW_H: usize = 24;

const IRON_CHANCE: u32 = 2;
const COPPER_CHANCE: u32 = 2;
const COAL_CHANCE: u32 = 2;

const SPRITE_PLAYER: [&str; 3] = [
    " O ",
    "/|\\",
    "/ \\",
];

const SPRITE_IRON: [&str; 2] = [
    "<.>",
    "/.\\",
];

const SPRITE_COPPER: [&str; 2] = [
    "/~\\",
    "\\~/",
];

const SPRITE_COAL: [&str; 2] = [
    "(o)",
    "o(o)",
];

fn ore_sprite(ore: char) -> &'static [&'static str] {
    match ore {
        'i' => &SPRITE_IRON,
        'c' => &SPRITE_COPPER,
        'k' => &SPRITE_COAL,
        _   => &[],
    }
}

fn stamp_sprite(buffer: &mut Vec<Vec<char>>, sprite: &[&str], sx: i32, sy: i32) {
    for (row_i, line) in sprite.iter().enumerate() {
        let ry = sy + row_i as i32;
        if ry < 0 || ry >= VIEW_H as i32 { continue; }

        for (col_i, ch) in line.chars().enumerate() {
            if ch == ' ' { continue; }
            let rx = sx + col_i as i32;
            if rx < 0 || rx >= VIEW_W as i32 { continue; }

            buffer[ry as usize][rx as usize] = ch;
        }
    }
}

fn main() {
    let mut out = stdout();
    execute!(out, terminal::EnterAlternateScreen).unwrap();
    terminal::enable_raw_mode().unwrap();

    let mut rng = rand::thread_rng();
    let mut map = vec![vec![' '; MAP_W]; MAP_H];

    for y in 0..MAP_H {
        for x in 0..MAP_W {
            let roll = rng.gen_range(0..1000);
            map[y][x] = match roll {
                r if r < IRON_CHANCE                               => 'i',
                r if r < IRON_CHANCE + COPPER_CHANCE               => 'c',
                r if r < IRON_CHANCE + COPPER_CHANCE + COAL_CHANCE => 'k',
                _ => ' ',
            };
        }
    }

    let mut px: i32 = MAP_W as i32 / 2;
    let mut py: i32 = MAP_H as i32 / 2;

    loop {
        let cam_x = px - VIEW_W as i32 / 2;
        let cam_y = py - VIEW_H as i32 / 2;

        let mut buffer = vec![vec![' '; VIEW_W]; VIEW_H];

        for vy in 0..VIEW_H {
            for vx in 0..VIEW_W {
                let mx = cam_x + vx as i32;
                let my = cam_y + vy as i32;
                if mx < 0 || my < 0 || mx >= MAP_W as i32 || my >= MAP_H as i32 { continue; }

                let tile = map[my as usize][mx as usize];
                if tile == ' ' { continue; }

                stamp_sprite(&mut buffer, ore_sprite(tile), vx as i32, vy as i32);
            }
        }

        // Записываем игрока поверх
        let cx = VIEW_W as i32 / 2 - 1;
        let cy = VIEW_H as i32 / 2 - 1;
        stamp_sprite(&mut buffer, &SPRITE_PLAYER, cx, cy);

        for (vy, row) in buffer.iter().enumerate() {
            let line: String = row.iter().collect();
            execute!(out, MoveTo(0, vy as u16), Print(&line)).unwrap();
        }

        // HUD
        execute!(out, MoveTo(0, VIEW_H as u16), Print(format!(
            "Pos: ({}, {})  i=железо c=медь k=уголь  WASD=движение Q=выход",
            px, py
        ))).unwrap();

        out.flush().unwrap();

        if let Ok(Event::Key(key)) = event::read() {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('w') => py -= 1,
                    KeyCode::Char('s') => py += 1,
                    KeyCode::Char('a') => px -= 2,
                    KeyCode::Char('d') => px += 2,
                    KeyCode::Char('q') => break,
                    _ => {}
                }
            }
        }
    }

    terminal::disable_raw_mode().unwrap();
    execute!(out, terminal::LeaveAlternateScreen).unwrap();
}
