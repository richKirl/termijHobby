use crate::font::FontAtlas;
use sdl2::event::Event;
use sdl2::event::WindowEvent;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

mod font;

#[derive(Clone)]
struct TerminalChar {
    c: char,
    fg: (u8, u8, u8), // Цвет текста (RGB)
    bg: (u8, u8, u8), // Цвет фона (RGB)
}

struct TerminalGrid {
    width: usize,
    height: usize,
    cells: Vec<Vec<TerminalChar>>,
    cursor_x: usize,
    cursor_y: usize,
}

impl TerminalGrid {
    fn new(w: usize, h: usize) -> Self {
        // Создаем пустую сетку с пробелами
        let mut cells = Vec::new();
        for _ in 0..h {
            let row = vec![
                TerminalChar {
                    c: ' ',
                    fg: (0, 255, 0),
                    bg: (0, 0, 0)
                };
                w
            ];
            cells.push(row);
        }
        Self {
            width: w,
            height: h,
            cells: cells,
            cursor_x: 0,
            cursor_y: 0,
        }
    }
    fn resize(&mut self, new_w: usize, new_h: usize) {
        // Изменяем количество строк
        self.cells.resize(
            new_h,
            vec![
                TerminalChar {
                    c: ' ',
                    fg: (0, 255, 0),
                    bg: (0, 0, 0)
                };
                new_w
            ],
        );

        // Изменяем длину каждой строки
        for row in self.cells.iter_mut() {
            row.resize(
                new_w,
                TerminalChar {
                    c: ' ',
                    fg: (0, 255, 0),
                    bg: (0, 0, 0),
                },
            );
        }

        self.width = new_w;
        self.height = new_h;

        // Корректируем курсор, чтобы он не оказался за пределами новой сетки
        if self.cursor_x >= self.width {
            self.cursor_x = self.width - 1;
        }
        if self.cursor_y >= self.height {
            self.cursor_y = self.height - 1;
        }
    }
    // Метод для записи символа и обработки переноса строки
    fn put_char(&mut self, ch: char) {
        if ch == '\n' {
            self.cursor_y += 1;
            self.cursor_x = 0;
        } else {
            if self.cursor_x < self.width && self.cursor_y < self.height {
                self.cells[self.cursor_y][self.cursor_x].c = ch;
                self.cursor_x += 1;
            }
        }
        // Тут еще нужна логика скроллинга, если cursor_y > height
    }
}

fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string())?;
    // Настраиваем размер окна (пока фиксированный, под сетку 80x24)
    let char_size_w = 8; // Размер одного символа (глифа)
    let char_size_h = 16;
    let cols = 80;
    let rows = 24;
    //println!("{} {}", cols * char_size, rows * char_size);

    // 1. Запускаем наш бинарник pty_proxy
    let mut child = Command::new("/home/klkl/testPT/target/release/testPT")
        .args(&["24", "80", "/bin/sh"]) // начальный размер и шелл
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start pty_proxy");

    let mut child_stdin = child.stdin.take().expect("Failed to open stdin");
    let mut child_stdout = child.stdout.take().expect("Failed to open stdout");

    // Канал для передачи данных от pty_proxy в SDL2
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();

    // 2. Поток чтения вывода (pty_proxy -> SDL2)
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        while let Ok(n) = child_stdout.read(&mut buffer) {
            if n == 0 {
                break;
            }
            let _ = tx.send(buffer[..n].to_vec());
        }
    });

    let window = video_subsystem
        .window(
            "Rust Matrix Terminal",
            cols * char_size_w,
            rows * char_size_h,
        )
        .position_centered()
        .resizable()
        .build()
        .map_err(|e| e.to_string())?;
    let display_scale = 1.0; // Например, увеличим шрифт в 2 раза
    let atlas_cell_size = 16; // Размер ячейки в файле атласа
    let mut grid = TerminalGrid::new(cols as usize, rows as usize);
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "1");
    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    let texture_creator = canvas.texture_creator();
    let font = ttf_context.load_font("/usr/local/share/fonts/ubuntu-font/UbuntuMono-R.ttf", 15)?;
    let mut atlas_texture = FontAtlas::create(&font, &texture_creator)?;
    // --- ПЕРЕД ЦИКЛОМ 'running ---
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl_context.event_pump()?;

    'running: loop {
        while let Ok(data) = rx.try_recv() {
            for byte in data {
                // Обработка базовых управляющих символов
                match byte {
                    b'\n' => {
                        grid.cursor_y += (1.0) as usize;
                        grid.cursor_x = 0;
                    }
                    b'\r' => grid.cursor_x = 0,
                    b'\x08' => {
                        //println!("{}", "BACKSPACE");
                        if grid.cursor_x > 0 {
                            grid.cursor_x -= 1;
                        }
                    } // Backspace
                    _ => grid.put_char(byte as char),
                }
            }
            // Скроллинг (если ушли за низ экрана)
            if grid.cursor_y >= grid.height {
                grid.cells.remove(0);
                grid.cells.push(vec![
                    TerminalChar {
                        c: ' ',
                        fg: (0, 255, 0),
                        bg: (0, 0, 0)
                    };
                    grid.width
                ]);
                grid.cursor_y = grid.height - 1;
            }
        }

        // 1. Обработка событий (Close, Resize, Keyboard)
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }
                Event::Window {
                    win_event: WindowEvent::SizeChanged(w, h),
                    ..
                } => {
                    let new_cols = (w as u32 / char_size_w) as usize;
                    let new_rows = (h as u32 / char_size_h) as usize;

                    if new_cols > 0 && new_rows > 0 {
                        grid.resize(new_cols, new_rows);
                        //println!("Grid resized to: {}x{}", new_cols, new_rows);
                        let rows = h / 16;
                        let cols = w / 16;
                        let resize_msg = format!("\x1b_RESIZE:{}:{}\x1b\\", rows, cols);
                        let _ = child_stdin.write_all(resize_msg.as_bytes());
                        let _ = child_stdin.flush();
                    }
                }
                Event::TextInput { text, .. } => {
                    let _ = child_stdin.write_all(text.as_bytes());
                    let _ = child_stdin.flush();
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Return),
                    ..
                } => {
                    let _ = child_stdin.write_all(b"\n");
                    let _ = child_stdin.flush();
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Backspace),
                    ..
                } => {
                    let _ = child_stdin.write_all(b"\x08");
                    let _ = child_stdin.flush();
                }
                // Event::KeyDown {
                //     keycode: Some(key), ..
                // } => {
                //     // Сюда мы потом прикрутим отправку нажатий в PTY канал
                //     //println!("Key pressed: {:?}", key);
                //     let _ = child_stdin.write_all(b"\n");
                //     let _ = child_stdin.flush();
                // }
                _ => {}
            }
        }

        // 2. Логика обновления (здесь будем читать из mpsc канала данные PTY)

        // 3. Отрисовка

        // --- ВНУТРИ ЦИКЛА 'running (Раздел 3. Отрисовка) ---
        canvas.set_draw_color(Color::RGB(0, 0, 0)); // Черный фон окна
        canvas.clear();

        let char_w = 8; // Ширина ячейки на экране
        let char_h = 16; // Высота ячейки на экране

        for y in 0..grid.height {
            for x in 0..grid.width {
                // На экране символ будет занимать:
                let screen_char_w = (atlas_cell_size as f32 * display_scale) as u32;
                let screen_char_h = (atlas_cell_size as f32 * display_scale) as u32;
                let t_char = &grid.cells[y][x];
                let dst_rect = Rect::new(
                    (x * char_w) as i32,
                    (y * char_h) as i32,
                    screen_char_w as u32,
                    screen_char_h as u32,
                );

                // 1. Рисуем фон ячейки (если он не черный)
                if t_char.bg != (0, 0, 0) {
                    canvas.set_draw_color(Color::RGB(t_char.bg.0, t_char.bg.1, t_char.bg.2));
                    canvas.fill_rect(dst_rect)?;
                }

                // 2. Рисуем символ
                if t_char.c != ' ' {
                    if let Some(&(px, py)) = atlas_texture.uv_map.get(&t_char.c) {
                        let src_rect =
                            Rect::new(px, py, atlas_texture.char_width, atlas_texture.char_height);

                        // Устанавливаем цвет текста из TerminalChar
                        atlas_texture
                            .texture
                            .set_color_mod(t_char.fg.0, t_char.fg.1, t_char.fg.2);
                        canvas.copy(&atlas_texture.texture, Some(src_rect), Some(dst_rect))?;
                    }
                }
            }
        }
        // Тут будет цикл отрисовки текстурного атласа

        canvas.present();

        // Ограничиваем FPS, чтобы не грузить процессор на 100%
        thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
