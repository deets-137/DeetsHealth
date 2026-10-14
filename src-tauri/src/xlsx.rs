// A small .xlsx writer for the dev inspector (feature `dev`; ops/DEBUGGING.md §5). Hand-rolled,
// so the dev tools add no crate (LESSONS.md §3.11): a workbook is a zip of a few XML parts, and
// a zip whose entries are stored (not compressed) needs only a CRC-32.
//
// What it writes: an About sheet, then one sheet per table. Every cell is text (an inline
// string) except the About sheet's row counts. Arial 10; a dark header row in white bold; the
// header row and the first column frozen; a filter on every column; column widths from the
// longest value, capped.

/// One sheet: its name and its rows, the header first.
pub struct SheetData<'a> {
    pub name: &'a str,
    pub rows: &'a [Vec<String>],
}

/// The workbook's bytes. `about` is the About sheet's lines: a title, then notes; then a table
/// of every sheet with its row count and `describe(name)`.
pub fn workbook(about: &[&str], sheets: &[SheetData], describe: impl Fn(&str) -> String) -> Vec<u8> {
    let names: Vec<String> = sheets.iter().map(|s| sheet_name(s.name)).collect();
    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();

    // The About sheet: its rows, with the counts as numbers.
    let mut about_rows: Vec<Vec<Cell>> = about.iter().map(|l| vec![Cell::Text((*l).to_owned(), if about[0] == *l { 2 } else { 0 })]).collect();
    about_rows.push(Vec::new());
    about_rows.push(vec![Cell::Text("Sheet".into(), 1), Cell::Text("Rows".into(), 1), Cell::Text("What it holds".into(), 1)]);
    for (s, n) in sheets.iter().zip(&names) {
        about_rows.push(vec![
            Cell::Text(n.clone(), 0),
            Cell::Number(s.rows.len().saturating_sub(1) as u64),
            Cell::Text(describe(s.name), 0),
        ]);
    }
    parts.push(("xl/worksheets/sheet1.xml".into(), sheet_xml(&about_rows, &[24.0, 8.0, 110.0], false, None).into_bytes()));

    for (i, s) in sheets.iter().enumerate() {
        let rows: Vec<Vec<Cell>> = s
            .rows
            .iter()
            .enumerate()
            .map(|(r, cells)| cells.iter().map(|c| Cell::Text(c.clone(), if r == 0 { 1 } else { 0 })).collect())
            .collect();
        let cols = s.rows.first().map_or(0, Vec::len);
        let widths: Vec<f64> = (0..cols)
            .map(|c| {
                let longest = s.rows.iter().filter_map(|r| r.get(c)).map(|v| v.chars().count()).max().unwrap_or(8);
                (longest as f64 + 2.0).clamp(8.0, 50.0)
            })
            .collect();
        let filter = (cols > 0).then(|| format!("A1:{}{}", column(cols - 1), s.rows.len().max(1)));
        parts.push((format!("xl/worksheets/sheet{}.xml", i + 2), sheet_xml(&rows, &widths, true, filter.as_deref()).into_bytes()));
    }

    // The workbook, its sheet list and the hidden filter ranges Excel expects.
    let mut sheet_list = String::from("<sheet name=\"About\" sheetId=\"1\" r:id=\"rId1\"/>");
    let mut defined = String::new();
    for (i, (s, n)) in sheets.iter().zip(&names).enumerate() {
        sheet_list.push_str(&format!("<sheet name=\"{}\" sheetId=\"{}\" r:id=\"rId{}\"/>", esc(n), i + 2, i + 2));
        let cols = s.rows.first().map_or(0, Vec::len);
        if cols > 0 {
            defined.push_str(&format!(
                "<definedName name=\"_xlnm._FilterDatabase\" localSheetId=\"{}\" hidden=\"1\">'{}'!$A$1:${}${}</definedName>",
                i + 1,
                esc(&n.replace('\'', "''")),
                column(cols - 1),
                s.rows.len().max(1)
            ));
        }
    }
    let defined = if defined.is_empty() { String::new() } else { format!("<definedNames>{defined}</definedNames>") };
    parts.push((
        "xl/workbook.xml".into(),
        format!(
            "{XML}<workbook xmlns=\"{MAIN}\" xmlns:r=\"{REL}\"><sheets>{sheet_list}</sheets>{defined}</workbook>"
        )
        .into_bytes(),
    ));

    let total = sheets.len() + 1;
    let mut rels = String::new();
    for i in 1..=total {
        rels.push_str(&format!(
            "<Relationship Id=\"rId{i}\" Type=\"{REL}/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>"
        ));
    }
    rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"{REL}/styles\" Target=\"styles.xml\"/>", total + 1));
    parts.push(("xl/_rels/workbook.xml.rels".into(), format!("{XML}<Relationships xmlns=\"{PKG}\">{rels}</Relationships>").into_bytes()));
    parts.push(("xl/styles.xml".into(), STYLES.as_bytes().to_vec()));

    let mut overrides = String::from(
        "<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>\
         <Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>",
    );
    for i in 1..=total {
        overrides.push_str(&format!(
            "<Override PartName=\"/xl/worksheets/sheet{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        ));
    }
    let content_types = format!(
        "{XML}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>{overrides}</Types>"
    );
    let root_rels = format!(
        "{XML}<Relationships xmlns=\"{PKG}\"><Relationship Id=\"rId1\" Type=\"{REL}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"
    );

    let mut all = vec![("[Content_Types].xml".to_owned(), content_types.into_bytes()), ("_rels/.rels".to_owned(), root_rels.into_bytes())];
    all.extend(parts);
    zip_stored(&all)
}

const XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>";
const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// Style 0: Arial 10. Style 1: the header (white bold on dark). Style 2: the About title.
const STYLES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
<fonts count=\"3\"><font><sz val=\"10\"/><name val=\"Arial\"/></font>\
<font><b/><sz val=\"10\"/><color rgb=\"FFFFFFFF\"/><name val=\"Arial\"/></font>\
<font><b/><sz val=\"12\"/><name val=\"Arial\"/></font></fonts>\
<fills count=\"3\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill>\
<fill><patternFill patternType=\"solid\"><fgColor rgb=\"FF4F4F7A\"/><bgColor indexed=\"64\"/></patternFill></fill></fills>\
<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>\
<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>\
<cellXfs count=\"3\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyAlignment=\"1\"><alignment vertical=\"top\"/></xf>\
<xf numFmtId=\"0\" fontId=\"1\" fillId=\"2\" borderId=\"0\" xfId=\"0\" applyFont=\"1\" applyFill=\"1\"/>\
<xf numFmtId=\"0\" fontId=\"2\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/></cellXfs>\
<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles></styleSheet>";

enum Cell {
    /// Text and its style.
    Text(String, u8),
    Number(u64),
}

fn sheet_xml(rows: &[Vec<Cell>], widths: &[f64], freeze: bool, filter: Option<&str>) -> String {
    let mut x = format!("{XML}<worksheet xmlns=\"{MAIN}\" xmlns:r=\"{REL}\">");
    if freeze {
        x.push_str(
            "<sheetViews><sheetView workbookViewId=\"0\"><pane xSplit=\"1\" ySplit=\"1\" topLeftCell=\"B2\" activePane=\"bottomRight\" state=\"frozen\"/>\
             <selection pane=\"bottomRight\"/></sheetView></sheetViews>",
        );
    }
    if !widths.is_empty() {
        x.push_str("<cols>");
        for (i, w) in widths.iter().enumerate() {
            x.push_str(&format!("<col min=\"{n}\" max=\"{n}\" width=\"{w:.1}\" customWidth=\"1\"/>", n = i + 1));
        }
        x.push_str("</cols>");
    }
    x.push_str("<sheetData>");
    for (r, cells) in rows.iter().enumerate() {
        x.push_str(&format!("<row r=\"{}\">", r + 1));
        for (c, cell) in cells.iter().enumerate() {
            let at = format!("{}{}", column(c), r + 1);
            match cell {
                Cell::Text(t, _) if t.is_empty() => {}
                Cell::Text(t, s) => x.push_str(&format!(
                    "<c r=\"{at}\" s=\"{s}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                    esc(&clip(t))
                )),
                Cell::Number(n) => x.push_str(&format!("<c r=\"{at}\"><v>{n}</v></c>")),
            }
        }
        x.push_str("</row>");
    }
    x.push_str("</sheetData>");
    if let Some(f) = filter {
        x.push_str(&format!("<autoFilter ref=\"{f}\"/>"));
    }
    x.push_str("</worksheet>");
    x
}

/// A cell holds at most 32 767 characters in Excel.
fn clip(t: &str) -> String {
    t.chars().take(32_767).collect()
}

/// `A`, `B`, … `Z`, `AA`, … for a zero-based column.
fn column(mut c: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// A sheet name Excel accepts: at most 31 characters, none of `[]:*?/\`.
fn sheet_name(n: &str) -> String {
    n.chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect()
}

/// XML text: the five escapes, and no control character XML forbids.
fn esc(t: &str) -> String {
    let mut o = String::with_capacity(t.len());
    for ch in t.chars() {
        match ch {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            '\t' | '\n' | '\r' => o.push(ch),
            c if (c as u32) < 0x20 => {}
            c => o.push(c),
        }
    }
    o
}

/// A zip archive with every entry stored (no compression).
fn zip_stored(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        let name_b = name.as_bytes();
        // Local file header.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        out.extend_from_slice(&0u16.to_le_bytes()); // time
        out.extend_from_slice(&0x21u16.to_le_bytes()); // date: 1980-01-01
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name_b);
        out.extend_from_slice(data);
        // Its central directory entry.
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // made by
        central.extend_from_slice(&20u16.to_le_bytes()); // needed
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0x21u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 8]); // extra length, comment length, disk, internal attributes
        central.extend_from_slice(&[0u8; 4]); // external attributes
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_b);
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]); // disk numbers
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_columns_and_escapes() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(column(0), "A");
        assert_eq!(column(25), "Z");
        assert_eq!(column(26), "AA");
        assert_eq!(column(701), "ZZ");
        assert_eq!(column(702), "AAA");
        assert_eq!(esc("a<b & \"c\"\u{1}"), "a&lt;b &amp; &quot;c&quot;");
        assert_eq!(sheet_name("a/b:c"), "abc");
    }

    #[test]
    fn a_workbook_is_a_zip_with_every_part() {
        let rows = vec![vec!["h1".to_owned(), "h2".to_owned()], vec!["x".to_owned(), "y".to_owned()]];
        let bytes = workbook(&["Title"], &[SheetData { name: "records", rows: &rows }], |_| "d".into());
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        // Walk the central directory from the end record, as a zip reader does.
        let le16 = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
        let le32 = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let end = bytes.len() - 22;
        assert_eq!(&bytes[end..end + 4], b"PK\x05\x06");
        let (count, mut at) = (le16(end + 10), le32(end + 16));
        let mut names = Vec::new();
        for _ in 0..count {
            assert_eq!(&bytes[at..at + 4], b"PK\x01\x02", "central entry signature");
            let (name_len, local) = (le16(at + 28), le32(at + 42));
            assert_eq!(&bytes[local..local + 4], b"PK\x03\x04", "local header at the offset");
            names.push(String::from_utf8(bytes[at + 46..at + 46 + name_len].to_vec()).unwrap());
            at += 46 + name_len + le16(at + 30) + le16(at + 32);
        }
        assert_eq!(at, end, "the directory ends where the end record starts");
        for part in ["[Content_Types].xml", "xl/workbook.xml", "xl/worksheets/sheet2.xml", "xl/styles.xml"] {
            assert!(names.iter().any(|n| n == part), "{part}");
        }
    }
}
