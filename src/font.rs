use sdl2::pixels::Color;
use sdl2::render::{Texture, TextureCreator};
use sdl2::ttf::Font;
use std::collections::HashMap;

pub struct FontAtlas<'a> {
    pub texture: Texture<'a>,
    pub char_width: u32,
    pub char_height: u32,
    // Карта: какой символ в каком квадрате сетки (x, y)
    pub uv_map: HashMap<char, (i32, i32)>,
}

impl<'a> FontAtlas<'a> {
    pub fn create<T>(font: &Font, texture_creator: &'a TextureCreator<T>) -> Result<Self, String> {
        let cell_w = 16;
        let cell_h = 16;
        let atlas_size = 256; // 16 cells * 16 pixels

        // 1. Создаем пустую поверхность (Surface) для рисования букв
        let mut surface = sdl2::surface::Surface::new(
            atlas_size,
            atlas_size,
            sdl2::pixels::PixelFormatEnum::RGBA8888,
        )?;

        let mut uv_map = HashMap::new();
        let mut current_char = 32u32;

        // Белое на прозрачном (в стиле Матрицы потом подсветим зеленым)
        let white = Color::RGB(255, 255, 255);
        let bw = Color::RGB(0, 0, 0);
        // Заполняем сетку 16x16
        for row in 0..16 {
            for col in 0..16 {
                // Выбираем символ: сначала ASCII, потом Кириллица (0x0400+)
                let c = match current_char {
                    0..=127 => std::char::from_u32(current_char),
                    128..=255 => std::char::from_u32(current_char - 128 + 0x0410), // А, Б, В...
                    _ => None,
                };

                if let Some(ch) = c {
                    // Рендерим символ в маленькую поверхность
                    if let Ok(char_surface) = font.render_char(ch).shaded(white, bw) {
                        let dest_rect = sdl2::rect::Rect::new(
                            (col * cell_w) as i32,
                            (row * cell_h) as i32,
                            cell_w,
                            cell_h,
                        );
                        // Копируем букву на общий атлас
                        char_surface.blit(None, &mut surface, Some(dest_rect))?;
                        uv_map.insert(ch, (dest_rect.x, dest_rect.y));
                    }
                }
                current_char += 1;
            }
        }

        // 2. Переносим Surface в Texture (в видеопамять)
        let texture = texture_creator
            .create_texture_from_surface(&surface)
            .map_err(|e| e.to_string())?;

        Ok(FontAtlas {
            texture,
            char_width: cell_w,
            char_height: cell_h,
            uv_map,
        })
    }
}
