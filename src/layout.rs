use crate::savers::Saver;

pub const BG: [f32; 4] = hex(0x12110e, 1.0);
pub const RAISE: [f32; 4] = hex(0x1b1914, 1.0);
pub const INK: [u8; 4] = [0xec, 0xe7, 0xdc, 255];
pub const MUTED: [u8; 4] = [0xae, 0xa8, 0x9a, 255];
pub const INK_SOFT: [u8; 4] = [0xec, 0xe7, 0xdc, 220];
pub const BRAND: [u8; 4] = [0xec, 0xe7, 0xdc, 158];
pub const AMBER: [u8; 4] = [0xe0, 0xa4, 0x5a, 255];

const LINE: [f32; 4] = hex(0xece7dc, 0.10);
const HOVER: [f32; 4] = hex(0xe0a45a, 0.55);

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
    /// The whole run is one code block. Inline code stays in `text` between backticks.
    pub block: bool,
}

#[derive(Clone, Copy)]
pub struct CardGeom {
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    /// The picture. Its shape is the screen's.
    pub h: f32,
    /// The name under the picture. Clicks and scrolling include it.
    pub tail: f32,
}

impl CardGeom {
    pub fn span(self) -> f32 {
        self.h + self.tail
    }
}

/// Width divided by height for a monitor. A missing size uses 16:9.
pub fn screen_aspect(width: f32, height: f32) -> f32 {
    if width.is_finite() && height.is_finite() && width >= 1.0 && height >= 1.0 {
        (width / height).clamp(0.25, 4.0)
    } else {
        16.0 / 9.0
    }
}

fn shelf_aspect(aspect: f32) -> f32 {
    if aspect.is_finite() && aspect > 0.0 {
        aspect.clamp(0.25, 4.0)
    } else {
        16.0 / 9.0
    }
}

#[derive(Clone, Copy, Debug)]
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
            (x >= card.x && y >= card.y && x < card.x + card.w && y < card.y + card.span())
                .then_some(card.index)
        })
    }
}

pub fn gallery_cols(win_w: f32) -> usize {
    if win_w <= 760.0 {
        1
    } else if win_w <= 1280.0 {
        3
    } else {
        5
    }
}

#[derive(Clone, Copy)]
pub enum Move {
    Previous,
    Next,
    Up,
    Down,
}

/// One step through the shelf. Left and right walk the name order and wrap.
/// Up and down move a row and stay put at the edge.
pub fn step_index(index: usize, count: usize, cols: usize, movement: Move) -> usize {
    if count == 0 {
        return 0;
    }
    let index = index.min(count - 1);
    let cols = cols.max(1);
    match movement {
        Move::Previous => (index as isize - 1).rem_euclid(count as isize) as usize,
        Move::Next => (index + 1) % count,
        Move::Up => {
            if index >= cols {
                index - cols
            } else {
                index
            }
        }
        Move::Down => {
            let next = index + cols;
            if next < count {
                next
            } else {
                index
            }
        }
    }
}

pub fn gallery(
    savers: &[Saver],
    shader_dir: &str,
    win_w: f32,
    win_h: f32,
    scroll: f32,
    hover: Option<usize>,
    detail: Option<u8>,
    pace: Option<u32>,
    aspect: f32,
) -> Layout {
    let cols = gallery_cols(win_w);
    let margin = if win_w <= 760.0 { 16.0 } else { 24.0 };
    let content_w = (win_w - margin * 2.0).min(1480.0);
    let x0 = (win_w - content_w) * 0.5;
    let gap = 14.4;
    let card_w = (content_w - gap * (cols as f32 - 1.0)) / cols as f32;
    // The picture matches the monitor, so the shelf shows the shader the way the screen will.
    let aspect = shelf_aspect(aspect);
    let picture_h = card_w / aspect;
    let name_size = (card_w * 0.085).clamp(16.0, 22.0);
    let tail = 8.0 + name_size + 4.0 + CARD_BODY_LH * 3.0;
    let card_h = picture_h + tail;
    let wide = win_w > 1280.0;
    let title_size = (win_w * 0.046).clamp(38.0, 70.0);
    let lede =
        "Open a card and let it run. `F` fills the screen. `Esc` or `Q` brings you back here.";
    let lede_h = estimate_lines(lede, content_w, LEDE_SIZE).max(1) as f32 * LEDE_LH + 4.0;

    let order: Vec<usize> = (0..savers.len()).collect();

    let group_h = |count: usize| -> f32 {
        if count == 0 {
            return 0.0;
        }
        let rows = count.div_ceil(cols) as f32;
        rows * card_h + (rows - 1.0).max(0.0) * gap
    };
    let header_h = if wide {
        22.0 + title_size + 18.0
    } else {
        22.0 + title_size + 12.0 + lede_h + 16.0
    };
    let footer_h = notes_block_height(shader_dir, content_w);
    let mut content_h = header_h;
    if savers.is_empty() {
        content_h += 48.0;
    } else {
        content_h += group_h(savers.len());
    }
    content_h += 22.0 + footer_h;
    // A short window scrolls instead of clipping the last row.
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

    let count = savers.len();
    let kicker = if count == 1 {
        "1 FULLSCREEN SAVER".to_string()
    } else {
        format!("{count} FULLSCREEN SAVERS")
    };
    texts.push(run(
        &kicker, x0, y, content_w, 12.5, 16.0, AMBER, false, false, 0.18, None,
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

    if wide {
        let lede_w = 520.0_f32.min(content_w * 0.46);
        texts.push(run(
            lede,
            x0 + content_w - lede_w,
            y + title_size * 0.15,
            lede_w,
            LEDE_SIZE,
            LEDE_LH,
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
            LEDE_SIZE,
            LEDE_LH,
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

    if order.is_empty() {
        texts.push(run(
            "No savers loaded.",
            x0,
            y,
            content_w,
            18.0,
            26.0,
            MUTED,
            false,
            false,
            0.0,
            None,
        ));
        y += 48.0;
    } else {
        y = place_group(
            savers, &order, x0, y, cols, card_w, picture_h, tail, name_size, gap, hover, detail,
            pace, &mut cards, &mut texts,
        );
    }

    y += 22.0;
    place_gallery_notes(shader_dir, x0, y, content_w, &mut texts);

    Layout {
        cards,
        texts,
        rects,
        scroll,
    }
}

struct GalleryNote {
    heading: &'static str,
    body: String,
}

const NOTE_HEAD: f32 = 16.0;
const NOTE_AFTER_HEAD: f32 = 6.0;
const NOTE_BODY_SIZE: f32 = 16.5;
const NOTE_BODY_LH: f32 = 25.0;
const NOTE_BLOCK_GAP: f32 = 22.0;
const NOTE_COL_GAP: f32 = 36.0;
const NOTE_COLUMNS: usize = 3;
/// Below this, the five notes stack so a column stays readable.
const NOTE_MIN_COL: f32 = 300.0;
const LEDE_SIZE: f32 = 18.0;
const LEDE_LH: f32 = 27.0;
const CARD_BODY_SIZE: f32 = 15.5;
const CARD_BODY_LH: f32 = 20.0;

/// Reading order. Wide windows lay the first three on one row and the last two on the next.
fn gallery_notes(shader_dir: &str) -> Vec<GalleryNote> {
    vec![
        GalleryNote {
            heading: "ON THE SHELF",
            body: "Each picture matches this screen. Rest the pointer on a card and it plays. Move away and it keeps that frame. `Arrows`, or `h j k l`, move through the cards. `Enter` opens the highlighted one.".into(),
        },
        GalleryNote {
            heading: "A CORNER MARK",
            body: "A corner mark means a channel is reading a `256` noise picture, or the shader needs a small edit. Hover the card and it says which. Open it for the line to change.".into(),
        },
        GalleryNote {
            heading: "WHILE IT RUNS",
            body: "`Left` and `Right`, or `h` and `l`, switch to the previous or next saver. `F` fills the screen. `Esc` or `Q` returns here. From this shelf, `Esc` or `Q` quits.".into(),
        },
        GalleryNote {
            heading: "PICTURE SIZE",
            body: "Keys `1` through `0` set the picture from 10% to the full window. On the shelf they set the highlighted card. While a saver runs, `j` and `k` step it by 10. `PgUp` and `PgDn` step the pace from 30 fps up to this screen. It opens at 50% and 30 fps, and both are remembered.".into(),
        },
        GalleryNote {
            heading: "ADD A SAVER",
            body: format!(
                "Save a Shadertoy `.frag` file in `{shader_dir}`. Start the app again and it joins the shelf, sorted by name."
            ),
        },
    ]
}

fn note_column_width(width: f32) -> Option<f32> {
    let col_w = (width - NOTE_COL_GAP * (NOTE_COLUMNS as f32 - 1.0)) / NOTE_COLUMNS as f32;
    (col_w >= NOTE_MIN_COL).then_some(col_w)
}

fn note_block_height(body: &str, width: f32) -> f32 {
    let lines = estimate_lines(body, width, NOTE_BODY_SIZE).max(1) as f32;
    NOTE_HEAD + NOTE_AFTER_HEAD + lines * NOTE_BODY_LH
}

fn column_height(notes: &[GalleryNote], width: f32) -> f32 {
    if notes.is_empty() {
        return 0.0;
    }
    let mut height = 0.0;
    for (index, note) in notes.iter().enumerate() {
        height += note_block_height(&note.body, width);
        if index + 1 != notes.len() {
            height += NOTE_BLOCK_GAP;
        }
    }
    height
}

fn notes_block_height(shader_dir: &str, width: f32) -> f32 {
    let notes = gallery_notes(shader_dir);
    let Some(col_w) = note_column_width(width) else {
        return column_height(&notes, width);
    };
    let row = |notes: &[GalleryNote]| {
        notes
            .iter()
            .map(|note| note_block_height(&note.body, col_w))
            .fold(0.0_f32, f32::max)
    };
    row(&notes[..3]) + NOTE_BLOCK_GAP + row(&notes[3..])
}

fn place_gallery_notes(shader_dir: &str, x: f32, y: f32, width: f32, texts: &mut Vec<TextRun>) {
    let notes = gallery_notes(shader_dir);
    let Some(col_w) = note_column_width(width) else {
        place_note_column(&notes, x, y, width, texts);
        return;
    };
    let mut row_bottom = y;
    for (index, note) in notes[..3].iter().enumerate() {
        let end = place_one_note(
            note,
            x + index as f32 * (col_w + NOTE_COL_GAP),
            y,
            col_w,
            texts,
        );
        row_bottom = row_bottom.max(end);
    }
    let y2 = row_bottom + NOTE_BLOCK_GAP;
    for (index, note) in notes[3..].iter().enumerate() {
        place_one_note(
            note,
            x + index as f32 * (col_w + NOTE_COL_GAP),
            y2,
            col_w,
            texts,
        );
    }
}

fn place_note_column(
    notes: &[GalleryNote],
    x: f32,
    mut y: f32,
    width: f32,
    texts: &mut Vec<TextRun>,
) -> f32 {
    for (index, note) in notes.iter().enumerate() {
        y = place_one_note(note, x, y, width, texts);
        if index + 1 != notes.len() {
            y += NOTE_BLOCK_GAP;
        }
    }
    y
}

fn place_one_note(note: &GalleryNote, x: f32, y: f32, width: f32, texts: &mut Vec<TextRun>) -> f32 {
    texts.push(run(
        note.heading,
        x,
        y,
        width,
        12.5,
        NOTE_HEAD,
        AMBER,
        false,
        false,
        0.16,
        None,
    ));
    let body_y = y + NOTE_HEAD + NOTE_AFTER_HEAD;
    push_paragraph(
        texts,
        &note.body,
        x,
        body_y,
        width,
        NOTE_BODY_SIZE,
        NOTE_BODY_LH,
        MUTED,
        false,
    )
}

fn place_group(
    savers: &[Saver],
    indices: &[usize],
    x0: f32,
    mut y: f32,
    cols: usize,
    card_w: f32,
    picture_h: f32,
    tail: f32,
    name_size: f32,
    gap: f32,
    hover: Option<usize>,
    detail: Option<u8>,
    pace: Option<u32>,
    cards: &mut Vec<CardGeom>,
    texts: &mut Vec<TextRun>,
) -> f32 {
    let mut placed = 0;
    while placed < indices.len() {
        let row = (indices.len() - placed).min(cols);
        for col in 0..row {
            let index = indices[placed + col];
            let saver = &savers[index];
            let x = x0 + col as f32 * (card_w + gap);
            cards.push(CardGeom {
                index,
                x,
                y,
                w: card_w,
                h: picture_h,
                tail,
            });
            let picture = Some([x, y, card_w, picture_h]);
            if hover == Some(index) && saver.hold.is_none() {
                if let Some(percent) = detail {
                    let label = match pace {
                        Some(pace) => format!("`{percent}% · {pace} fps`"),
                        None => format!("`{percent}%`"),
                    };
                    texts.push(run(
                        &label,
                        x + 14.0,
                        y + 12.0,
                        (card_w - 28.0).max(72.0),
                        14.5,
                        18.0,
                        AMBER,
                        false,
                        false,
                        0.06,
                        picture,
                    ));
                }
            }
            let pad = 2.0;
            let body_size = CARD_BODY_SIZE;
            let body = if hover == Some(index) {
                if let Some(hold) = &saver.hold {
                    Some((hold.card.as_str(), AMBER))
                } else if !saver.note.is_empty() {
                    Some((saver.note.as_str(), AMBER))
                } else if !saver.blurb.is_empty() {
                    Some((saver.blurb.as_str(), INK_SOFT))
                } else {
                    None
                }
            } else if !saver.blurb.is_empty() {
                Some((saver.blurb.as_str(), INK_SOFT))
            } else {
                None
            };
            let caption = Some([x, y + picture_h, card_w, tail]);
            let mut ty = y + picture_h + 8.0;
            texts.push(run(
                &saver.name,
                x + pad,
                ty,
                card_w - pad * 2.0,
                name_size,
                name_size,
                INK,
                true,
                true,
                0.0,
                caption,
            ));
            ty += name_size + 4.0;
            if let Some((text, color)) = body {
                texts.push(run(
                    text,
                    x + pad,
                    ty,
                    card_w - pad * 2.0,
                    body_size,
                    CARD_BODY_LH,
                    color,
                    false,
                    false,
                    0.0,
                    caption,
                ));
            }
        }
        placed += row;
        y += picture_h + tail + gap;
    }
    y - gap
}

pub fn hud(
    savers: &[Saver],
    index: usize,
    win_w: f32,
    win_h: f32,
    detail: u8,
    pace: u32,
) -> Vec<TextRun> {
    let saver = &savers[index];
    let left = 28.0;
    let width = (win_w - left * 2.0).max(160.0);
    let hint = "`F` fullscreen  ·  `h l` switch  ·  `j k` or `1–0` detail  ·  `PgUp` `PgDn` pace  ·  `Esc` or `Q` back";
    let hint_size = 16.5;
    let hint_lh = 24.0;
    let hint_lines = estimate_lines(hint, width, hint_size).max(1) as f32;
    let hint_y = win_h - 18.0 - hint_lines * hint_lh;
    let title_size = (win_w * 0.04).clamp(32.0, 50.0);
    let title_y = hint_y - 12.0 - title_size;
    let brand_y = title_y - 24.0;
    vec![
        run(
            &format!(
                "SAVE THE SCREEN  ·  {} OF {}  ·  {detail}%  ·  {pace} fps",
                index + 1,
                savers.len()
            ),
            left,
            brand_y,
            width,
            12.5,
            16.0,
            BRAND,
            false,
            false,
            0.16,
            None,
        ),
        run(
            &saver.name,
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
            hint, left, hint_y, width, hint_size, hint_lh, INK_SOFT, false, false, 0.0, None,
        ),
    ]
}

/// The still note for a shader this app could not compile. Paragraphs in `hold.page`
/// are separated by a blank line. Each run stays within the text buffer's line window.
pub fn hold_page(saver: &Saver, win_w: f32, win_h: f32) -> Vec<TextRun> {
    let Some(hold) = &saver.hold else {
        return Vec::new();
    };
    let left = 48.0_f32.min(win_w * 0.08).max(16.0);
    let width = (win_w - left * 2.0).clamp(160.0, 720.0);
    let mut y = 48.0;
    let mut texts = Vec::new();
    texts.push(run(
        "A SMALL EDIT",
        left,
        y,
        width,
        12.5,
        16.0,
        AMBER,
        false,
        false,
        0.16,
        None,
    ));
    y += 28.0;
    let title_size = (win_w * 0.04).clamp(28.0, 48.0);
    texts.push(run(
        &saver.name,
        left,
        y,
        width,
        title_size,
        title_size,
        INK,
        true,
        true,
        0.0,
        None,
    ));
    y += title_size + 20.0;
    for paragraph in hold.page.split("\n\n") {
        let raw = paragraph.trim();
        if raw.is_empty() {
            continue;
        }
        let (text, block) = if let Some(code) = fenced_block(raw) {
            (code, true)
        } else {
            (raw, is_code(raw))
        };
        if text.is_empty() {
            continue;
        }
        let size = if block { 17.5 } else { 19.5 };
        let line_height = if block { 24.0 } else { 28.0 };
        let color = if block { INK } else { INK_SOFT };
        y = push_paragraph(
            &mut texts,
            text,
            left,
            y,
            width,
            size,
            line_height,
            color,
            block,
        );
        y += if block { 18.0 } else { 14.0 };
    }
    if !saver.origin.is_empty() && !saver.origin.starts_with("built-in") {
        y = push_paragraph(
            &mut texts,
            &format!("The file is `{}`.", saver.origin),
            left,
            y,
            width,
            17.5,
            25.0,
            MUTED,
            false,
        );
        y += 14.0;
    }
    let hint_y = if y + 28.0 < win_h - 40.0 {
        win_h - 40.0
    } else {
        y + 8.0
    };
    texts.push(run(
        "`Esc` or `Q` brings you back to the shelf.",
        left,
        hint_y,
        width,
        16.5,
        24.0,
        INK_SOFT,
        false,
        false,
        0.0,
        None,
    ));
    texts
}

fn is_code(text: &str) -> bool {
    if text.contains('\n') {
        return text.contains("mat2") || text.contains('{') || text.contains('}');
    }
    text.contains("mat2(") && !text.contains("write ")
}

/// A markdown fence around a shader line. The fences are not drawn.
fn fenced_block(text: &str) -> Option<&str> {
    let rest = text.trim().strip_prefix("```\n")?;
    rest.strip_suffix("\n```")
}

/// Splits markdown inline code. An unclosed backtick stays as written.
pub(crate) fn inline_spans(text: &str) -> Vec<(&str, bool)> {
    let mut spans = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        if start > 0 {
            spans.push((&rest[..start], false));
        }
        let after = &rest[start + 1..];
        if let Some(end) = after.find('`') {
            if end > 0 {
                spans.push((&after[..end], true));
            }
            rest = &after[end + 1..];
        } else {
            spans.push((&rest[start..], false));
            rest = "";
            break;
        }
    }
    if !rest.is_empty() {
        spans.push((rest, false));
    }
    if spans.is_empty() {
        spans.push(("", false));
    }
    spans
}

/// Visible width, in roughly average characters. Backticks are not drawn.
/// Each inline span adds a little for the chip padding.
fn span_units(text: &str) -> usize {
    inline_spans(text)
        .into_iter()
        .map(|(slice, code)| {
            let count = slice.chars().count();
            if code {
                // Room for the side margin and the padding inside a code pill.
                count + 5
            } else {
                count
            }
        })
        .sum()
}

/// Glyphon keeps about eight lines per run. Hand it shorter pieces.
fn push_paragraph(
    texts: &mut Vec<TextRun>,
    text: &str,
    x: f32,
    mut y: f32,
    width: f32,
    size: f32,
    line_height: f32,
    color: [u8; 4],
    block: bool,
) -> f32 {
    let mut rest = text;
    while !rest.is_empty() {
        let (chunk, next) = take_lines(rest, width, size, 6, block);
        if chunk.is_empty() {
            break;
        }
        let lines = estimate_lines_block(chunk, width, size, block).max(1);
        let mut piece = run(
            chunk,
            x,
            y,
            width,
            size,
            line_height,
            color,
            false,
            false,
            0.0,
            None,
        );
        piece.block = block;
        texts.push(piece);
        y += lines as f32 * line_height;
        if next == rest {
            break;
        }
        rest = next.trim_start_matches('\n');
    }
    y
}

fn take_lines(text: &str, width: f32, size: f32, max_rows: usize, block: bool) -> (&str, &str) {
    let mut rows = 0;
    let mut end = 0;
    for line in text.split_inclusive('\n') {
        let wrapped = estimate_lines_block(line.trim_end_matches('\n'), width, size, block).max(1);
        if rows > 0 && rows + wrapped > max_rows {
            break;
        }
        rows += wrapped;
        end += line.len();
        if rows >= max_rows {
            break;
        }
    }
    if end == 0 {
        return (text, "");
    }
    (&text[..end], &text[end..])
}

fn estimate_lines(text: &str, width: f32, size: f32) -> usize {
    estimate_lines_block(text, width, size, false)
}

fn estimate_lines_block(text: &str, width: f32, size: f32, block: bool) -> usize {
    let factor = if block { 0.62 } else { 0.55 };
    let per = ((width / (size * factor)).floor() as usize).max(12);
    let mut rows = 0;
    for line in text.lines() {
        let count = if block {
            line.chars().count()
        } else {
            span_units(line)
        };
        rows += if count == 0 { 1 } else { count.div_ceil(per) };
    }
    if rows == 0 {
        1
    } else {
        rows
    }
}

/// A small mark in the top-right of a card that has something to say.
/// Drawn after the preview, so `mode` stays 0 and the caller appends these late.
pub fn note_marks(cards: &[CardGeom], savers: &[Saver]) -> Vec<PaintRect> {
    let mut marks = Vec::new();
    for card in cards {
        let Some(saver) = savers.get(card.index) else {
            continue;
        };
        if saver.note.is_empty() && saver.hold.is_none() {
            continue;
        }
        let d = 9.0;
        let x = card.x + card.w - 14.0 - d;
        let y = card.y + 14.0;
        marks.push(PaintRect {
            x: x - 1.5,
            y: y - 1.5,
            w: d + 3.0,
            h: d + 3.0,
            color: hex(0x0a0907, 0.72),
            radius: 20.0,
            mode: 0.0,
        });
        marks.push(PaintRect {
            x,
            y,
            w: d,
            h: d,
            color: hex(0xe0a45a, 1.0),
            radius: 20.0,
            mode: 0.0,
        });
    }
    marks
}

pub fn card_paints(cards: &[CardGeom], hover: Option<usize>) -> Vec<PaintRect> {
    let mut rects = Vec::with_capacity(cards.len() * 2);
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
        block: false,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_walk_the_grid() {
        assert_eq!(step_index(0, 10, 5, Move::Next), 1);
        assert_eq!(step_index(4, 10, 5, Move::Next), 5);
        assert_eq!(step_index(0, 10, 5, Move::Previous), 9);
        assert_eq!(step_index(9, 10, 5, Move::Next), 0);
        assert_eq!(step_index(6, 10, 5, Move::Up), 1);
        assert_eq!(step_index(2, 10, 5, Move::Up), 2);
        assert_eq!(step_index(3, 10, 5, Move::Down), 8);
        assert_eq!(step_index(8, 10, 5, Move::Down), 8);
        assert_eq!(step_index(0, 0, 5, Move::Next), 0);
    }

    fn sample() -> crate::savers::Saver {
        crate::savers::Saver {
            id: "veil".into(),
            name: "Veil".into(),
            blurb: "A quiet cloth.".into(),
            source: String::new(),
            origin: String::new(),
            note: String::new(),
            hold: None,
        }
    }

    #[test]
    fn a_held_card_keeps_its_mark_and_says_why() {
        let mut saver = sample();
        saver.hold = Some(crate::savers::Hold {
            card: "This shader's turn is written too briefly for this app. Open the card for the line to change.".into(),
            page: "Line 110\n\nreturn mat2(c.xy, c.zw);".into(),
        });
        let savers = [saver];
        let laid = gallery(
            &savers,
            "/shaders",
            1440.0,
            900.0,
            0.0,
            Some(0),
            Some(50),
            None,
            16.0 / 9.0,
        );
        let joined = laid
            .texts
            .iter()
            .map(|text| text.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("too briefly"));
        assert!(!joined.contains("quiet cloth"));
        assert!(laid.texts.iter().all(|text| text.text != "50%"));
        assert!(!note_marks(&laid.cards, &savers).is_empty());

        let page = hold_page(&savers[0], 1440.0, 900.0);
        let page_text = page
            .iter()
            .map(|text| text.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(page_text.contains("mat2(c.xy, c.zw)"));
        assert!(page
            .iter()
            .any(|text| text.block && text.text.contains("mat2")));
        assert!(page_text.contains("`Esc` or `Q`"));
    }

    #[test]
    fn cards_match_the_screen() {
        let savers = [sample()];
        let wide = gallery(
            &savers,
            "/shaders",
            1440.0,
            900.0,
            0.0,
            None,
            None,
            None,
            16.0 / 9.0,
        );
        let card = wide.cards[0];
        assert!((card.w / card.h - 16.0 / 9.0).abs() < 0.01);
        assert!(wide.hit(card.x + 8.0, card.y + card.h + 8.0).is_some());
        assert!(wide.hit(card.x + 8.0, card.y + card.span() + 4.0).is_none());

        let tall = gallery(
            &savers,
            "/shaders",
            800.0,
            1400.0,
            0.0,
            None,
            None,
            None,
            9.0 / 16.0,
        );
        let card = tall.cards[0];
        assert!(card.h > card.w);
        assert!((card.w / card.h - 9.0 / 16.0).abs() < 0.01);

        assert!((screen_aspect(3840.0, 2160.0) - 16.0 / 9.0).abs() < 1e-4);
        assert!((screen_aspect(2160.0, 3840.0) - 9.0 / 16.0).abs() < 1e-4);
        assert!((screen_aspect(0.0, 100.0) - 16.0 / 9.0).abs() < 1e-4);
    }

    fn note(laid: &Layout, heading: &str) -> TextRun {
        laid.texts
            .iter()
            .find(|text| text.text == heading)
            .cloned()
            .unwrap_or_else(|| panic!("missing {heading}"))
    }

    #[test]
    fn gallery_notes_are_grouped_for_anyone() {
        let savers = [sample()];
        let wide = gallery(
            &savers,
            "/shaders",
            1440.0,
            900.0,
            0.0,
            None,
            None,
            None,
            16.0 / 9.0,
        );
        let joined = wide
            .texts
            .iter()
            .map(|text| text.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Open a card and let it run."));
        assert!(joined.contains("/shaders"));
        assert!(!joined.contains("IPS"));
        assert!(!joined.contains("afterimage"));
        assert!(!joined.contains("ghost"));

        let shelf = note(&wide, "ON THE SHELF");
        let mark = note(&wide, "A CORNER MARK");
        let running = note(&wide, "WHILE IT RUNS");
        let size = note(&wide, "PICTURE SIZE");
        let add = note(&wide, "ADD A SAVER");
        let col = mark.x - shelf.x;
        assert!(col > 40.0, "three columns need a gap, got {col}");
        assert!((running.x - mark.x - col).abs() < 2.0);
        assert!((mark.y - shelf.y).abs() < 2.0);
        assert!((running.y - shelf.y).abs() < 2.0);
        assert!((size.x - shelf.x).abs() < 1.0);
        assert!((add.x - mark.x).abs() < 2.0);
        assert!((add.y - size.y).abs() < 2.0);
        assert!(size.y > shelf.y + 20.0);
        assert!((shelf.w - mark.w).abs() < 2.0);
        assert!((shelf.w - running.w).abs() < 2.0);
        assert!((shelf.w - size.w).abs() < 2.0);
        assert!((shelf.w - add.w).abs() < 2.0);
        assert!(
            add.x + add.w < running.x + 8.0,
            "row two stays left of the third column"
        );

        let narrow = gallery(
            &savers,
            "/shaders",
            800.0,
            1400.0,
            0.0,
            None,
            None,
            None,
            16.0 / 9.0,
        );
        let shelf = note(&narrow, "ON THE SHELF");
        let mark = note(&narrow, "A CORNER MARK");
        let running = note(&narrow, "WHILE IT RUNS");
        let size = note(&narrow, "PICTURE SIZE");
        let add = note(&narrow, "ADD A SAVER");
        assert!((mark.x - shelf.x).abs() < 1.0);
        assert!((running.x - shelf.x).abs() < 1.0);
        assert!(mark.y > shelf.y);
        assert!(running.y > mark.y);
        assert!(size.y > running.y);
        assert!(add.y > size.y);

        for text in &wide.texts {
            if (text.size - NOTE_BODY_SIZE).abs() < 0.1 {
                let lines = estimate_lines(&text.text, text.w, text.size);
                assert!(
                    lines <= 7,
                    "a note run stays inside the text window, got {lines}: {}",
                    text.text
                );
            }
        }

        let scrolled = gallery(
            &savers,
            "/shaders",
            1440.0,
            420.0,
            10_000.0,
            None,
            None,
            None,
            16.0 / 9.0,
        );
        let add = note(&scrolled, "ADD A SAVER");
        assert!(add.y > 0.0);
        assert!(add.y + 48.0 < 420.0);
    }

    #[test]
    fn keys_and_code_are_marked_for_the_screen() {
        let savers = [sample()];
        let laid = gallery(
            &savers,
            "/shaders",
            1440.0,
            900.0,
            0.0,
            None,
            None,
            None,
            16.0 / 9.0,
        );
        let joined = laid
            .texts
            .iter()
            .map(|text| text.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("`h j k l`"));
        assert!(joined.contains("`h` and `l`"));
        assert!(joined.contains("`PgUp`"));
        assert!(joined.contains("`PgDn`"));
        assert!(joined.contains("`Enter`"));
        assert!(joined.contains("`F`"));
        assert!(joined.contains("`Esc`"));
        assert!(joined.contains("`.frag`"));
        assert!(joined.contains("`/shaders`"));
        assert!(laid.texts.iter().all(|text| !text.block));

        let hovered = gallery(
            &savers,
            "/shaders",
            1440.0,
            900.0,
            0.0,
            Some(0),
            Some(50),
            Some(30),
            16.0 / 9.0,
        );
        assert!(hovered
            .texts
            .iter()
            .any(|text| text.text == "`50% · 30 fps`"));

        let hint = &hud(&savers, 0, 1440.0, 900.0, 50, 30)[2];
        assert!(hint.text.contains("`h l`"));
        assert!(hint.text.contains("`1–0`"));
        assert!(hint.text.contains("`PgUp`"));
        assert!(hint.text.contains("`PgDn`"));
        assert!(!hint.block);
        assert!(hint.size > 16.0);

        assert_eq!(
            inline_spans("or `h` and `l`."),
            vec![
                ("or ", false),
                ("h", true),
                (" and ", false),
                ("l", true),
                (".", false),
            ]
        );
        assert_eq!(
            inline_spans("press `h"),
            vec![("press ", false), ("`h", false)]
        );
        assert_eq!(
            fenced_block("```\nreturn textureLod(iChannel0, x, 0.0);\n```"),
            Some("return textureLod(iChannel0, x, 0.0);")
        );

        let mut held = sample();
        held.origin = "/shaders/iq.hell-2.frag".into();
        held.hold = Some(crate::savers::Hold {
            card: "needs a look".into(),
            page: "See `mainImage`.\n\n```\nreturn textureLod(iChannel0, x, 0.0);\n```".into(),
        });
        let page = hold_page(&held, 1440.0, 900.0);
        assert!(page.iter().all(|text| !text.text.contains("```")));
        assert!(page
            .iter()
            .any(|text| { !text.block && text.text.contains("`mainImage`") }));
        assert!(page.iter().any(|text| {
            text.block && text.text.contains("textureLod") && !text.text.contains('`')
        }));
        assert!(page
            .iter()
            .any(|text| text.text.contains("`/shaders/iq.hell-2.frag`")));
    }
}
