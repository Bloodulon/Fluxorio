use std::io::{stdout, Write};
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
const VIEW_W: usize = 60;
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

fn draw_sprite(out: &mut impl Write, sprite: &[&str], sx: i32, sy: i32) {
    for (row_i, line) in sprite.iter().enumerate() {
        let ry = sy + row_i as i32;
        if ry < 0 || ry >= VIEW_H as i32 { continue; }

        for (col_i, ch) in line.chars().enumerate() {
            if ch == ' ' { continue; }
            let rx = sx + col_i as i32;
            if rx < 0 || rx >= VIEW_W as i32 { continue; }

            execute!(out, MoveTo(rx as u16, ry as u16), Print(ch)).unwrap();
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
            let roll = rng.gen_range(0..100);
            map[y][x] = match roll {
                r if r < IRON_CHANCE                                   => 'i',
                r if r < IRON_CHANCE + COPPER_CHANCE                   => 'c',
                r if r < IRON_CHANCE + COPPER_CHANCE + COAL_CHANCE     => 'k',
                _ => ' ',
            };
        }
    }

    let mut px: i32 = MAP_W as i32 / 2;
    let mut py: i32 = MAP_H as i32 / 2;

    loop {
        let cam_x = px - VIEW_W as i32 / 2;
        let cam_y = py - VIEW_H as i32 / 2;

        for vy in 0..VIEW_H {
            for vx in 0..VIEW_W {
                execute!(out, MoveTo(vx as u16, vy as u16), Print(' ')).unwrap();
            }
        }

        for vy in 0..VIEW_H {
            for vx in 0..VIEW_W {
                let mx = cam_x + vx as i32;
                let my = cam_y + vy as i32;

                if mx < 0 || my < 0 || mx >= MAP_W as i32 || my >= MAP_H as i32 {
                    continue;
                }

                let tile = map[my as usize][mx as usize];
                if tile == ' ' { continue; }

                let sprite = ore_sprite(tile);
                draw_sprite(&mut out, sprite, vx as i32, vy as i32);
            }
        }

        let center_x = VIEW_W as i32 / 2 - 1;
        let center_y = VIEW_H as i32 / 2 - 1;
        draw_sprite(&mut out, &SPRITE_PLAYER, center_x, center_y);

        execute!(out, MoveTo(0, VIEW_H as u16), Print(format!(
            "Pos: ({}, {}) WASD=движение Q=выход",
            px, py
        ))).unwrap();

        out.flush().unwrap();

        if let Ok(Event::Key(key)) = event::read() {
            match key.code {
                KeyCode::Char('w') => py -= 1,
                KeyCode::Char('s') => py += 1,
                KeyCode::Char('a') => px -= 1,
                KeyCode::Char('d') => px += 1,
                KeyCode::Char('q') => break,
                _ => {}
            }
        }
    }

    terminal::disable_raw_mode().unwrap();
    execute!(out, terminal::LeaveAlternateScreen).unwrap();
}
