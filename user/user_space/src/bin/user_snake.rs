#![no_std]
#![no_main]

use user_lib::fs::OpenFlags;
use user_lib::mos::fs::FileDesc;
use user_lib::{clock_us, create_window, destroy_window, exit, open, rand, read,
               window_mark_dirty, window_sync};

// ─── 方向 ─────────────────────────────

#[derive(Copy, Clone, PartialEq, Eq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn delta(self) -> (isize, isize) {
        match self {
            Dir::Up => (0, -1),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }
    fn opposite(self) -> Self {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }
    fn from_char(c: u8) -> Option<Self> {
        match c {
            b'w' | b'W' => Some(Dir::Up),
            b's' | b'S' => Some(Dir::Down),
            b'a' | b'A' => Some(Dir::Left),
            b'd' | b'D' => Some(Dir::Right),
            _ => None,
        }
    }
}

// ─── 颜色 ─────────────────────────────

type RGBA = [u8; 4];
const GREEN: RGBA = [0, 255, 0, 255];
const DARK_GREEN: RGBA = [0, 180, 0, 255];
const RED: RGBA = [255, 50, 50, 255];
const BLACK: RGBA = [0, 0, 0, 255];
const WHITE: RGBA = [255, 255, 255, 255];
const DARK_RED: RGBA = [40, 0, 0, 255];

// ─── 游戏参数 ─────────────────────────

const CELL: usize = 10;
const COLS: usize = 40;
const ROWS: usize = 40;
const WIN_W: usize = COLS * CELL; // 400
const WIN_H: usize = ROWS * CELL; // 400
const TICK_US: u64 = 150_000; // 150ms per move

// ─── 全局状态 (UnsafeCell 避免 Sync 限制, 游戏单线程使用) ───

struct Game {
    snake: [(usize, usize); 1024],
    snake_len: usize,
    dir: Dir,
    food: (usize, usize),
    game_over: bool,
    score: u64,
    cols: usize,
    rows: usize,
}

static mut GAME: Game = Game::new();

impl Game {
    const fn new() -> Self {
        Self {
            snake: [(0, 0); 1024],
            snake_len: 0,
            dir: Dir::Right,
            food: (0, 0),
            game_over: false,
            score: 0,
            cols: COLS,
            rows: ROWS,
        }
    }

    fn init(&mut self) {
        let hx = self.cols / 2;
        let hy = self.rows / 2;
        self.snake[0] = (hx, hy);
        self.snake[1] = (hx - 1, hy);
        self.snake[2] = (hx - 2, hy);
        self.snake_len = 3;
        self.dir = Dir::Right;
        self.game_over = false;
        self.score = 0;
        self.spawn_food();
    }

    fn spawn_food(&mut self) {
        loop {
            let r = rand();
            let fx = r as usize % self.cols;
            let fy = (r >> 32) as usize % self.rows;
            let mut found = false;
            for i in 0..self.snake_len {
                if self.snake[i] == (fx, fy) {
                    found = true;
                    break;
                }
            }
            if !found {
                self.food = (fx, fy);
                return;
            }
        }
    }

    fn tick(&mut self) {
        if self.game_over {
            return;
        }
        let (dx, dy) = self.dir.delta();
        let head = self.snake[0];
        let nx = (head.0 as isize + dx).rem_euclid(self.cols as isize) as usize;
        let ny = (head.1 as isize + dy).rem_euclid(self.rows as isize) as usize;

        // 碰撞自身 (不含尾部, 因为尾部会弹出)
        for i in 0..self.snake_len.saturating_sub(1) {
            if self.snake[i] == (nx, ny) {
                self.game_over = true;
                return;
            }
        }

        // 移位
        for i in (1..self.snake_len).rev() {
            self.snake[i] = self.snake[i - 1];
        }
        self.snake[0] = (nx, ny);

        if (nx, ny) == self.food {
            self.score += 1;
            self.snake_len += 1;
            self.spawn_food();
        }
    }

    fn handle_input(&mut self, tty: FileDesc) -> bool {
        let mut buf = [0u8; 8];
        loop {
            match read(tty, &mut buf) {
                Ok(n) => {
                    for i in 0..n {
                        match buf[i] {
                            b'q' | b'Q' => return true, // 退出
                            c => {
                                if let Some(d) = Dir::from_char(c) {
                                    if d.opposite() != self.dir {
                                        self.dir = d;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
        false
    }
}

// ─── 绘制 ─────────────────────────────

fn fill_cell(buf: &mut [RGBA], pitch: usize, cx: usize, cy: usize, color: RGBA) {
    let px = cx * CELL;
    let py = cy * CELL;
    for y in py..py + CELL {
        let row_off = y * pitch;
        for x in px..px + CELL {
            buf[row_off + x] = color;
        }
    }
}

// ─── 主循环 ─────────────────────────

#[unsafe(no_mangle)]
fn main() -> i32 {
    // 打开非阻塞 tty 用于键盘输入
    let tty = match open(c"/dev/tty", OpenFlags::READ | OpenFlags::NONBLOCK) {
        Some(fd) => fd,
        None => return -1,
    };

    // 创建窗口 (避开 kernel GameOfLife@1500,500 和 BadApple@1000,500)
    let info = match create_window(100, 100, WIN_W as u64, WIN_H as u64) {
        Some(info) => info,
        None => return -1,
    };
    let pitch = info.pitch as usize;
    let buf = unsafe {
        core::slice::from_raw_parts_mut(info.pixels_ptr as *mut RGBA, pitch * WIN_H)
    };

    let game = unsafe { &mut *core::ptr::addr_of_mut!(GAME) };
    game.init();

    let mut last_tick = clock_us();

    loop {
        // 1. 处理键盘输入 (返回 true 表示按下 Q 退出)
        if game.handle_input(tty) {
            destroy_window(info.id);
            exit(0);
        }

        // 2. 游戏 tick
        let now = clock_us();
        if now - last_tick >= TICK_US {
            game.tick();
            last_tick = now;
        }

        // 3. 绘制
        // 背景
        buf.fill(BLACK);

        if game.game_over {
            // 棋盘格游戏结束画面
            for y in 0..WIN_H {
                let row_off = y * pitch;
                for x in 0..WIN_W {
                    buf[row_off + x] = if (x / CELL + y / CELL) & 1 == 0 {
                        DARK_RED
                    } else {
                        BLACK
                    };
                }
            }
            window_mark_dirty(info.id, 0, 0, WIN_W as u64, WIN_H as u64);
            window_sync();

            // 等待 2 秒后重新开始
            let restart_at = clock_us() + 2_000_000;
            while clock_us() < restart_at {}
            game.init();
            last_tick = clock_us();
            continue;
        }

        // 食物
        fill_cell(buf, pitch, game.food.0, game.food.1, RED);

        // 蛇
        for i in 0..game.snake_len {
            let (sx, sy) = game.snake[i];
            let color = if i == 0 { GREEN } else { DARK_GREEN };
            fill_cell(buf, pitch, sx, sy, color);
        }

        // 边框
        for x in 0..WIN_W {
            buf[x] = WHITE;
            buf[(WIN_H - 1) * pitch + x] = WHITE;
        }
        for y in 1..WIN_H - 1 {
            let row_off = y * pitch;
            buf[row_off] = WHITE;
            buf[row_off + WIN_W - 1] = WHITE;
        }

        // 4. 提交 (全帧脏, 因为是全屏重绘)
        window_mark_dirty(info.id, 0, 0, WIN_W as u64, WIN_H as u64);
        window_sync();
    }
}
