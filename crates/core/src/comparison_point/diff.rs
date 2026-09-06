// Naygo — comparación conservadora: ausencia sólo con cobertura conocida.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Missing,
    Unknown,
    SameMetadata,
    SameContent,
}
impl ChangeKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Added => "points.added",
            Self::Modified => "points.modified",
            Self::Missing => "points.missing",
            Self::Unknown => "points.unknown",
            Self::SameMetadata => "points.same_metadata",
            Self::SameContent => "points.same_content",
        }
    }
}
#[derive(Debug)]
pub struct Change {
    pub path: PathBuf,
    pub kind: ChangeKind,
    pub actionable: bool,
}
#[derive(Debug, Default)]
pub struct Comparison {
    pub rows: Vec<Change>,
    pub counts: [usize; 6],
    pub partial: bool,
    pub rows_truncated: bool,
}
pub fn compare(
    before: &ComparisonPoint,
    now: &ComparisonPoint,
    token: &CancellationToken,
) -> Result<Comparison, SpaceError> {
    checkpoint(token)?;
    before.validate()?;
    now.validate()?;
    if before.scope != now.scope
        || matches!((&before.resolved_root, &now.resolved_root), (Some(a), Some(b)) if key(a) != key(b))
    {
        return Err(SpaceError::Changed);
    }
    let mut all = BTreeMap::new();
    let before_gaps: HashSet<_> = before.gaps.iter().map(|p| key(p)).collect();
    let now_gaps: HashSet<_> = now.gaps.iter().map(|p| key(p)).collect();
    // Consultar ancestros indexados evita O(entradas × zonas inaccesibles).
    let covers = |point: &ComparisonPoint, gaps: &HashSet<String>, path: &Path| {
        !point.truncated && !path.ancestors().any(|p| gaps.contains(&key(p)))
    };
    for entry in &before.entries {
        all.entry(key(&entry.path)).or_insert((None, None)).0 = Some(entry);
    }
    for entry in &now.entries {
        all.entry(key(&entry.path)).or_insert((None, None)).1 = Some(entry);
    }
    let mut report = Comparison {
        partial: !before.complete() || !now.complete(),
        ..Default::default()
    };
    for (_, (old, new)) in all {
        checkpoint(token)?;
        let entry = new.or(old).ok_or(SpaceError::Invalid)?;
        let known = |e: &Record, p: &ComparisonPoint, gaps: &HashSet<String>| {
            e.kind != EntryKind::Other
                && !gaps.contains(&key(&e.path))
                && e.modified_ns.is_some()
                && (!p.scope.hashed || e.kind != EntryKind::File || e.sha256.is_some())
        };
        let kind = match (old, new) {
            (None, Some(n))
                if covers(before, &before_gaps, &n.path) && known(n, now, &now_gaps) =>
            {
                ChangeKind::Added
            }
            (Some(o), None) if covers(now, &now_gaps, &o.path) => ChangeKind::Missing,
            (Some(o), Some(n)) if known(o, before, &before_gaps) && known(n, now, &now_gaps) => {
                if o.path != n.path
                    || o.kind != n.kind
                    || o.bytes != n.bytes
                    || (n.kind == EntryKind::File
                        && (o.modified_ns != n.modified_ns || o.sha256 != n.sha256))
                {
                    ChangeKind::Modified
                } else if now.scope.hashed && n.kind == EntryKind::File {
                    ChangeKind::SameContent
                } else {
                    ChangeKind::SameMetadata
                }
            }
            _ => ChangeKind::Unknown,
        };
        report.counts[kind as usize] += 1;
        if matches!(kind, ChangeKind::SameContent | ChangeKind::SameMetadata) {
            continue;
        }
        if report.rows.len() == MAX_ROWS {
            report.rows_truncated = true;
            continue;
        }
        let actionable = matches!(kind, ChangeKind::Added | ChangeKind::Modified)
            && new.is_some_and(|n| n.kind == EntryKind::File);
        report.rows.push(Change {
            path: entry.path.clone(),
            kind,
            actionable,
        });
    }
    // Mostrar también zonas que no llegaron a producir una entrada (p. ej. raíz inaccesible).
    let mut shown: HashSet<_> = report.rows.iter().map(|r| key(&r.path)).collect();
    for gap in before.gaps.iter().chain(&now.gaps) {
        checkpoint(token)?;
        if !shown.insert(key(gap)) {
            continue;
        }
        report.counts[ChangeKind::Unknown as usize] += 1;
        if report.rows.len() == MAX_ROWS {
            report.rows_truncated = true;
            continue;
        }
        report.rows.push(Change {
            path: gap.clone(),
            kind: ChangeKind::Unknown,
            actionable: false,
        });
    }
    Ok(report)
}
