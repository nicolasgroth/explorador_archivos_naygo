# Narrow regression-fixture fixes, following preview-fixes.py.
from pathlib import Path
p=Path('crates/core/src/config/mod.rs');s=p.read_text(encoding='utf-8')
a=s.index('    fn settings_round_trip()');i=s.index('            preview_rules:',a)
s=s[:i]+'            preview_policy: crate::preview_policy::PreviewPolicy::default(),\n'+s[i:]
p.write_text(s,encoding='utf-8',newline='\n')
p=Path('crates/core/src/mesh_preview.rs');s=p.read_text(encoding='utf-8')
assert '.any(|pixel| pixel != [28, 31, 36, 255])' in s
p.write_text(s.replace('.any(|pixel| pixel != [28, 31, 36, 255])','.any(|pixel| *pixel != [28, 31, 36, 255])'),encoding='utf-8',newline='\n')
