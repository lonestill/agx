use std::collections::HashMap;
use std::fs;
use colored::*;
use rusqlite::{Connection, OpenFlags};

use std::time::Duration;

#[derive(Debug, Default, Clone)]
pub struct ModelUsage {
    pub name: String,
    pub count: u64,
    pub input: u64,
    pub cache: u64,
    pub output: u64,
}

impl ModelUsage {
    pub fn total(&self) -> u64 {
        self.input + self.cache + self.output
    }

    pub fn cost(&self) -> f64 {
        let name_lower = self.name.to_lowercase();
        let (p_in, p_cache, p_out) = if name_lower.contains("sonnet") {
            (3.00, 0.30, 15.00)
        } else if name_lower.contains("opus") {
            (5.00, 0.50, 25.00)
        } else if name_lower.contains("pro") {
            (1.25, 0.12, 10.00)
        } else {
            (0.30, 0.03, 2.50)
        };

        (self.input as f64 * p_in + self.cache as f64 * p_cache + self.output as f64 * p_out) / 1_000_000.0
    }
}

// Protobuf wire types
#[allow(dead_code)]
#[derive(Debug, Clone)]
enum ProtoValue<'a> {
    Varint(u64),
    LengthDelimited(&'a [u8]),
    Fixed64(&'a [u8]),
    Fixed32(&'a [u8]),
}

fn decode_varint(data: &[u8], mut pos: usize) -> Option<(u64, usize)> {
    let mut res: u64 = 0;
    let mut shift = 0;
    while pos < data.len() {
        let b = data[pos];
        pos += 1;
        res |= ((b & 0x7F) as u64) << shift;
        shift += 7;
        if (b & 0x80) == 0 {
            return Some((res, pos));
        }
        if shift >= 64 {
            return None;
        }
    }
    None
}

fn parse_proto<'a>(data: &'a [u8]) -> HashMap<u32, Vec<ProtoValue<'a>>> {
    let mut fields: HashMap<u32, Vec<ProtoValue<'a>>> = HashMap::new();
    let mut pos = 0;
    let n = data.len();

    while pos < n {
        let (tag, next_pos) = match decode_varint(data, pos) {
            Some(v) => v,
            None => break,
        };
        pos = next_pos;
        let fn_num = (tag >> 3) as u32;
        let wt = tag & 0x7;

        match wt {
            0 => {
                // Varint
                if let Some((val, next_pos)) = decode_varint(data, pos) {
                    pos = next_pos;
                    fields.entry(fn_num).or_default().push(ProtoValue::Varint(val));
                } else {
                    break;
                }
            }
            1 => {
                // 64-bit
                if pos + 8 <= n {
                    let slice = &data[pos..pos + 8];
                    pos += 8;
                    fields.entry(fn_num).or_default().push(ProtoValue::Fixed64(slice));
                } else {
                    break;
                }
            }
            2 => {
                // Length-delimited
                if let Some((len, next_pos)) = decode_varint(data, pos) {
                    pos = next_pos;
                    let l = len as usize;
                    if pos + l <= n {
                        let slice = &data[pos..pos + l];
                        pos += l;
                        fields.entry(fn_num).or_default().push(ProtoValue::LengthDelimited(slice));
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            5 => {
                // 32-bit
                if pos + 4 <= n {
                    let slice = &data[pos..pos + 4];
                    pos += 4;
                    fields.entry(fn_num).or_default().push(ProtoValue::Fixed32(slice));
                } else {
                    break;
                }
            }
            _ => break,
        }
    }

    fields
}

fn extract_varint(fields: &HashMap<u32, Vec<ProtoValue>>, field_num: u32) -> u64 {
    if let Some(vals) = fields.get(&field_num) {
        if let Some(ProtoValue::Varint(v)) = vals.first() {
            return *v;
        }
    }
    0
}

fn extract_bytes<'a>(fields: &'a HashMap<u32, Vec<ProtoValue<'a>>>, field_num: u32) -> Option<&'a [u8]> {
    if let Some(vals) = fields.get(&field_num) {
        if let Some(ProtoValue::LengthDelimited(b)) = vals.first() {
            return Some(*b);
        }
    }
    None
}

pub fn collect_antigravity_tokens() -> (usize, u64, HashMap<String, ModelUsage>) {
    let conv_dir = crate::account::gemini_dir().join("antigravity/conversations");

    let mut sessions_count = 0;
    let mut requests_count = 0;
    let mut by_model: HashMap<String, ModelUsage> = HashMap::new();

    let entries = match fs::read_dir(&conv_dir) {
        Ok(e) => e,
        Err(_) => return (0, 0, by_model),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("db") {
            sessions_count += 1;
            // Open read-only with busy timeout and URI fallback for active WAL locks on Windows
            let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .or_else(|_| {
                    let uri = format!("file:{}?mode=ro&immutable=1", path.to_string_lossy().replace('\\', "/"));
                    Connection::open_with_flags(uri, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI)
                });

            let conn = match conn {
                Ok(c) => {
                    let _ = c.busy_timeout(Duration::from_millis(1500));
                    c
                }
                Err(_) => continue,
            };

            let mut stmt = match conn.prepare("SELECT data FROM gen_metadata") {
                Ok(s) => s,
                Err(_) => continue,
            };

            let rows = match stmt.query_map([], |row| row.get::<_, Vec<u8>>(0)) {
                Ok(r) => r,
                Err(_) => continue,
            };

            for row in rows.flatten() {
                let top = parse_proto(&row);
                if let Some(m1_bytes) = extract_bytes(&top, 1) {
                    let m1 = parse_proto(m1_bytes);
                    let model_name = extract_bytes(&m1, 19)
                        .and_then(|b| std::str::from_utf8(b).ok())
                        .unwrap_or("unknown")
                        .to_string();

                    let mut in_tok = 0;
                    let mut out_tok = 0;
                    let mut cache_tok = 0;

                    if let Some(m4_bytes) = extract_bytes(&m1, 4) {
                        let m4 = parse_proto(m4_bytes);
                        in_tok = extract_varint(&m4, 2);
                        out_tok = extract_varint(&m4, 3);
                        cache_tok = extract_varint(&m4, 5);
                    } else if let Some(m17_bytes) = extract_bytes(&m1, 17) {
                        let m17 = parse_proto(m17_bytes);
                        if let Some(m4_bytes) = extract_bytes(&m17, 2) {
                            let m4 = parse_proto(m4_bytes);
                            in_tok = extract_varint(&m4, 2);
                            out_tok = extract_varint(&m4, 3);
                            cache_tok = extract_varint(&m4, 5);
                        }
                    }

                    if in_tok > 0 || out_tok > 0 || cache_tok > 0 {
                        requests_count += 1;
                        let usage = by_model.entry(model_name.clone()).or_insert_with(|| ModelUsage {
                            name: model_name,
                            ..Default::default()
                        });
                        usage.count += 1;
                        usage.input += in_tok;
                        usage.output += out_tok;
                        usage.cache += cache_tok;
                    }
                }
            }
        }
    }

    (sessions_count, requests_count, by_model)
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.2}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn print_token_stats() {
    let t0 = std::time::Instant::now();
    let (sessions, reqs, by_model) = collect_antigravity_tokens();
    let elapsed = t0.elapsed();

    let mut total_in = 0;
    let mut total_cache = 0;
    let mut total_out = 0;
    let mut total_cost = 0.0;

    let mut models: Vec<&ModelUsage> = by_model.values().collect();
    models.sort_by_key(|m| std::cmp::Reverse(m.total()));

    for m in &models {
        total_in += m.input;
        total_cache += m.cache;
        total_out += m.output;
        total_cost += m.cost();
    }

    let total_all = total_in + total_cache + total_out;
    let cache_pct = if (total_in + total_cache) > 0 {
        (total_cache as f64 / (total_in + total_cache) as f64) * 100.0
    } else {
        0.0
    };

    println!("{}", "=== Antigravity Token Consumption Stats ===".bold());
    println!("  Scanned: {} conversation databases ({} total generations) in {}ms\n",
        sessions.to_string().cyan(),
        reqs.to_string().cyan(),
        elapsed.as_millis().to_string().dimmed()
    );

    println!("{:<28} {:>7} {:>10} {:>10} {:>10} {:>10} {:>9}",
        "MODEL".bold(), "GENS".bold(), "INPUT".bold(), "CACHED".bold(), "OUTPUT".bold(), "TOTAL".bold(), "EST COST".bold()
    );
    println!("{}", "-".repeat(90).dimmed());

    for m in &models {
        let cost_str = format!("${:.2}", m.cost());
        println!("{:<28} {:>7} {:>10} {:>10} {:>10} {:>10} {:>9}",
            m.name.cyan(),
            m.count,
            format_tokens(m.input),
            format_tokens(m.cache).green(),
            format_tokens(m.output),
            format_tokens(m.total()).bold(),
            cost_str.yellow()
        );
    }

    println!("{}", "-".repeat(90).dimmed());
    println!("{:<28} {:>7} {:>10} {:>10} {:>10} {:>10} {:>9}",
        "SUMMARY".bold().green(),
        reqs,
        format_tokens(total_in),
        format_tokens(total_cache).green(),
        format_tokens(total_out),
        format_tokens(total_all).bold().green(),
        format!("${:.2}", total_cost).yellow().bold()
    );

    println!("\n  Prompt Cache Efficiency: {} of input tokens served from cache",
        format!("{:.1}%", cache_pct).green().bold()
    );
    println!("  Total estimated API value: {}\n",
        format!("${:.2}", total_cost).yellow().bold()
    );
}
