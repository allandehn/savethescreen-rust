use crate::savers::SAVERS;

pub const BG: [f32; 4] = hex(0x12110e, 1.0);
pub const RAISE: [f32; 4] = hex(0x1b1914, 1.0);
pub const INK: [u8; 4] = [0xec, 0xe7, 0xdc, 255];
pub const MUTED: [u8; 4] = [0x9a, 0x94, 0x86, 255];
pub const INK_SOFT: [u8; 4] = [0xec, 0xe7, 0xdc, 199];
pub const BRAND: [u8; 4] = [0xec, 0xe7, 0xdc, 158];
pub const AMBER: [u8; 4] = [0xe0, 0xa4, 0x5a, 255];

const LINE: [f32; 4] = hex(0xece7dc, 0.10);
const HOVER: [f32; 4] = hex(0xe0a45a, 0.55);
const SHADOW: [f32; 4] = hex(0x0a0907, 1.0);

pub const fn hex(rgb: u32, a: f32) -> [f32; 4] {
    [
        ((rgb >> 16) & 255) as f32 / 255.0,
        ((rgb >> 8) & 255) as f32 / 255.0,
        (rgb & 255) as f32 / 255.0,
        a,
    ]
}

#[derive(Clone)]
pub struct TextRun {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub size: f32,
    pub line_height: f32,
    pub color: [u8; 4],
    pub serif: bool,
    pub italic: bool,
    /// Extra letter spacing in ems. The shaper multiplies this by the font size.
    pub tracking: f32,
    pub clip: Option<[f32; 4]>,
}

#[derive(Clone, Copy)]
pub struct CardGeom {
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy)]
pub struct PaintRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
    pub radius: f32,
    pub mode: f32,
}

pub struct Layout {
    pub cards: Vec<CardGeom>,
    pub texts: Vec<TextRun>,
    pub rects: Vec<PaintRect>,
    /// Vertical offset applied to this layout, already clamped to the content.
    pub scroll: f32,
}

impl Layout {
    pub fn hit(&self, x: f32, y: f32) -> Option<usize> {
        self.cards.iter().rev().find_map(|card| {
            (x >= card.x && y >= card.y && x < card.x + card.w && y < card.y + card.h)
                .then_some(card.index)
        })
    }
}

pub fn gallery(win_w: f32, win_h: f32, scroll: f32) -> Layout {
    let cols: usize = if win_w <= 760.0 {
        1
    } else if win_w <= 1280.0 {
        3
    } else {
        5
    };
    let margin = if win_w <= 760.0 { 16.0 } else { 24.0 };
    let content_w = (win_w - margin * 2.0).min(1480.0);
    let x0 = (win_w - content_w) * 0.5;
    let gap = 14.4;
    let card_w = (content_w - gap * (cols as f32 - 1.0)) / cols as f32;
    let aspect = if win_w <= 760.0 { 16.0 / 10.0 } else { 4.0 / 5.0 };
    let card_h = card_w / aspect;
    let wide = win_w > 1280.0;
    let title_size = (win_w * 0.046).clamp(38.0, 70.0);

    let day: Vec<usize> = SAVERS
        .iter()
        .enumerate()
        .filter(|(_, saver)| !saver.night)
        .map(|(i, _)| i)
        .collect();
    let night: Vec<usize> = SAVERS
        .iter()
        .enumerate()
        .filter(|(_, saver)| saver.night)
        .map(|(i, _)| i)
        .collect();

    let group_h = |count: usize| -> f32 {
        let rows = count.div_ceil(cols) as f32;
        rows * card_h + (rows - 1.0).max(0.0) * gap
    };
    let lede_h = 76.0;
    let header_h = if wide {
        22.0 + title_size + 18.0
    } else {
        22.0 + title_size + 12.0 + lede_h + 16.0
    };
    let content_h = header_h + group_h(day.len()) + 40.0 + group_h(night.len()) + 96.0;
    // A tiled window is often shorter than two groups of cards. Scroll instead of clipping Night.
    let max_scroll = (content_h + 32.0 - win_h).max(0.0);
    let scroll = if max_scroll == 0.0 {
        0.0
    } else {
        scroll.clamp(0.0, max_scroll)
    };
    let mut y = if max_scroll == 0.0 {
        (win_h - content_h) * 0.5
    } else {
        18.0 - scroll
    };

    let mut texts = Vec::new();
    let mut rects = Vec::new();
    let mut cards = Vec::new();

    texts.push(run(
        "TEN FULLSCREEN SAVERS",
        x0,
        y,
        content_w,
        11.5,
        16.0,
        AMBER,
        false,
        false,
        0.18,
        None,
    ));
    texts.push(run(
        "Save the Screen",
        x0,
        y + 18.0,
        content_w * 0.7,
        title_size,
        title_size * 0.95,
        INK,
        true,
        true,
        0.0,
        None,
    ));

    let lede = "Open one, press F, and let it run. Walking color is what keeps a panel from holding a ghost of your last window. ½ brings you back here.";
    if wide {
        let lede_w = 520.0_f32.min(content_w * 0.46);
        texts.push(run(
            lede,
            x0 + content_w - lede_w,
            y + title_size * 0.15,
            lede_w,
            16.5,
            24.0,
            MUTED,
            false,
            false,
            0.0,
            None,
        ));
    } else {
        texts.push(run(
            lede,
            x0,
            y + 22.0 + title_size,
            content_w,
            16.5,
            24.0,
            MUTED,
            false,
            false,
            0.0,
            None,
        ));
    }

    y += header_h;
    rects.push(PaintRect {
        x: x0,
        y: y - 12.0,
        w: content_w,
        h: 1.0,
        color: LINE,
        radius: 0.0,
        mode: 0.0,
    });

    y = place_group(
        &day,
        x0,
        y,
        cols,
        card_w,
        card_h,
        gap,
        &mut cards,
        &mut texts,
    );
    y += 22.0;
    texts.push(run(
        "NIGHT",
        x0,
        y,
        content_w,
        11.5,
        16.0,
        AMBER,
        false,
        false,
        0.18,
        None,
    ));
    y += 26.0;
    y = place_group(
        &night,
        x0,
        y,
        cols,
        card_w,
        card_h,
        gap,
        &mut cards,
        &mut texts,
    );

    y += 22.0;
    texts.push(run(
        "This panel is IPS Black, so OLED-style burn-in is unlikely — a bright static window can still leave a faint afterimage. Every pixel here keeps moving, plus a slow whole-frame wander. Open a card, then click or press F for fullscreen. Keys 1–0 jump between versions. ½ returns to this gallery — the same key as the Quake console.",
        x0,
        y,
        content_w.min(760.0),
        14.5,
        22.0,
        MUTED,
        false,
        false,
        0.0,
        None,
    ));

    Layout {
        cards,
        texts,
        rects,
        scroll,
    }
}

fn place_group(
    indices: &[usize],
    x0: f32,
    mut y: f32,
    cols: usize,
    card_w: f32,
    card_h: f32,
    gap: f32,
    cards: &mut Vec<CardGeom>,
    texts: &mut Vec<TextRun>,
) -> f32 {
    let mut placed = 0;
    while placed < indices.len() {
        let row = (indices.len() - placed).min(cols);
        for col in 0..row {
            let index = indices[placed + col];
            let saver = &SAVERS[index];
            let x = x0 + col as f32 * (card_w + gap);
            cards.push(CardGeom {
                index,
                x,
                y,
                w: card_w,
                h: card_h,
            });
            let clip = Some([x, y, card_w, card_h]);
            let pad = 16.0;
            let name_size = (card_w * 0.105).clamp(20.0, 26.0);
            let body_size = 14.0;
            let chars_per_line = ((card_w - pad * 2.0) / (body_size * 0.52)).max(10.0);
            let lines = (saver.blurb.chars().count() as f32 / chars_per_line)
                .ceil()
                .clamp(1.0, 4.0);
            let block = 16.0 + 4.0 + name_size + 4.0 + lines * 18.0;
            let mut ty = y + card_h - pad - block;
            texts.push(run(
                saver.number,
                x + pad,
                ty,
                card_w - pad * 2.0,
                11.0,
                14.0,
                AMBER,
                false,
                false,
                0.14,
                clip,
            ));
            ty += 16.0;
            texts.push(run(
                saver.name,
                x + pad,
                ty,
                card_w - pad * 2.0,
                name_size,
                name_size,
                INK,
                true,
                true,
                0.0,
                clip,
            ));
            ty += name_size + 2.0;
            texts.push(run(
                saver.blurb,
                x + pad,
                ty,
                card_w - pad * 2.0,
                body_size,
                18.0,
                INK_SOFT,
                false,
                false,
                0.0,
                clip,
            ));
        }
        placed += row;
        y += card_h + gap;
    }
    y - gap
}

pub fn hud(index: usize, win_w: f32, win_h: f32) -> Vec<TextRun> {
    let saver = &SAVERS[index];
    let left = 28.0;
    let width = (win_w - left * 2.0).min(560.0);
    let title_size = (win_w * 0.04).clamp(32.0, 50.0);
    let hint_y = win_h - 36.0;
    let title_y = hint_y - 10.0 - title_size;
    let brand_y = title_y - 22.0;
    vec![
        run(
            &format!("SAVE THE SCREEN  ·  {} OF 10", index + 1),
            left,
            brand_y,
            width,
            11.5,
            16.0,
            BRAND,
            false,
            false,
            0.16,
            None,
        ),
        run(
            saver.name,
            left,
            title_y,
            width,
            title_size,
            title_size,
            INK,
            true,
            true,
            0.0,
            None,
        ),
        run(
            "Click or press F for fullscreen  ·  1–0 to switch  ·  ½ back to the gallery",
            left,
            hint_y,
            width,
            15.0,
            20.0,
            INK_SOFT,
            false,
            false,
            0.0,
            None,
        ),
    ]
}

pub fn card_paints(cards: &[CardGeom], hover: Option<usize>) -> Vec<PaintRect> {
    let mut rects = Vec::with_capacity(cards.len() * 3);
    for card in cards {
        let hot = hover == Some(card.index);
        rects.push(PaintRect {
            x: card.x,
            y: card.y,
            w: card.w,
            h: card.h,
            color: if hot { HOVER } else { LINE },
            radius: 14.0,
            mode: 0.0,
        });
        rects.push(PaintRect {
            x: card.x + 1.0,
            y: card.y + 1.0,
            w: card.w - 2.0,
            h: card.h - 2.0,
            color: RAISE,
            radius: 13.0,
            mode: 0.0,
        });
        rects.push(PaintRect {
            x: card.x + 1.0,
            y: card.y + 1.0,
            w: card.w - 2.0,
            h: card.h - 2.0,
            color: SHADOW,
            radius: 13.0,
            mode: 1.0,
        });
    }
    rects
}

fn run(
    text: &str,
    x: f32,
    y: f32,
    w: f32,
    size: f32,
    line_height: f32,
    color: [u8; 4],
    serif: bool,
    italic: bool,
    tracking: f32,
    clip: Option<[f32; 4]>,
) -> TextRun {
    TextRun {
        text: text.to_string(),
        x,
        y,
        w,
        size,
        line_height,
        color,
        serif,
        italic,
        tracking,
        clip,
    }
}

/// Whole-frame drift, matching the page's 97s CSS wander. Values are CSS pixels,
/// positive x to the right and positive y down.
pub fn wander(time_s: f32) -> (f32, f32) {
    const KEYS: [(f32, f32, f32); 5] = [
        (0.00, 0.0, 0.0),
        (0.20, 10.0, -8.0),
        (0.45, -9.0, 7.0),
        (0.70, 7.0, 10.0),
        (1.00, 0.0, 0.0),
    ];
    let t = time_s.rem_euclid(97.0) / 97.0;
    let mut i = 0;
    while i + 1 < KEYS.len() - 1 && t > KEYS[i + 1].0 {
        i += 1;
    }
    let (t0, x0, y0) = KEYS[i];
    let (t1, x1, y1) = KEYS[i + 1];
    let u = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
    let s = u * u * (3.0 - 2.0 * u);
    (x0 + (x1 - x0) * s, y0 + (y1 - y0) * s)
}
