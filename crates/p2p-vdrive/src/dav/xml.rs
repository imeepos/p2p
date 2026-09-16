//! WebDAV 报文 XML：multistatus（PROPFIND 应答）与 lock 假应答、
//! HTTP 日期（RFC 1123）与 ISO 创建日期。
//!
//! 客户端请求体统一忽略（恒按 allprop 应答，属性为超集，各主流挂载端
//! 兼容；specs/vdrive.md §6）。

use crate::wire::{Entry, EntryKind};

/// XML 属性/文本转义。
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// RFC 1123 GMT（getlastmodified）。
pub fn http_date(unix_secs: u64) -> String {
    let (y, mo, d, h, mi, s) = civil_from_unix(unix_secs);
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(mo - 1) as usize];
    let weekday = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        [(((unix_secs / 86_400) + 3) % 7) as usize];
    format!("{weekday}, {d:02} {month} {y} {h:02}:{mi:02}:{s:02} GMT")
}

/// ISO 8601（creationdate）。
pub fn iso_date(unix_secs: u64) -> String {
    let (y, mo, d, h, mi, s) = civil_from_unix(unix_secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// unix 秒 → (年,月,日,时,分,秒)。
fn civil_from_unix(unix_secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let (y, mo, d) = civil_from_days((unix_secs / 86_400) as i64);
    let rem = unix_secs % 86_400;
    (
        y,
        mo,
        d,
        (rem / 3600) as u32,
        ((rem % 3600) / 60) as u32,
        (rem % 60) as u32,
    )
}

/// Howard Hinnant civil_from_days：epoch 天数 → (年, 月, 日)。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = ((mp + 2) % 12 + 1) as u32;
    // mp>=10 为 1/2 月（公元纪年 +1），mp<10 为 3..12 月。
    (y + (mp >= 10) as i64, m, d)
}

/// PROPFIND 207 multistatus：entries[0] 为目标自身，其余为 Depth:1 子项。
pub fn multistatus(entries: &[Entry]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<D:multistatus xmlns:D=\"DAV:\">\n",
    );
    for e in entries {
        let href = href_of(e);
        xml.push_str("<D:response>\n");
        xml.push_str(&format!("<D:href>{}</D:href>\n", escape(&href)));
        xml.push_str("<D:propstat><D:prop>\n");
        xml.push_str(&format!(
            "<D:displayname>{}</D:displayname>\n",
            escape(&e.name)
        ));
        match e.kind {
            EntryKind::Dir => xml.push_str("<D:resourcetype><D:collection/></D:resourcetype>\n"),
            EntryKind::File => {
                xml.push_str("<D:resourcetype/>\n");
                xml.push_str(&format!("<D:getcontentlength>{}</D:getcontentlength>\n", e.size));
                xml.push_str(&format!(
                    "<D:getetag>\"{}-{}\"</D:getetag>\n",
                    e.size, e.mtime
                ));
            }
        }
        xml.push_str(&format!(
            "<D:getlastmodified>{}</D:getlastmodified>\n",
            http_date(e.mtime)
        ));
        xml.push_str(&format!(
            "<D:creationdate>{}</D:creationdate>\n",
            iso_date(e.ctime)
        ));
        xml.push_str(
            "<D:supportedlock><D:lockentry><D:lockscope><D:exclusive/></D:lockscope>\
<D:locktype><D:write/></D:locktype></D:lockentry></D:supportedlock>\n",
        );
        xml.push_str("<D:status>HTTP/1.1 200 OK</D:status>\n");
        xml.push_str("</D:prop></D:propstat>\n</D:response>\n");
    }
    xml.push_str("</D:multistatus>");
    xml
}

fn href_of(e: &Entry) -> String {
    match (e.name.is_empty(), e.kind) {
        (true, _) => "/".into(),
        (false, EntryKind::Dir) => format!("/{}/", e.name),
        (false, EntryKind::File) => format!("/{}", e.name),
    }
}

/// LOCK 假应答（桥不持锁：单写者语义由远端后端自管）。
pub fn lock_response(token: &str, depth: &str, owner: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<D:prop xmlns:D=\"DAV:\">\
<D:lockdiscovery><D:activelock><D:locktype><D:write/></D:locktype>\
<D:lockscope><D:exclusive/></D:lockscope><D:depth>{depth}</D:depth>\
<D:owner>{}</D:owner><D:locktoken><D:href>{token}</D:href></D:locktoken>\
</D:activelock></D:lockdiscovery></D:prop>",
        escape(owner)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_all_meta() {
        assert_eq!(escape("a<&>\"'"), "a&lt;&amp;&gt;&quot;&apos;");
    }

    #[test]
    fn dates_roundtrip_known_instants() {
        // epoch 0 = 1970-01-01 周四。
        assert_eq!(http_date(0), "Thu, 01 Jan 1970 00:00:00 GMT");
        // 2026-09-15T00:00:00Z（周二：2024-01-01 周一 + 988 天，988%7=1）。
        assert_eq!(http_date(1_789_430_400), "Tue, 15 Sep 2026 00:00:00 GMT");
        assert_eq!(iso_date(1_789_430_400), "2026-09-15T00:00:00Z");
        // 闰年 2024-02-29（周四）= 1709164800。
        assert_eq!(iso_date(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn multistatus_contains_collection_and_file() {
        let entries = vec![
            Entry {
                name: String::new(),
                kind: EntryKind::Dir,
                size: 0,
                mtime: 0,
                ctime: 0,
            },
            Entry {
                name: "a.txt".into(),
                kind: EntryKind::File,
                size: 3,
                mtime: 1_789_516_800,
                ctime: 1_789_516_800,
            },
        ];
        let xml = multistatus(&entries);
        assert!(xml.contains("<D:href>/</D:href>"), "{xml}");
        assert!(xml.contains("<D:href>/a.txt</D:href>"));
        assert!(xml.contains("<D:collection/>"));
        assert!(xml.contains("<D:getcontentlength>3</D:getcontentlength>"));
    }
}
