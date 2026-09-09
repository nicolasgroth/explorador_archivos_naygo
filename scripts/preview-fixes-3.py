# Apply integration corrections, then machine-applicable Clippy suggestions.
from pathlib import Path
import subprocess
import sys

def replace(name, old, new):
    p=Path(name); s=p.read_text(encoding='utf-8')
    assert old in s, (name, old)
    p.write_text(s.replace(old,new),encoding='utf-8',newline='\n')

replace('crates/ui-slint/src/tick.rs','use slint::{ModelRc, SharedString, TimerMode, VecModel};','use slint::{Model, ModelRc, SharedString, TimerMode, VecModel};')
replace('crates/ui-slint/src/preview.rs','doc.extract_text_with_limit(&selected, 4 * 1024 * 1024)','doc.extract_text(&selected)')
replace('crates/ui-slint/src/preview.rs','// Load once; extract only the first three pages with a per-page expansion budget.\n    // The disposable process also enforces a wall-time and address-space budget.','// Load once and extract only the first three pages. lopdf 0.41 does not expose\n    // bounded page expansion; the disposable process enforces wall time and committed\n    // memory limits for both loading and extraction, including hostile streams.')
replace('docs/PREVIEW_AND_PATHS.md','PDF: una carga, tres páginas como máximo, expansión por página limitada a 4 MiB.\nSe elimina `pdf-extract` y su segunda familia de dependencias PDF.','PDF: una carga y tres páginas como máximo. La versión conservada de lopdf (0.41)\nno ofrece extracción con presupuesto por página: la protección frente a expansión\nexcesiva es el límite de memoria y tiempo del proceso desechable. El texto mostrado\nse limita adicionalmente a 8000 caracteres. Se elimina `pdf-extract` y su segunda\nfamilia de dependencias PDF.')
subprocess.run(['cargo','fmt','--all'],check=True)
Path('validation').mkdir(exist_ok=True)
with open('validation/lint-fixes.log','wb') as log:
    p=subprocess.Popen(['cargo','clippy','--fix','--workspace','--all-targets','--allow-dirty','--allow-staged','--','-D','warnings'],stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    for line in p.stdout:
        log.write(line);log.flush();sys.stdout.buffer.write(line);sys.stdout.buffer.flush()
    # The subsequent strict gate is authoritative; do not hide non-fixable diagnostics.
    print('Machine-applicable lint pass exit status:',p.wait())
subprocess.run(['cargo','fmt','--all'],check=True)
