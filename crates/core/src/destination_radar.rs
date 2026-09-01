// Naygo — ranking puro y determinista del radar de destinos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationSource {
    OpenPanel,
    LastOperation,
    Favorite,
    Frequent,
    Recent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DestinationCandidate {
    pub path: PathBuf,
    pub label: String,
    pub source: DestinationSource,
}

/// Combina grupos ya ordenados por utilidad. La precedencia es explícita y los duplicados
/// conservan su primera aparición; no consulta disco ni depende de relojes/servicios.
pub fn rank(
    groups: impl IntoIterator<Item = Vec<DestinationCandidate>>,
    limit: usize,
) -> Vec<DestinationCandidate> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for group in groups {
        for candidate in group {
            let key = candidate.path.to_string_lossy().to_lowercase();
            if seen.insert(key) {
                result.push(candidate);
                if result.len() == limit {
                    return result;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(path: &str, source: DestinationSource) -> DestinationCandidate {
        DestinationCandidate {
            path: path.into(),
            label: path.into(),
            source,
        }
    }

    #[test]
    fn ranking_respeta_precedencia_deduplica_windows_y_limita() {
        let got = rank(
            [
                vec![c("C:/Uno", DestinationSource::OpenPanel)],
                vec![
                    c("c:/uno", DestinationSource::LastOperation),
                    c("D:/Dos", DestinationSource::LastOperation),
                ],
                vec![c("E:/Tres", DestinationSource::Favorite)],
            ],
            2,
        );
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].source, DestinationSource::OpenPanel);
        assert_eq!(got[1].path, PathBuf::from("D:/Dos"));
    }
}
