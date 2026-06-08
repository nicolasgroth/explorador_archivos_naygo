// Naygo — formateo de tamaños legibles (B/KB/MB/GB/TB). Puro y reutilizable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `human_size` formatea un número de bytes a una cadena legible (1 decimal). Base
//! 1024. Reutilizado por tamaños de archivo, de transferencia y de disco.

const KB: f64 = 1024.0;
const MB: f64 = KB * 1024.0;
const GB: f64 = MB * 1024.0;
const TB: f64 = GB * 1024.0;

/// Formatea `bytes` como "B/KB/MB/GB/TB" con un decimal (salvo bytes crudos).
pub fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= TB {
        format!("{:.1} TB", b / TB)
    } else if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_crudos() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1023), "1023 B");
    }

    #[test]
    fn kilobytes() {
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1536), "1.5 KB");
    }

    #[test]
    fn mega_giga_tera() {
        assert_eq!(human_size(1024 * 1024), "1.0 MB");
        assert_eq!(human_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(human_size(1024u64 * 1024 * 1024 * 1024), "1.0 TB");
    }
}
