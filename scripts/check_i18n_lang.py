# Naygo — valida un idioma contra es.json (parity de claves + placeholders).
# Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
# Uso: python scripts/check_i18n_lang.py <code>   (p. ej. fr, de, zh)
import json, re, sys

base = "crates/core/src/i18n"
code = sys.argv[1]
es = json.load(open(f"{base}/es.json", encoding="utf-8"))
try:
    lang = json.load(open(f"{base}/{code}.json", encoding="utf-8"))
except json.JSONDecodeError as e:
    print(f"[{code}] JSON INVÁLIDO: {e}")
    sys.exit(1)

missing = sorted(set(es) - set(lang))
extra = sorted(set(lang) - set(es))

def ph(s):
    return set(re.findall(r"\{[^}]*\}", s))

# batch.help.text tiene tokens de plantilla traducibles a propósito (se excluye)
skip = {"batch.help.text"}
bad_ph = [k for k in es if k not in skip and k in lang and ph(es[k]) != ph(lang[k])]

ok = not missing and not extra and not bad_ph
print(f"[{code}] {'OK' if ok else 'PROBLEMAS'}")
if missing:
    print(f"  faltan {len(missing)}: {missing[:15]}{' …' if len(missing) > 15 else ''}")
if extra:
    print(f"  sobran {len(extra)}: {extra[:15]}{' …' if len(extra) > 15 else ''}")
if bad_ph:
    print(f"  placeholders distintos {len(bad_ph)}: {bad_ph[:15]}")
sys.exit(0 if ok else 1)
