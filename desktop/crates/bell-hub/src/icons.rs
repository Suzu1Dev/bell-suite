use bell_core::AppId;

pub const FAVICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" fill="none">
  <rect width="32" height="32" rx="8" fill="#141519"/>
  <rect x="0.75" y="0.75" width="30.5" height="30.5" rx="7.25" stroke="#ffffff" stroke-opacity="0.14" stroke-width="1.5"/>
  <path d="M16 6.75a7.25 7.25 0 0 0-7.25 7.25v4.5l-2.1 2.9a1.25 1.25 0 0 0 1.01 1.98h16.68a1.25 1.25 0 0 0 1.01-1.98l-2.1-2.9V14A7.25 7.25 0 0 0 16 6.75Z" stroke="#F7F8F8" stroke-width="2.2" stroke-linejoin="round"/>
  <path d="M13.3 25.8a2.9 2.9 0 0 0 5.4 0" stroke="#43CABC" stroke-width="2.2" stroke-linecap="round"/>
</svg>
"##;

pub fn build_favicon_ico_bytes() -> Vec<u8> {
    fn make_bmp_entry(size: u32) -> Vec<u8> {
        let xor_size = (size * size * 4) as usize;
        let and_row_bytes = size.div_ceil(32) as usize * 4;
        let and_size = and_row_bytes * size as usize;
        let mut dib = vec![0u8; 40 + xor_size + and_size];
        dib[0..4].copy_from_slice(&40u32.to_le_bytes());
        dib[4..8].copy_from_slice(&(size as i32).to_le_bytes());
        dib[8..12].copy_from_slice(&((size * 2) as i32).to_le_bytes());
        dib[12..14].copy_from_slice(&1u16.to_le_bytes());
        dib[14..16].copy_from_slice(&32u16.to_le_bytes());
        dib[20..24].copy_from_slice(&((xor_size + and_size) as u32).to_le_bytes());

        let cx = (size as f32 - 1.0) * 0.5;
        let r_corner = size as f32 * 0.22;
        for y in 0..size {
            let ty = (size - 1 - y) as f32 / (size - 1) as f32;
            for x in 0..size {
                let tx = x as f32 / (size - 1) as f32;
                let dx = ((x as f32 - cx).abs() - (cx - r_corner)).max(0.0);
                let dy = ((y as f32 - cx).abs() - (cx - r_corner)).max(0.0);
                let dist = dx.hypot(dy);
                if dist > r_corner + 0.6 {
                    continue;
                }
                let alpha = if dist > r_corner - 0.6 {
                    ((r_corner + 0.6 - dist) * 255.0).round() as u8
                } else {
                    255
                };
                let bx = (tx - 0.5).abs();
                let in_dome = (0.22..=0.62).contains(&ty)
                    && (bx / 0.24).hypot((ty - 0.45) / 0.23) <= 1.0;
                let in_skirt = ty > 0.45 && ty <= 0.72 && bx <= 0.24 + (ty - 0.45) * 0.42;
                let in_clapper = (0.77..=0.86).contains(&ty) && bx <= 0.09;
                let off = 40 + ((y * size + x) * 4) as usize;
                if in_clapper {
                    dib[off] = 0xBC;
                    dib[off + 1] = 0xCA;
                    dib[off + 2] = 0x43;
                } else if in_dome || in_skirt {
                    dib[off] = 0xF8;
                    dib[off + 1] = 0xF8;
                    dib[off + 2] = 0xF7;
                } else {
                    dib[off] = 0x19;
                    dib[off + 1] = 0x15;
                    dib[off + 2] = 0x14;
                }
                dib[off + 3] = alpha;
            }
        }
        dib
    }

    let img16 = make_bmp_entry(16);
    let img32 = make_bmp_entry(32);
    let header_len = 6 + 16 * 2;
    let mut out = Vec::with_capacity(header_len + img16.len() + img32.len());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());

    let mut offset = header_len as u32;
    for (sz, buf) in [(16u8, &img16), (32u8, &img32)] {
        out.push(sz);
        out.push(sz);
        out.push(0);
        out.push(0);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(buf.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += buf.len() as u32;
    }
    out.extend_from_slice(&img16);
    out.extend_from_slice(&img32);
    out
}

pub const fn bell() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M8 2a4.2 4.2 0 0 0-4.2 4.2v2.6L2.5 10.6a.7.7 0 0 0 .57 1.1h9.86a.7.7 0 0 0 .57-1.1l-1.3-1.8V6.2A4.2 4.2 0 0 0 8 2Z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><path d="M6.5 13.2a1.7 1.7 0 0 0 3 0" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#
}

pub const fn search() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><circle cx="7.2" cy="7.2" r="4.6" stroke="currentColor" stroke-width="1.4"/><path d="M10.7 10.7 13.8 13.8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#
}

pub const fn overview() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><rect x="2" y="2" width="5" height="5" rx="1.2" stroke="currentColor" stroke-width="1.35"/><rect x="9" y="2" width="5" height="5" rx="1.2" stroke="currentColor" stroke-width="1.35"/><rect x="2" y="9" width="5" height="5" rx="1.2" stroke="currentColor" stroke-width="1.35"/><rect x="9" y="9" width="5" height="5" rx="1.2" stroke="currentColor" stroke-width="1.35"/></svg>"#
}

pub const fn inbox() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M2.5 9.2h3.1l1 1.6h2.8l1-1.6h3.1M3.4 3h9.2l1.2 6.2v3.3a1 1 0 0 1-1 1H3.2a1 1 0 0 1-1-1V9.2L3.4 3Z" stroke="currentColor" stroke-width="1.35" stroke-linejoin="round"/></svg>"#
}

pub const fn layers() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M8 2.2 13.8 5.4 8 8.6 2.2 5.4 8 2.2Z" stroke="currentColor" stroke-width="1.35" stroke-linejoin="round"/><path d="m2.2 8.2 5.8 3.2 5.8-3.2M2.2 11l5.8 3.2L13.8 11" stroke="currentColor" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round"/></svg>"#
}

pub const fn database() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><ellipse cx="8" cy="3.8" rx="5.2" ry="1.9" stroke="currentColor" stroke-width="1.35"/><path d="M2.8 3.8v4.2c0 1.05 2.33 1.9 5.2 1.9s5.2-.85 5.2-1.9V3.8M2.8 8v4.2c0 1.05 2.33 1.9 5.2 1.9s5.2-.85 5.2-1.9V8" stroke="currentColor" stroke-width="1.35"/></svg>"#
}

pub const fn terminal() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><rect x="2" y="2.5" width="12" height="11" rx="1.8" stroke="currentColor" stroke-width="1.35"/><path d="m4.8 6 2.3 2-2.3 2M8.4 10h2.8" stroke="currentColor" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round"/></svg>"#
}

pub const fn app_icon(app: AppId) -> &'static str {
    match app {
        AppId::Personal => {
            r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><circle cx="8" cy="8" r="5.5" stroke="currentColor" stroke-width="1.4"/><path d="m5.6 8.1 1.6 1.6 3.3-3.4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#
        }
        AppId::Explain => {
            r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M8 2.2 9.6 6.4 13.8 8 9.6 9.6 8 13.8 6.4 9.6 2.2 8 6.4 6.4 8 2.2Z" stroke="currentColor" stroke-width="1.35" stroke-linejoin="round"/></svg>"#
        }
        AppId::Pr => {
            r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><circle cx="4.5" cy="4.2" r="1.6" stroke="currentColor" stroke-width="1.35"/><circle cx="4.5" cy="11.8" r="1.6" stroke="currentColor" stroke-width="1.35"/><circle cx="11.5" cy="11.8" r="1.6" stroke="currentColor" stroke-width="1.35"/><path d="M4.5 5.8v4.4M11.5 10.2V6.6a2.2 2.2 0 0 0-2.2-2.2H7.2" stroke="currentColor" stroke-width="1.35" stroke-linecap="round"/></svg>"#
        }
        AppId::Reader => {
            r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M2.5 3.6h4.2a1.3 1.3 0 0 1 1.3 1.3v7.8a1 1 0 0 0-1-1H2.5V3.6Zm11 0H9.3A1.3 1.3 0 0 0 8 4.9v7.8a1 1 0 0 1 1-1h4.5V3.6Z" stroke="currentColor" stroke-width="1.35" stroke-linejoin="round"/></svg>"#
        }
        AppId::Campaign => {
            r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M3 3.2v9.6M3 3.8h7.8l-1.7 2.6 1.7 2.6H3" stroke="currentColor" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round"/></svg>"#
        }
    }
}

pub const fn refresh() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M13.2 8a5.2 5.2 0 1 1-1.3-3.45M13.2 2.8v2.4h-2.4" stroke="currentColor" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round"/></svg>"#
}

pub const fn copy() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><rect x="5.5" y="5.5" width="8" height="8" rx="1.4" stroke="currentColor" stroke-width="1.35"/><path d="M10.5 5.5V3.8A1.3 1.3 0 0 0 9.2 2.5H3.8A1.3 1.3 0 0 0 2.5 3.8v5.4a1.3 1.3 0 0 0 1.3 1.3h1.7" stroke="currentColor" stroke-width="1.35"/></svg>"#
}

pub const fn play() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M5 3.6 12.2 8 5 12.4V3.6Z" stroke="currentColor" stroke-width="1.35" stroke-linejoin="round"/></svg>"#
}

pub const fn stop() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><rect x="4.2" y="4.2" width="7.6" height="7.6" rx="1.2" stroke="currentColor" stroke-width="1.35"/></svg>"#
}

pub const fn bolt() -> &'static str {
    r#"<svg class="ico" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M8.8 1.8 3.6 8.8h4.1l-.5 5.4 5.2-7H8.3l.5-5.4Z" stroke="currentColor" stroke-width="1.35" stroke-linejoin="round"/></svg>"#
}
